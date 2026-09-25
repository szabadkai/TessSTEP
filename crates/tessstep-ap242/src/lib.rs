//! Explicit adapters for a shared STEP product/representation subset.
//!
//! The crate name reserves the AP242 adapter boundary; it is not a claim of
//! AP242 schema conformance. Callers supply structurally decoded metadata.
//! Geometry items and placement pairs remain opaque, ordered descriptions.
#![forbid(unsafe_code)]

mod units;
use std::collections::BTreeMap;
use tessstep_model::decode::{DecodedDocument, EntityView};
use tessstep_part21::{EntityId, SourceSpan, StepValue, ValueKind};
use tessstep_product::*;
use tessstep_schema::DeclarationId;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_work: usize,
    pub max_text_bytes: usize,
    pub max_unit_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 5_000_000,
            max_text_bytes: 16_000_000,
            max_unit_depth: 128,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Schema,
    MissingAttribute,
    TypeMismatch,
    Unsupported,
    Units,
    Graph,
    ResourceLimit,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub entity: Option<EntityId>,
    pub attribute: Option<&'static str>,
    pub source: Option<SourceSpan>,
    pub message: &'static str,
    pub graph_error: Option<tessstep_product::Error>,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}

const ROLES: &[&str] = &[
    "product",
    "product_definition_formation",
    "product_definition",
    "product_definition_shape",
    "product_definition_relationship",
    "next_assembly_usage_occurrence",
    "shape_definition_representation",
    "representation",
    "geometric_representation_context",
    "global_unit_assigned_context",
    "representation_relationship",
    "representation_relationship_with_transformation",
    "context_dependent_shape_representation",
    "item_defined_transformation",
    "representation_item",
    "representation_map",
    "mapped_item",
    "si_unit",
    "conversion_based_unit",
    "conversion_based_unit_with_offset",
    "length_unit",
    "plane_angle_unit",
    "solid_angle_unit",
    "measure_with_unit",
    "dimensional_exponents",
];

/// Adapt by resolved declaration identity and named attributes, never physical
/// parameter offsets or an ADVANCED_FACE scan. No schema rules are suppressed.
/// All semantic output is owned; failure publishes no partial model.
pub fn adapt(
    decoded: &DecodedDocument<'_>,
    schema_name: &str,
    limits: Limits,
) -> Result<Model, Error> {
    let mut cx = Context {
        decoded,
        roles: BTreeMap::new(),
        limits,
        work: 0,
        text_bytes: 0,
        entity: None,
        attribute: None,
        source: None,
    };
    let mut selected = None;
    for s in decoded.schemas().schemas {
        cx.tick()?;
        if s.name.eq_ignore_ascii_case(schema_name) {
            selected = Some(s);
            break;
        }
    }
    let schema =
        selected.ok_or_else(|| cx.error(ErrorKind::Schema, "adapter schema not supplied"))?;
    for &role in ROLES {
        for symbol in schema.symbols {
            cx.tick()?;
            if symbol.name.eq_ignore_ascii_case(role)
                && cx.roles.insert(role, symbol.declaration).is_some()
            {
                return Err(cx.error(ErrorKind::Schema, "ambiguous role declaration"));
            }
        }
    }
    let mut parts = Parts::default();
    let mut contexts = BTreeMap::new();
    for view in decoded.entities() {
        cx.set_entity(view.id);
        cx.tick()?;
        let id = view.id.get();
        if cx.is(view, "product")? {
            parts.products.push(Product {
                id: ProductId(id),
                identifier: cx.string(view, "id")?,
                name: cx.string(view, "name")?,
            });
        }
        if cx.is(view, "product_definition_formation")? {
            let product = cx.target(view, "of_product", "product")?;
            parts.formations.push(Formation {
                id: FormationId(id),
                product: ProductId(product.id.get()),
                identifier: cx.string(view, "id")?,
            });
        }
        if cx.is(view, "product_definition")? {
            let formation = cx.target(view, "formation", "product_definition_formation")?;
            parts.definitions.push(Definition {
                id: DefinitionId(id),
                formation: FormationId(formation.id.get()),
                identifier: cx.string(view, "id")?,
            });
        }
        if cx.is(view, "product_definition_relationship")? {
            if !cx.is(view, "next_assembly_usage_occurrence")? {
                return Err(cx.error(
                    ErrorKind::Unsupported,
                    "only next assembly usage occurrences are interpreted",
                ));
            }
            let parent = cx.target(view, "relating_product_definition", "product_definition")?;
            let child = cx.target(view, "related_product_definition", "product_definition")?;
            parts.occurrences.push(Occurrence {
                id: OccurrenceId(id),
                identifier: cx.string(view, "id")?,
                parent: DefinitionId(parent.id.get()),
                child: DefinitionId(child.id.get()),
            });
        }
        if cx.is(view, "product_definition_shape")? {
            let target = cx.reference(view, "definition")?;
            let target = if cx.is(target, "product_definition")? {
                ShapeTarget::Definition(DefinitionId(target.id.get()))
            } else if cx.is(target, "next_assembly_usage_occurrence")? {
                ShapeTarget::Occurrence(OccurrenceId(target.id.get()))
            } else {
                return Err(cx.error(ErrorKind::Unsupported, "shape characterization target"));
            };
            parts.shapes.push(Shape {
                id: ShapeId(id),
                target,
            });
        }
        if cx.is(view, "shape_definition_representation")? {
            let shape = cx.target(view, "definition", "product_definition_shape")?;
            let rep = cx.target(view, "used_representation", "representation")?;
            parts.shape_bindings.push(ShapeBinding {
                shape: ShapeId(shape.id.get()),
                representation: RepresentationId(rep.id.get()),
            });
        }
        if cx.is(view, "representation")? {
            let context = cx.reference(view, "context_of_items")?;
            let context_id = ContextId(context.id.get());
            if let std::collections::btree_map::Entry::Vacant(e) = contexts.entry(context_id) {
                let units = cx.context_units(context)?;
                e.insert(parts.contexts.len());
                parts.contexts.push(tessstep_product::Context {
                    id: context_id,
                    units,
                });
            }
            let values = cx.aggregate(view, "items")?;
            let mut items = Vec::new();
            for value in values {
                cx.tick()?;
                let item = cx.ref_value(value)?;
                cx.expect(item, "representation_item")?;
                items.push(ItemId(item.id.get()));
            }
            parts.representations.push(Representation {
                id: RepresentationId(id),
                name: cx.string(view, "name")?,
                context: context_id,
                items,
            });
        }
        if cx.is(view, "representation_relationship")? {
            let a = cx.target(view, "rep_1", "representation")?;
            let b = cx.target(view, "rep_2", "representation")?;
            let transform = if cx.is(view, "representation_relationship_with_transformation")? {
                let transform = cx.target(
                    view,
                    "transformation_operator",
                    "item_defined_transformation",
                )?;
                let item_1 = cx.target(transform, "transform_item_1", "representation_item")?;
                let item_2 = cx.target(transform, "transform_item_2", "representation_item")?;
                Some(ItemTransform {
                    item_1: ItemId(item_1.id.get()),
                    item_2: ItemId(item_2.id.get()),
                })
            } else {
                None
            };
            parts.relationships.push(Relationship {
                id: RelationshipId(id),
                rep_1: RepresentationId(a.id.get()),
                rep_2: RepresentationId(b.id.get()),
                transform,
            });
        }
        if cx.is(view, "context_dependent_shape_representation")? {
            let rel = cx.target(
                view,
                "representation_relation",
                "representation_relationship",
            )?;
            let shape = cx.target(
                view,
                "represented_product_relation",
                "product_definition_shape",
            )?;
            let occurrence = cx.target(shape, "definition", "next_assembly_usage_occurrence")?;
            parts.placements.push(OccurrencePlacement {
                occurrence: OccurrenceId(occurrence.id.get()),
                relationship: RelationshipId(rel.id.get()),
            });
        }
        if cx.is(view, "representation_map")? {
            let rep = cx.target(view, "mapped_representation", "representation")?;
            let origin = cx.target(view, "mapping_origin", "representation_item")?;
            parts.maps.push(RepresentationMap {
                id: MapId(id),
                representation: RepresentationId(rep.id.get()),
                origin: ItemId(origin.id.get()),
            });
        }
        if cx.is(view, "mapped_item")? {
            let map = cx.target(view, "mapping_source", "representation_map")?;
            let target = cx.target(view, "mapping_target", "representation_item")?;
            parts.mapped_items.push(MappedItem {
                id: ItemId(id),
                map: MapId(map.id.get()),
                target: ItemId(target.id.get()),
            });
        }
    }
    // Global graph errors have no invented physical owner.
    cx.entity = None;
    cx.attribute = None;
    cx.source = None;
    Model::new(parts, limits.max_work.saturating_sub(cx.work)).map_err(|graph_error| Error {
        graph_error: Some(graph_error),
        ..cx.error(
            if graph_error == tessstep_product::Error::ResourceLimit {
                ErrorKind::ResourceLimit
            } else {
                ErrorKind::Graph
            },
            "invalid product graph",
        )
    })
}

struct Context<'d, 'a> {
    decoded: &'d DecodedDocument<'a>,
    roles: BTreeMap<&'static str, DeclarationId>,
    limits: Limits,
    work: usize,
    text_bytes: usize,
    entity: Option<EntityId>,
    attribute: Option<&'static str>,
    source: Option<SourceSpan>,
}
impl<'d, 'a> Context<'d, 'a> {
    fn error(&self, kind: ErrorKind, message: &'static str) -> Error {
        Error {
            kind,
            entity: self.entity,
            attribute: self.attribute,
            source: self.source,
            message,
            graph_error: None,
        }
    }
    fn tick(&mut self) -> Result<(), Error> {
        if self.work >= self.limits.max_work {
            return Err(self.error(ErrorKind::ResourceLimit, "product adapter work budget"));
        }
        self.work += 1;
        Ok(())
    }
    fn set_entity(&mut self, id: EntityId) {
        self.entity = Some(id);
        self.attribute = None;
        self.source = self.decoded.document().entities().get(id).map(|e| e.source);
    }
    fn is(&mut self, view: &EntityView<'_>, role: &str) -> Result<bool, Error> {
        self.tick()?;
        let Some(&id) = self.roles.get(role) else {
            return Ok(false);
        };
        for &member in &view.types {
            self.tick()?;
            if member == id {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn expect(&mut self, view: &EntityView<'_>, role: &str) -> Result<(), Error> {
        if self.is(view, role)? {
            Ok(())
        } else {
            Err(self.error(
                ErrorKind::Unsupported,
                "referenced entity has no supported semantic role",
            ))
        }
    }
    fn attr(
        &mut self,
        view: &'d EntityView<'a>,
        name: &'static str,
    ) -> Result<&'a StepValue, Error> {
        self.set_entity(view.id);
        self.attribute = Some(name);
        let mut result = None;
        for a in &view.attributes {
            self.tick()?;
            if a.declaration.name.eq_ignore_ascii_case(name) {
                if result.is_some() {
                    return Err(self.error(ErrorKind::Schema, "ambiguous semantic attribute"));
                }
                result = Some(a.value);
            }
        }
        let value = result.ok_or_else(|| {
            self.error(
                ErrorKind::MissingAttribute,
                "semantic attribute absent from supplied schema",
            )
        })?;
        self.source = Some(value.source);
        Ok(value)
    }
    fn untag(&mut self, mut value: &'a StepValue) -> Result<&'a StepValue, Error> {
        // Physical and structural decoding already bound nesting; charge each step.
        while let ValueKind::Typed { value: inner, .. } = &value.kind {
            self.tick()?;
            value = inner;
        }
        Ok(value)
    }
    fn ref_value(&mut self, value: &'a StepValue) -> Result<&'d EntityView<'a>, Error> {
        let value = self.untag(value)?;
        self.tick()?;
        if let ValueKind::Reference(id) = value.kind {
            self.decoded
                .get(id)
                .ok_or_else(|| self.error(ErrorKind::TypeMismatch, "reference is not local"))
        } else {
            Err(self.error(ErrorKind::TypeMismatch, "expected entity reference"))
        }
    }
    fn reference(
        &mut self,
        view: &'d EntityView<'a>,
        name: &'static str,
    ) -> Result<&'d EntityView<'a>, Error> {
        let value = self.attr(view, name)?;
        self.ref_value(value)
    }
    fn target(
        &mut self,
        view: &'d EntityView<'a>,
        name: &'static str,
        role: &str,
    ) -> Result<&'d EntityView<'a>, Error> {
        let result = self.reference(view, name)?;
        self.expect(result, role)?;
        Ok(result)
    }
    fn string(&mut self, view: &'d EntityView<'a>, name: &'static str) -> Result<String, Error> {
        let value = self.attr(view, name)?;
        if let ValueKind::String(s) = &value.kind {
            self.text_bytes = self
                .text_bytes
                .checked_add(s.len())
                .filter(|n| *n <= self.limits.max_text_bytes)
                .ok_or_else(|| self.error(ErrorKind::ResourceLimit, "product text budget"))?;
            Ok(s.to_string())
        } else {
            Err(self.error(ErrorKind::TypeMismatch, "expected text"))
        }
    }
    fn aggregate(
        &mut self,
        view: &'d EntityView<'a>,
        name: &'static str,
    ) -> Result<&'a [StepValue], Error> {
        let value = self.attr(view, name)?;
        if let ValueKind::Aggregate(values) = &value.kind {
            Ok(values)
        } else {
            Err(self.error(ErrorKind::TypeMismatch, "expected aggregate"))
        }
    }
    fn number(&mut self, value: &'a StepValue) -> Result<f64, Error> {
        let value = self.untag(value)?;
        match value.kind {
            ValueKind::Real(n) => Ok(n),
            ValueKind::Integer(n) => Ok(n as f64),
            _ => Err(self.error(ErrorKind::TypeMismatch, "expected real or integer")),
        }
    }
}
