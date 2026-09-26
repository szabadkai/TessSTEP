//! Bounded structural decoding against explicitly supplied reflection metadata.
//!
//! Simple and complex entities are decoded through schema metadata. Success is
//! structural validation of this subset, not full EXPRESS or AP conformance.
//! Values borrow the physical document; no generated owned record is constructed.
mod bounds;
mod equality;
mod hierarchy;
mod select;

use crate::Document;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
use tessstep_part21::{EntityId, EntityKind, Record, SourceSpan, StepValue, ValueKind};
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
    DuplicateValue,
    ComplexMapping,
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
    pub owner: DeclarationId,
    pub declaration: &'a Attribute,
    pub value: &'a StepValue,
}
#[derive(Clone, Debug)]
pub struct EntityView<'a> {
    pub id: EntityId,
    /// Single leaf for an internal mapping; None for external complex mapping.
    pub declaration: Option<DeclarationId>,
    /// Complete entity membership, including supertypes, without duplicates.
    pub types: Vec<DeclarationId>,
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
    decode_scope(document, schemas, schema_name, limits, None, &[], &[])
}

/// Structurally decode only the local entity-reference closure of explicit roots.
/// Unrelated records are not validated. FILE_SCHEMA is not interpreted and this
/// result does not establish schema conformance of the complete document.
/// Traversal is iterative, cycle-safe, and shares the decoding work budget.
pub fn decode_reachable<'a>(
    document: &'a Document,
    schemas: &'a SchemaSet,
    schema_name: &str,
    roots: &[EntityId],
    limits: Limits,
) -> Result<DecodedDocument<'a>, Error> {
    decode_scope(
        document,
        schemas,
        schema_name,
        limits,
        Some(roots),
        &[],
        &[],
    )
}

/// An explicitly declared physical-profile slot that must contain `*`.
/// The profile adapter, not the structural decoder, supplies its semantic value.
/// This is not general EXPRESS DERIVE evaluation or schema conformance.
#[derive(Clone, Copy, Debug)]
pub struct OmittedSlot {
    pub entity: DeclarationId,
    pub attribute: &'static str,
}

/// Decode a selected reference closure under an explicit physical mapping profile.
/// Only declared slots accept `*`, and those slots reject every other value.
/// Ordinary `decode` and `decode_reachable` never permit this override. Metadata
/// rules/unsupported diagnostics are still checked without suppression.
pub fn decode_reachable_profile<'a>(
    document: &'a Document,
    schemas: &'a SchemaSet,
    schema_name: &str,
    roots: &[EntityId],
    omitted: &[OmittedSlot],
    limits: Limits,
) -> Result<DecodedDocument<'a>, Error> {
    decode_scope(
        document,
        schemas,
        schema_name,
        limits,
        Some(roots),
        omitted,
        &[],
    )
}

/// An explicitly declared physical-profile reference that is retained but not followed.
/// Its value must be `$` (when optional) or a reference to a local entity. Neither the
/// target nor its closure is decoded or type-checked, so the declared domain is not
/// applied. Adapters use it for provenance links to geometry outside a reduced profile.
#[derive(Clone, Copy, Debug)]
pub struct LinkSlot {
    pub entity: DeclarationId,
    pub attribute: &'static str,
}

/// `decode_reachable_profile` that additionally leaves declared link slots unexpanded.
/// Link targets are absent from the result unless another decoded reference reaches them.
pub fn decode_reachable_profile_with_links<'a>(
    document: &'a Document,
    schemas: &'a SchemaSet,
    schema_name: &str,
    roots: &[EntityId],
    omitted: &[OmittedSlot],
    links: &[LinkSlot],
    limits: Limits,
) -> Result<DecodedDocument<'a>, Error> {
    decode_scope(
        document,
        schemas,
        schema_name,
        limits,
        Some(roots),
        omitted,
        links,
    )
}

fn decode_scope<'a>(
    document: &'a Document,
    schemas: &'a SchemaSet,
    schema_name: &str,
    limits: Limits,
    roots: Option<&[EntityId]>,
    omitted: &[OmittedSlot],
    links: &[LinkSlot],
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
    cx.check_diagnostics()?;
    let mut slots = BTreeSet::new();
    for slot in omitted {
        cx.tick()?;
        let mut matches = 0;
        for owner in cx.hierarchy(slot.entity)? {
            for attribute in cx.local_attributes(owner)? {
                cx.tick()?;
                if attribute.name == slot.attribute {
                    matches += 1;
                }
            }
        }
        if matches != 1 || !slots.insert((slot.entity, slot.attribute)) {
            return Err(cx.error(
                ErrorKind::InvalidMetadata,
                "ambiguous, missing or duplicate physical-profile slot",
            ));
        }
    }
    // Each link resolves to its single declaring owner, so equally named attributes
    // inherited from another branch are never mistaken for the link.
    let mut resolved = Vec::new();
    for slot in links {
        cx.tick()?;
        let mut owners = Vec::new();
        for owner in cx.hierarchy(slot.entity)? {
            for attribute in cx.local_attributes(owner)? {
                cx.tick()?;
                if attribute.name == slot.attribute {
                    owners.push(owner);
                }
            }
        }
        if owners.len() != 1 || !slots.insert((slot.entity, slot.attribute)) {
            return Err(cx.error(
                ErrorKind::InvalidMetadata,
                "ambiguous, missing or duplicate physical-profile slot",
            ));
        }
        resolved.push((slot.entity, owners[0], slot.attribute));
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
    let mut selected = BTreeSet::new();
    if let Some(roots) = roots {
        let mut pending = Vec::new();
        for &root in roots {
            cx.tick()?;
            if selected.insert(root) {
                pending.push(root);
            }
        }
        while let Some(id) = pending.pop() {
            cx.entity = Some(id);
            cx.source = document.entities().get(id).map(|e| e.source);
            cx.tick()?;
            let entity = document.entities().get(id).ok_or_else(|| {
                cx.error(
                    ErrorKind::MissingReference,
                    "closure references a nonlocal entity",
                )
            })?;
            let types = if resolved.is_empty() {
                Vec::new()
            } else {
                cx.instance_types(&entity.kind)?
            };
            for record in entity.kind.records() {
                cx.tick()?;
                let skip = if resolved.is_empty() {
                    Vec::new()
                } else {
                    let (_, attributes) = cx.record_attributes(&entity.kind, record)?;
                    let mut skip = Vec::new();
                    for (owner, attribute) in attributes {
                        skip.push(cx.is_link(&resolved, &types, owner, attribute)?);
                    }
                    // Without positional agreement no slot can be identified safely.
                    if skip.len() != record.parameters.len() {
                        return Err(cx.error(
                            ErrorKind::AttributeCount,
                            "physical parameter count differs from explicit attribute count",
                        ));
                    }
                    skip
                };
                let mut values = Vec::new();
                for (index, value) in record.parameters.iter().enumerate() {
                    cx.tick()?;
                    if !skip.get(index).copied().unwrap_or(false) {
                        values.push(value);
                    }
                }
                while let Some(value) = values.pop() {
                    cx.tick()?;
                    cx.source = Some(value.source);
                    match &value.kind {
                        ValueKind::Reference(target) => {
                            if document.entities().get(*target).is_none() {
                                return Err(cx.error(
                                    ErrorKind::MissingReference,
                                    "closure references a nonlocal entity",
                                ));
                            }
                            if selected.insert(*target) {
                                pending.push(*target);
                            }
                        }
                        ValueKind::Aggregate(items) => {
                            for item in items {
                                cx.tick()?;
                                values.push(item);
                            }
                        }
                        ValueKind::Typed { value, .. } => values.push(value),
                        _ => (),
                    }
                }
            }
        }
    }
    let mut instances = Vec::new();
    for entity in document.entities().iter() {
        cx.tick()?;
        if roots.is_none() || selected.contains(&entity.id) {
            instances.push(entity);
        }
    }
    // Resolve all identities before inspecting values, allowing forward references.
    for entity in &instances {
        cx.entity = Some(entity.id);
        cx.source = Some(entity.source);
        cx.tick()?;
        let members = cx.instance_types(&entity.kind)?;
        cx.types.insert(entity.id, members);
    }
    let mut entities = Vec::new();
    let mut by_id = BTreeMap::new();
    for entity in &instances {
        cx.entity = Some(entity.id);
        cx.attribute = None;
        cx.source = Some(entity.source);
        let mut views = Vec::new();
        let mut primary = None;
        let types = if resolved.is_empty() {
            Vec::new()
        } else {
            cx.types[&entity.id].clone()
        };
        for record in entity.kind.records() {
            let (id, attributes) = cx.record_attributes(&entity.kind, record)?;
            if matches!(entity.kind, EntityKind::Simple(_)) {
                primary = Some(id);
            }
            if record.parameters.len() != attributes.len() {
                return Err(cx.error(
                    ErrorKind::AttributeCount,
                    "physical parameter count differs from explicit attribute count",
                ));
            }
            for ((owner, attribute), value) in attributes.into_iter().zip(&record.parameters) {
                cx.attribute = Some(attribute.name);
                cx.source = Some(value.source);
                let link = cx.is_link(&resolved, &types, owner, attribute)?;
                let mut placeholder = false;
                for slot in omitted {
                    cx.tick()?;
                    if slot.attribute == attribute.name {
                        for index in 0..cx.types[&entity.id].len() {
                            cx.tick()?;
                            if cx.types[&entity.id][index] == slot.entity {
                                placeholder = true;
                            }
                        }
                    }
                }
                if placeholder {
                    if !matches!(value.kind, ValueKind::Omitted) {
                        return Err(cx.error(
                            ErrorKind::TypeMismatch,
                            "physical-profile slot requires a derived marker",
                        ));
                    }
                } else if link {
                    cx.link(value, attribute.optional)?;
                } else {
                    cx.value(&attribute.domain, value, attribute.optional, 0)?;
                }
                views.push(AttributeView {
                    owner,
                    declaration: attribute,
                    value,
                });
            }
            cx.attribute = None;
        }
        by_id.insert(entity.id, entities.len());
        for _ in 0..cx.types[&entity.id].len() {
            cx.tick()?;
        }
        let types = cx.types[&entity.id].clone();
        entities.push(EntityView {
            id: entity.id,
            declaration: primary,
            types,
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
    types: BTreeMap<EntityId, Vec<DeclarationId>>,
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
                if unique || kind == AggregateKind::Set {
                    for (index, item) in values.iter().enumerate() {
                        // Unset OPTIONAL ARRAY elements have no known value to duplicate.
                        if matches!(item.kind, ValueKind::Null) {
                            continue;
                        }
                        for previous in &values[..index] {
                            if self.equal(element, previous, item, depth + 1)? {
                                self.source = Some(item.source);
                                return Err(self.error(
                                    ErrorKind::DuplicateValue,
                                    "aggregate requires unique values",
                                ));
                            }
                        }
                    }
                }
                Ok(())
            }
            Domain::Select(alternatives) => self.select_value(alternatives, value, depth + 1),
        }
    }
    fn reference(&mut self, expected: DeclarationId, value: &StepValue) -> Result<(), Error> {
        let ValueKind::Reference(target) = value.kind else {
            return Err(self.error(ErrorKind::TypeMismatch, "expected entity reference"));
        };
        let Some(actual) = self.types.get(&target) else {
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
        for index in 0..actual.len() {
            self.tick()?;
            if self.types[&target][index] == expected {
                return Ok(());
            }
        }
        Err(self.error(
            ErrorKind::ReferenceType,
            "target is not assignable to entity domain",
        ))
    }
    fn link(&mut self, value: &StepValue, optional: bool) -> Result<(), Error> {
        self.tick()?;
        match value.kind {
            ValueKind::Null if optional => Ok(()),
            ValueKind::Null => Err(self.error(ErrorKind::RequiredValue, "required value is unset")),
            ValueKind::Reference(target) if self.document.entities().get(target).is_some() => {
                Ok(())
            }
            ValueKind::Reference(_) => Err(self.error(
                ErrorKind::MissingReference,
                "profile link target is missing",
            )),
            _ => Err(self.error(
                ErrorKind::TypeMismatch,
                "profile link requires an entity reference",
            )),
        }
    }
}
