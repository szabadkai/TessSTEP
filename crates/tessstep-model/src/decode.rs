//! Bounded structural decoding against explicitly supplied reflection metadata.
//!
//! This first slice handles simple entities with single inheritance. Success is
//! structural validation of this subset, not full EXPRESS or AP conformance.
//! Values borrow the physical document; no generated owned record is constructed.
use crate::Document;
use std::{collections::BTreeMap, fmt};
use tessstep_part21::{EntityId, EntityKind, SourceSpan, StepValue, ValueKind};
use tessstep_schema::{
    AggregateKind, Attribute, AttributeKind, Builtin, DeclarationId, DeclarationKind, Domain,
    Expression, Schema, SchemaSet,
};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_work: usize,
    pub max_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 5_000_000,
            max_depth: 128,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    UnknownSchema,
    InvalidMetadata,
    UnknownEntity,
    AbstractEntity,
    AttributeCount,
    RequiredValue,
    TypeMismatch,
    Cardinality,
    Width,
    MissingReference,
    ReferenceType,
    Unsupported,
    ResourceLimit,
}
/// Physical source and owner are retained, including for nested values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub entity: Option<EntityId>,
    pub attribute: Option<&'static str>,
    pub source: Option<SourceSpan>,
    pub message: &'static str,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug)]
pub struct AttributeView<'a> {
    pub declaration: &'a Attribute,
    pub value: &'a StepValue,
}
#[derive(Clone, Debug)]
pub struct EntityView<'a> {
    pub id: EntityId,
    pub declaration: DeclarationId,
    pub attributes: Vec<AttributeView<'a>>,
}
/// Immutable borrowed results, published only after every instance passes.
/// Reference cycles are permitted; references are checked without graph expansion.
#[derive(Debug)]
pub struct DecodedDocument<'a> {
    document: &'a Document,
    schemas: &'a SchemaSet,
    entities: Vec<EntityView<'a>>,
    by_id: BTreeMap<EntityId, usize>,
}
impl<'a> DecodedDocument<'a> {
    pub fn document(&self) -> &'a Document {
        self.document
    }
    pub fn schemas(&self) -> &'a SchemaSet {
        self.schemas
    }
    pub fn entities(&self) -> &[EntityView<'a>] {
        &self.entities
    }
    pub fn get(&self, id: EntityId) -> Option<&EntityView<'a>> {
        self.by_id.get(&id).map(|&i| &self.entities[i])
    }
}

/// Decode using a caller-selected schema. FILE_SCHEMA and DATA population mapping
/// are not inferred. Parameterized DATA sections are explicitly unsupported.
/// The first error is deterministic; failures return no partially decoded view.
pub fn decode<'a>(
    document: &'a Document,
    schemas: &'a SchemaSet,
    schema_name: &str,
    limits: Limits,
) -> Result<DecodedDocument<'a>, Error> {
    let mut cx = Context {
        schemas,
        schema: None,
        document,
        limits,
        work: 0,
        entity: None,
        attribute: None,
        source: None,
        types: BTreeMap::new(),
    };
    for schema in schemas.schemas {
        cx.tick()?;
        if schema.name.eq_ignore_ascii_case(schema_name) {
            cx.schema = Some(schema);
            break;
        }
    }
    if cx.schema.is_none() {
        return Err(cx.error(ErrorKind::UnknownSchema, "selected schema is not supplied"));
    }
    // Opaque source constraints must never become a silent success.
    if !schemas.unsupported.is_empty() {
        return Err(cx.error(
            ErrorKind::Unsupported,
            "schema set contains unsupported EXPRESS semantics",
        ));
    }
    for section in document.data_sections() {
        cx.tick()?;
        if !section.parameters.is_empty() {
            cx.source = Some(section.source);
            return Err(cx.error(
                ErrorKind::Unsupported,
                "parameterized DATA population mapping",
            ));
        }
    }
    // Resolve all identities before inspecting values, allowing forward references.
    for entity in document.entities().iter() {
        cx.entity = Some(entity.id);
        cx.source = Some(entity.source);
        cx.tick()?;
        let EntityKind::Simple(record) = &entity.kind else {
            return Err(cx.error(ErrorKind::Unsupported, "complex entity mapping"));
        };
        let id = cx
            .lookup(&record.name)?
            .ok_or_else(|| cx.error(ErrorKind::UnknownEntity, "unknown entity name"))?;
        match cx.declaration(id)? {
            DeclarationKind::Entity {
                abstract_entity: true,
                ..
            } => {
                return Err(cx.error(
                    ErrorKind::AbstractEntity,
                    "abstract entity cannot be instantiated",
                ));
            }
            DeclarationKind::Entity { .. } => (),
            _ => return Err(cx.error(ErrorKind::UnknownEntity, "record name is not an entity")),
        }
        cx.types.insert(entity.id, id);
    }
    let mut entities = Vec::new();
    let mut by_id = BTreeMap::new();
    for entity in document.entities().iter() {
        cx.entity = Some(entity.id);
        cx.attribute = None;
        cx.source = Some(entity.source);
        let id = cx.types[&entity.id];
        let attributes = cx.attributes(id)?;
        let record = &entity.kind.records()[0];
        if record.parameters.len() != attributes.len() {
            return Err(cx.error(
                ErrorKind::AttributeCount,
                "physical parameter count differs from explicit attribute count",
            ));
        }
        let mut views = Vec::new();
        for (attribute, value) in attributes.into_iter().zip(&record.parameters) {
            cx.attribute = Some(attribute.name);
            cx.source = Some(value.source);
            cx.value(&attribute.domain, value, attribute.optional, 0)?;
            views.push(AttributeView {
                declaration: attribute,
                value,
            });
        }
        by_id.insert(entity.id, entities.len());
        entities.push(EntityView {
            id: entity.id,
            declaration: id,
            attributes: views,
        });
    }
    Ok(DecodedDocument {
        document,
        schemas,
        entities,
        by_id,
    })
}
struct Context<'a> {
    schemas: &'a SchemaSet,
    schema: Option<&'a Schema>,
    document: &'a Document,
    limits: Limits,
    work: usize,
    entity: Option<EntityId>,
    attribute: Option<&'static str>,
    source: Option<SourceSpan>,
    types: BTreeMap<EntityId, DeclarationId>,
}
impl<'a> Context<'a> {
    fn error(&self, kind: ErrorKind, message: &'static str) -> Error {
        Error {
            kind,
            entity: self.entity,
            attribute: self.attribute,
            source: self.source,
            message,
        }
    }
    fn tick(&mut self) -> Result<(), Error> {
        if self.work >= self.limits.max_work {
            return Err(self.error(ErrorKind::ResourceLimit, "schema decoding work budget"));
        }
        self.work += 1;
        Ok(())
    }
    fn lookup(&mut self, name: &str) -> Result<Option<DeclarationId>, Error> {
        for symbol in self.schema.expect("schema selected before lookup").symbols {
            self.tick()?;
            if symbol.name.eq_ignore_ascii_case(name) {
                return Ok(Some(symbol.declaration));
            }
        }
        Ok(None)
    }
    fn declaration(&mut self, id: DeclarationId) -> Result<DeclarationKind, Error> {
        self.tick()?;
        self.schemas
            .declaration(id)
            .map(|d| d.kind)
            .ok_or_else(|| self.error(ErrorKind::InvalidMetadata, "declaration ID is out of range"))
    }
    fn attributes(&mut self, mut id: DeclarationId) -> Result<Vec<&'a Attribute>, Error> {
        let mut chain = Vec::new();
        loop {
            if chain.len() >= self.limits.max_depth.min(128) {
                return Err(self.error(ErrorKind::ResourceLimit, "inheritance depth budget"));
            }
            let DeclarationKind::Entity {
                supertypes,
                attributes,
                unique,
                where_rules,
                supertype_constraint,
                ..
            } = self.declaration(id)?
            else {
                return Err(self.error(ErrorKind::InvalidMetadata, "supertype is not an entity"));
            };
            if !unique.is_empty() || !where_rules.is_empty() || supertype_constraint.is_some() {
                return Err(self.error(
                    ErrorKind::Unsupported,
                    "entity constraints, DERIVE or INVERSE",
                ));
            }
            for attribute in attributes {
                self.tick()?;
                if !matches!(attribute.kind, AttributeKind::Explicit) {
                    return Err(self.error(ErrorKind::Unsupported, "DERIVE or INVERSE attribute"));
                }
            }
            chain.push(attributes);
            match supertypes {
                [] => break,
                [parent] => id = *parent,
                _ => return Err(self.error(ErrorKind::Unsupported, "multiple inheritance mapping")),
            }
        }
        let mut result = Vec::new();
        for local in chain.into_iter().rev() {
            for attr in local {
                self.tick()?;
                result.push(attr);
            }
        }
        Ok(result)
    }
    fn bound(&mut self, expression: Expression) -> Result<i64, Error> {
        self.tick()?;
        expression
            .text
            .trim()
            .parse()
            .map_err(|_| self.error(ErrorKind::Unsupported, "nonliteral EXPRESS bound or width"))
    }
    fn value(
        &mut self,
        domain: &Domain,
        value: &StepValue,
        optional: bool,
        depth: usize,
    ) -> Result<(), Error> {
        self.source = Some(value.source);
        self.tick()?;
        if depth >= self.limits.max_depth.min(128) {
            return Err(self.error(ErrorKind::ResourceLimit, "domain/value depth budget"));
        }
        if matches!(value.kind, ValueKind::Null) {
            return if optional {
                Ok(())
            } else {
                Err(self.error(ErrorKind::RequiredValue, "required value is unset"))
            };
        }
        if matches!(value.kind, ValueKind::Omitted) {
            return Err(self.error(
                ErrorKind::TypeMismatch,
                "derived marker is not an explicit attribute value",
            ));
        }
        if matches!(
            value.kind,
            ValueKind::ValueReference(_)
                | ValueKind::ConstantValue(_)
                | ValueKind::ConstantEntity(_)
        ) {
            return Err(self.error(ErrorKind::Unsupported, "value references and constants"));
        }
        match *domain {
            Domain::Named(id) => match self.declaration(id)? {
                DeclarationKind::Type {
                    domain,
                    where_rules,
                } => {
                    if !where_rules.is_empty() {
                        return Err(self.error(ErrorKind::Unsupported, "type WHERE constraint"));
                    }
                    self.value(&domain, value, false, depth + 1)
                }
                DeclarationKind::Entity { .. } => self.reference(id, value),
                _ => Err(self.error(
                    ErrorKind::InvalidMetadata,
                    "named domain is neither type nor entity",
                )),
            },
            Domain::Builtin { kind, width, fixed } => {
                let valid = match (kind, &value.kind) {
                    (Builtin::Boolean, ValueKind::Enumeration(s)) => {
                        matches!(s.as_ref(), "T" | "F")
                    }
                    (Builtin::Logical, ValueKind::Enumeration(s)) => {
                        matches!(s.as_ref(), "T" | "F" | "U")
                    }
                    (Builtin::Integer, ValueKind::Integer(_))
                    | (
                        Builtin::Real | Builtin::Number,
                        ValueKind::Integer(_) | ValueKind::Real(_),
                    )
                    | (Builtin::String, ValueKind::String(_))
                    | (Builtin::Binary, ValueKind::Binary(_)) => true,
                    _ => false,
                };
                if !valid {
                    return Err(self.error(
                        ErrorKind::TypeMismatch,
                        "value does not match primitive domain",
                    ));
                }
                if let Some(width) = width {
                    let n = self.bound(width)?;
                    let len = match &value.kind {
                        ValueKind::String(s) => {
                            let mut count = 0;
                            for _ in s.chars() {
                                self.tick()?;
                                count += 1;
                            }
                            count
                        }
                        ValueKind::Binary(b) => b.bit_len,
                        _ => {
                            return Err(
                                self.error(ErrorKind::Unsupported, "numeric precision constraint")
                            );
                        }
                    };
                    if n < 0 || (fixed && len as u128 != n as u128) || len as u128 > n as u128 {
                        return Err(self.error(ErrorKind::Width, "STRING/BINARY width constraint"));
                    }
                }
                Ok(())
            }
            Domain::Enumeration(members) => {
                if let ValueKind::Enumeration(name) = &value.kind {
                    for member in members {
                        self.tick()?;
                        if member.eq_ignore_ascii_case(name) {
                            return Ok(());
                        }
                    }
                }
                Err(self.error(
                    ErrorKind::TypeMismatch,
                    "value is not an enumeration member",
                ))
            }
            Domain::Aggregate {
                kind,
                bounds,
                optional,
                unique,
                element,
            } => {
                if unique || kind == AggregateKind::Set {
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "aggregate value equality/uniqueness",
                    ));
                }
                let ValueKind::Aggregate(values) = &value.kind else {
                    return Err(self.error(ErrorKind::TypeMismatch, "expected aggregate value"));
                };
                if let Some((lower, upper)) = bounds {
                    let low = self.bound(lower)?;
                    let high = if upper.text.trim() == "?" {
                        None
                    } else {
                        Some(self.bound(upper)?)
                    };
                    let len = values.len() as i128;
                    let valid = if kind == AggregateKind::Array {
                        high.is_some_and(|high| {
                            high >= low && len == i128::from(high) - i128::from(low) + 1
                        })
                    } else {
                        low >= 0
                            && len >= i128::from(low)
                            && high.is_none_or(|high| high >= low && len <= i128::from(high))
                    };
                    if !valid {
                        return Err(
                            self.error(ErrorKind::Cardinality, "aggregate cardinality constraint")
                        );
                    }
                } else if kind == AggregateKind::Array {
                    return Err(self.error(ErrorKind::InvalidMetadata, "ARRAY requires bounds"));
                }
                if optional && kind != AggregateKind::Array {
                    return Err(self.error(
                        ErrorKind::InvalidMetadata,
                        "optional elements require ARRAY",
                    ));
                }
                for item in values {
                    self.value(element, item, optional, depth + 1)?;
                }
                Ok(())
            }
            Domain::Select(_) => Err(self.error(ErrorKind::Unsupported, "SELECT decoding")),
        }
    }
    fn reference(&mut self, expected: DeclarationId, value: &StepValue) -> Result<(), Error> {
        let ValueKind::Reference(target) = value.kind else {
            return Err(self.error(ErrorKind::TypeMismatch, "expected entity reference"));
        };
        let Some(mut actual) = self.types.get(&target).copied() else {
            for external in self.document.external_references() {
                self.tick()?;
                if external.id == tessstep_part21::Occurrence::Entity(target) {
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "external entity reference resolution",
                    ));
                }
            }
            return Err(self.error(
                ErrorKind::MissingReference,
                "entity reference target is missing",
            ));
        };
        for _ in 0..self.limits.max_depth.min(128) {
            self.tick()?;
            if actual == expected {
                return Ok(());
            }
            let DeclarationKind::Entity { supertypes, .. } = self.declaration(actual)? else {
                return Err(self.error(
                    ErrorKind::InvalidMetadata,
                    "reference target is not an entity",
                ));
            };
            match supertypes {
                [] => {
                    return Err(self.error(
                        ErrorKind::ReferenceType,
                        "target is not assignable to entity domain",
                    ));
                }
                [parent] => actual = *parent,
                _ => {
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "multiple inheritance reference compatibility",
                    ));
                }
            }
        }
        Err(self.error(
            ErrorKind::ResourceLimit,
            "reference inheritance depth budget",
        ))
    }
}
