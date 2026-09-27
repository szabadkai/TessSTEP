//! Product structure: link imported shape roots to product definitions and
//! occurrence placements, and place their meshes in an assembly scene.
//!
//! Product records are decoded with the bundled original product profile
//! (`corpus/geometry/product.exp`) and adapted by `tessstep-ap242`; representation
//! items are retained as links, never decoded, so any geometry may sit behind them.
//! Placements come only from explicit occurrence relationships; a missing or
//! ambiguous placement fails, and no placement is ever defaulted to identity.
use super::*;
use std::collections::BTreeSet;
use std::sync::Arc;
use tessstep_mesh::Mesh;
use tessstep_mesh::scene::{Asset, AssetId, Instance, InstanceId, Scene};
use tessstep_product::{ExpansionLimits, Model, RepresentationId};

/// An imported shape root: its mesh in metres, in its representation's coordinates.
#[derive(Clone, Debug)]
pub struct ShapeAsset {
    pub root: EntityId,
    pub mesh: Arc<Mesh>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AssemblyOptions {
    pub import: ImportOptions,
    pub expansion: ExpansionLimits,
    pub scene: tessstep_mesh::scene::Limits,
}

/// A product record left out because its closure is outside the product profile.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcludedRecord {
    pub record: EntityId,
    /// An occurrence or its placement: without it the assembly structure is
    /// incomplete. Other records (shape definitions and untransformed shape
    /// relationships) only leave their representations unlinked.
    pub structural: bool,
    pub error: Error,
}

/// One scene instance and the product records it stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssemblyNode {
    pub instance: InstanceId,
    /// The PRODUCT_DEFINITION placed by this node, or owning this asset leaf.
    pub definition: EntityId,
    /// The NEXT_ASSEMBLY_USAGE_OCCURRENCE that places a definition node; None for a
    /// root definition and for asset leaves.
    pub occurrence: Option<EntityId>,
    /// The shape representation whose coordinates the node's children use.
    pub representation: EntityId,
    /// The shape root of an asset leaf (its asset ID is the root's entity ID).
    pub root: Option<EntityId>,
}

/// A product definition and the product it defines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductInfo {
    pub definition: EntityId,
    pub product: EntityId,
    /// PRODUCT.id and PRODUCT.name as written.
    pub id: String,
    pub name: String,
}

/// A product structure linked to imported shapes.
#[derive(Clone, Debug)]
pub struct LinkedAssembly {
    scene: Arc<Scene>,
    nodes: Vec<AssemblyNode>,
    products: Vec<ProductInfo>,
    occurrences: usize,
    unplaced: Vec<EntityId>,
    unimported: Vec<EntityId>,
    excluded: Vec<ExcludedRecord>,
    /// Discovered shape roots by containing representation.
    pub(crate) held: BTreeMap<EntityId, Vec<EntityId>>,
}
impl LinkedAssembly {
    /// Assets are every supplied shape, with `AssetId` equal to its root entity ID.
    /// Definition nodes carry their occurrence placement; asset leaves are identity
    /// children of the definition whose shape representation holds the root.
    pub fn scene(&self) -> &Arc<Scene> {
        &self.scene
    }
    /// One entry per scene instance, in scene instance order.
    pub fn nodes(&self) -> &[AssemblyNode] {
        &self.nodes
    }
    pub fn products(&self) -> &[ProductInfo] {
        &self.products
    }
    /// NEXT_ASSEMBLY_USAGE_OCCURRENCE records in the product structure.
    pub fn occurrence_count(&self) -> usize {
        self.occurrences
    }
    /// Supplied shapes that no product definition's shape representation holds.
    pub fn unplaced(&self) -> &[EntityId] {
        &self.unplaced
    }
    /// Discovered roots in placed representations for which no shape was supplied.
    pub fn unimported(&self) -> &[EntityId] {
        &self.unimported
    }
    /// Product records left out because their closure is outside the product
    /// profile, with the typed reason; the remaining structure was linked without them.
    pub fn excluded(&self) -> &[ExcludedRecord] {
        &self.excluded
    }
    /// Whether every occurrence and occurrence placement was linked.
    pub fn is_complete(&self) -> bool {
        !self.excluded.iter().any(|e| e.structural)
    }
}

/// Link supplied shapes to the document's product structure and build a scene.
/// Shapes are keyed by their root entity (MANIFOLD_SOLID_BREP, BREP_WITH_VOIDS,
/// FACETED_BREP or a shape tessellation root); each is placed wherever a product
/// definition's shape representation, or a representation associated with it by an
/// untransformed shape representation relationship in the same context, holds it.
pub fn link_assembly(
    document: &Document,
    shapes: &[ShapeAsset],
    options: AssemblyOptions,
) -> Result<LinkedAssembly, Error> {
    let solids = discover_solids(document, options.import)?;
    let tessellations = discover_tessellations(document, options.import)?;
    let mut held: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    let roots = solids
        .iter()
        .map(|s| (s.entity, &s.representations))
        .chain(tessellations.iter().map(|t| (t.entity, &t.representations)));
    for (root, representations) in roots {
        for &r in representations {
            held.entry(r).or_default().push(root);
        }
    }
    link(document, shapes, &held, options)
}

pub(crate) fn link(
    document: &Document,
    shapes: &[ShapeAsset],
    held: &BTreeMap<EntityId, Vec<EntityId>>,
    options: AssemblyOptions,
) -> Result<LinkedAssembly, Error> {
    let mut work = options.import.max_work;
    let mut charge = |n: usize| -> Result<(), Error> {
        work = work.checked_sub(n).ok_or_else(|| {
            product_error(
                ErrorKind::ResourceLimit,
                None,
                &format!(
                    "assembly work budget exceeded (max_work {})",
                    options.import.max_work
                ),
            )
        })?;
        Ok(())
    };
    let mut meshes: BTreeMap<EntityId, &ShapeAsset> = BTreeMap::new();
    for shape in shapes {
        charge(1)?;
        if meshes.insert(shape.root, shape).is_some() {
            return Err(product_error(
                ErrorKind::InvalidOptions,
                Some(shape.root),
                "duplicate shape root",
            ));
        }
    }
    let roots = product_roots(document, &mut charge)?;
    let (decoded, excluded) = decode_product(document, &roots, options.import)?;
    let model = match &decoded {
        Some(decoded) => Some(
            tessstep_ap242::adapt(
                decoded,
                "tessstep_product",
                tessstep_ap242::Limits {
                    max_work: options.import.max_work,
                    ..tessstep_ap242::Limits::default()
                },
            )
            .map_err(|e| adapter_error(document, e))?,
        ),
        None => None,
    };
    let entity = |id: u64| EntityId::new(id).expect("adapter identities are STEP instance IDs");
    let mut nodes = Vec::new();
    let mut instances = Vec::new();
    let mut placed = BTreeSet::new();
    let mut unimported = BTreeSet::new();
    let mut products = Vec::new();
    let mut occurrences = 0;
    if let Some(model) = &model {
        let parts = model.parts();
        occurrences = parts.occurrences.len();
        for d in &parts.definitions {
            charge(1)?;
            let formation = parts.formations.iter().find(|f| f.id == d.formation);
            let product = formation.and_then(|f| parts.products.iter().find(|p| p.id == f.product));
            if let Some(p) = product {
                products.push(ProductInfo {
                    definition: entity(d.id.0),
                    product: entity(p.id.0),
                    id: p.identifier.clone(),
                    name: p.name.clone(),
                });
            }
        }
        let same_frame = associations(model, &mut charge)?;
        for &root in model.roots() {
            charge(1)?;
            let Some(representations) = model.representations_of(root) else {
                continue;
            };
            let mut expansion = None;
            let mut first_error = None;
            for &r in representations {
                charge(1)?;
                match model.expand(root, r, &BTreeMap::new(), options.expansion) {
                    Ok(found) => {
                        expansion = Some(found);
                        break;
                    }
                    Err(e) => {
                        first_error.get_or_insert(e);
                    }
                }
            }
            let Some(expansion) = expansion else {
                let e = first_error.expect("a definition with representations was tried");
                let kind = if e == tessstep_product::Error::ResourceLimit {
                    ErrorKind::ResourceLimit
                } else {
                    ErrorKind::InvalidGeometry
                };
                return Err(Error {
                    message: format!("assembly expansion: {e}"),
                    ..product_error(kind, Some(entity(root.0)), "")
                });
            };
            let base = instances.len();
            for inst in &expansion {
                charge(1)?;
                let id = InstanceId(instances.len() as u64 + 1);
                let parent = inst.parent.map(|p| InstanceId(base as u64 + p as u64 + 1));
                // Expansion lists parents before their children; asset leaves follow.
                instances.push(Instance {
                    id,
                    parent,
                    asset: None,
                    local_transform: inst.local_to_parent,
                });
                nodes.push(AssemblyNode {
                    instance: id,
                    definition: entity(inst.definition.0),
                    occurrence: inst.occurrence.map(|o| entity(o.0)),
                    representation: entity(inst.representation.0),
                    root: None,
                });
            }
            for (index, inst) in expansion.iter().enumerate() {
                let frame = same_frame
                    .get(&inst.representation)
                    .cloned()
                    .unwrap_or_else(|| BTreeSet::from([inst.representation]));
                for r in frame {
                    charge(1)?;
                    for &root in held.get(&entity(r.0)).map_or(&[][..], Vec::as_slice) {
                        charge(1)?;
                        if !meshes.contains_key(&root) {
                            unimported.insert(root);
                            continue;
                        }
                        placed.insert(root);
                        let id = InstanceId(instances.len() as u64 + 1);
                        instances.push(Instance {
                            id,
                            parent: Some(InstanceId(base as u64 + index as u64 + 1)),
                            asset: Some(AssetId(root.get())),
                            local_transform: tessstep_mesh::scene::Transform::identity(),
                        });
                        nodes.push(AssemblyNode {
                            instance: id,
                            definition: entity(inst.definition.0),
                            occurrence: None,
                            representation: entity(r.0),
                            root: Some(root),
                        });
                    }
                }
            }
        }
    }
    let assets = shapes
        .iter()
        .map(|s| Asset {
            id: AssetId(s.root.get()),
            mesh: s.mesh.clone(),
        })
        .collect();
    let scene = Scene::new(assets, instances, options.scene).map_err(|e| Error {
        message: e.to_string(),
        ..product_error(
            if e == tessstep_mesh::scene::Error::ResourceLimit {
                ErrorKind::ResourceLimit
            } else {
                ErrorKind::InvalidGeometry
            },
            None,
            "",
        )
    })?;
    Ok(LinkedAssembly {
        scene: Arc::new(scene),
        nodes,
        products,
        occurrences,
        unplaced: shapes
            .iter()
            .map(|s| s.root)
            .filter(|r| !placed.contains(r))
            .collect(),
        unimported: unimported.into_iter().collect(),
        excluded,
        held: held.clone(),
    })
}

/// Representations sharing a frame: those related by an untransformed shape
/// representation relationship in the same context, transitively.
fn associations(
    model: &Model,
    charge: &mut impl FnMut(usize) -> Result<(), Error>,
) -> Result<BTreeMap<RepresentationId, BTreeSet<RepresentationId>>, Error> {
    let parts = model.parts();
    let context: BTreeMap<RepresentationId, _> = parts
        .representations
        .iter()
        .map(|r| (r.id, r.context))
        .collect();
    let mut next: BTreeMap<RepresentationId, Vec<RepresentationId>> = BTreeMap::new();
    for id in &parts.associations {
        charge(1)?;
        let Some(r) = parts.relationships.iter().find(|r| r.id == *id) else {
            continue;
        };
        if context.get(&r.rep_1) == context.get(&r.rep_2) {
            next.entry(r.rep_1).or_default().push(r.rep_2);
            next.entry(r.rep_2).or_default().push(r.rep_1);
        }
    }
    let mut frames = BTreeMap::new();
    for &start in next.keys() {
        let mut found = BTreeSet::new();
        let mut stack = vec![start];
        while let Some(r) = stack.pop() {
            charge(1)?;
            if found.insert(r) {
                stack.extend(next.get(&r).into_iter().flatten().copied());
            }
        }
        frames.insert(start, found);
    }
    Ok(frames)
}

/// Product records selected by physical record name: occurrences, context dependent
/// shape representations, shape definition representations of product definition
/// shapes, and untransformed shape representation relationships.
fn product_roots(
    document: &Document,
    charge: &mut impl FnMut(usize) -> Result<(), Error>,
) -> Result<Vec<EntityId>, Error> {
    let named = |e: &tessstep_part21::EntityInstance, name: &str| {
        e.kind
            .records()
            .iter()
            .any(|r| r.name.as_ref().eq_ignore_ascii_case(name))
    };
    let mut roots = Vec::new();
    for e in document.entities().iter() {
        charge(1)?;
        let records = e.kind.records();
        let simple =
            |name: &str| matches!(records, [r] if r.name.as_ref().eq_ignore_ascii_case(name));
        let selected = if named(e, "NEXT_ASSEMBLY_USAGE_OCCURRENCE")
            || named(e, "CONTEXT_DEPENDENT_SHAPE_REPRESENTATION")
            || simple("SHAPE_REPRESENTATION_RELATIONSHIP")
        {
            true
        } else if simple("SHAPE_DEFINITION_REPRESENTATION") {
            // Only shapes of product definitions and occurrences; property and shape
            // aspect representations are not product structure.
            match records[0].parameters.first().map(|v| &v.kind) {
                Some(ValueKind::Reference(id)) => document
                    .entities()
                    .get(*id)
                    .is_some_and(|t| named(t, "PRODUCT_DEFINITION_SHAPE")),
                _ => false,
            }
        } else {
            false
        };
        if selected {
            roots.push(e.id);
        }
    }
    Ok(roots)
}

/// Decode the product records' closure. When the combined closure leaves the
/// profile, each record is decoded alone and those outside it are excluded with
/// their typed reason.
fn decode_product<'a>(
    document: &'a Document,
    roots: &[EntityId],
    options: ImportOptions,
) -> Result<(Option<DecodedDocument<'a>>, Vec<ExcludedRecord>), Error> {
    if roots.is_empty() {
        return Ok((None, Vec::new()));
    }
    let decode = |roots: &[EntityId]| decode_with_profile(document, roots, options);
    match decode(roots) {
        Ok(decoded) => Ok((Some(decoded), Vec::new())),
        Err(e) if e.kind == decode::ErrorKind::ResourceLimit => {
            Err(product_decode_error(document, e))
        }
        Err(_) => {
            let mut kept = Vec::new();
            let mut excluded = Vec::new();
            for &root in roots {
                match decode(&[root]) {
                    Ok(_) => kept.push(root),
                    Err(e) if e.kind == decode::ErrorKind::ResourceLimit => {
                        return Err(product_decode_error(document, e));
                    }
                    Err(e) => {
                        let structural = document.entities().get(root).is_some_and(|r| {
                            r.kind.records().iter().any(|r| {
                                let name = r.name.as_ref();
                                name.eq_ignore_ascii_case("NEXT_ASSEMBLY_USAGE_OCCURRENCE")
                                    || name.eq_ignore_ascii_case(
                                        "CONTEXT_DEPENDENT_SHAPE_REPRESENTATION",
                                    )
                            })
                        });
                        let e = product_decode_error(document, e);
                        excluded.push(ExcludedRecord {
                            record: root,
                            structural,
                            error: Error {
                                message: format!("product record #{}: {}", root.get(), e.message),
                                ..e
                            },
                        });
                    }
                }
            }
            if kept.is_empty() {
                return Ok((None, excluded));
            }
            let decoded = decode(&kept).map_err(|e| product_decode_error(document, e))?;
            Ok((Some(decoded), excluded))
        }
    }
}

fn decode_with_profile<'a>(
    document: &'a Document,
    roots: &[EntityId],
    options: ImportOptions,
) -> Result<DecodedDocument<'a>, decode::Error> {
    use product_profile::schema_tessstep_product as s;
    use tessstep_schema::EntityBinding;
    decode::decode_reachable_profile_with_links(
        document,
        &product_profile::SCHEMA_SET,
        "tessstep_product",
        roots,
        &[
            crate::discovery::unit_slots(
                s::Entity_SI_UNIT::DECLARATION,
                s::Entity_CONVERSION_BASED_UNIT::DECLARATION,
                options,
            ),
            vec![
                decode::OmittedSlot {
                    entity: s::Entity_CHARACTERIZED_OBJECT::DECLARATION,
                    attribute: "OBJECT_NAME",
                    allow_unset: false,
                    allow_value: !options.strict,
                },
                decode::OmittedSlot {
                    entity: s::Entity_CHARACTERIZED_OBJECT::DECLARATION,
                    attribute: "OBJECT_DESCRIPTION",
                    allow_unset: false,
                    allow_value: !options.strict,
                },
            ],
        ]
        .concat(),
        &[
            decode::LinkSlot {
                entity: s::Entity_REPRESENTATION::DECLARATION,
                attribute: "ITEMS",
            },
            decode::LinkSlot {
                entity: s::Entity_PRODUCT_DEFINITION_WITH_ASSOCIATED_DOCUMENTS::DECLARATION,
                attribute: "DOCUMENTATION_IDS",
            },
        ],
        decode::Limits {
            max_work: options.max_work,
            ..decode::Limits::default()
        },
    )
}

fn product_error(kind: ErrorKind, entity: Option<EntityId>, message: &str) -> Error {
    Error {
        kind,
        stage: Stage::Product,
        entity,
        source: None,
        message: message.into(),
    }
}

fn product_decode_error(document: &Document, e: decode::Error) -> Error {
    Error {
        kind: match e.kind {
            decode::ErrorKind::ResourceLimit => ErrorKind::ResourceLimit,
            decode::ErrorKind::MissingReference => ErrorKind::MissingEntity,
            decode::ErrorKind::UnknownEntity | decode::ErrorKind::Unsupported => {
                ErrorKind::Unsupported
            }
            _ => ErrorKind::InvalidGeometry,
        },
        stage: Stage::Product,
        entity: e.entity,
        source: e.source,
        message: if e.kind == decode::ErrorKind::UnknownEntity {
            outside_profile(
                document,
                &product_profile::SCHEMA_SET,
                "tessstep_product",
                e.entity,
            )
        } else {
            e.to_string()
        },
    }
}

fn adapter_error(document: &Document, e: tessstep_ap242::Error) -> Error {
    use tessstep_ap242::ErrorKind as A;
    let mut message = e.message.to_string();
    if let Some(g) = e.graph_error {
        message = format!("{message}: {g}");
    }
    if let Some(m) = e.math_error {
        message = format!("{message}: {m:?}");
    }
    Error {
        kind: match e.kind {
            A::ResourceLimit => ErrorKind::ResourceLimit,
            A::Unsupported | A::Schema => ErrorKind::Unsupported,
            A::MissingAttribute => ErrorKind::MissingEntity,
            A::TypeMismatch | A::Units | A::Placement | A::Graph => ErrorKind::InvalidGeometry,
        },
        stage: Stage::Product,
        entity: e.entity,
        source: e.source.or_else(|| {
            e.entity
                .and_then(|id| document.entities().get(id).map(|r| r.source))
        }),
        message,
    }
}

/// The profile a shape root was imported with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeRootKind {
    Solid(SolidKind),
    Tessellated(TessellatedKind),
}

/// The outcome of importing one discovered shape root.
#[derive(Clone, Debug, PartialEq)]
pub struct RootImport {
    pub root: EntityId,
    pub kind: ShapeRootKind,
    /// Whether representation selection chose this root for its shape. An
    /// alternative is imported only when every earlier member of its group failed.
    pub selected: bool,
    pub result: Result<(), Error>,
}

#[derive(Clone, Copy, Debug)]
pub struct AssemblyImportOptions {
    pub assembly: AssemblyOptions,
    pub preference: RepresentationPreference,
    /// Chord and angle tolerance for every B-rep root, in metres and radians.
    pub tolerance: TessellationTolerance,
    pub tessellation: TessellationOptions,
    /// Each root's model tolerance is its context's declared length uncertainty,
    /// never below this many metres; it is also used when none is declared.
    pub minimum_model_tolerance: f64,
    pub mesh: tessstep_mesh::Limits,
}
impl Default for AssemblyImportOptions {
    fn default() -> Self {
        Self {
            assembly: AssemblyOptions::default(),
            preference: RepresentationPreference::Exact,
            tolerance: TessellationTolerance::new(
                tessstep_math::Length::metres(1e-4).expect("finite chord"),
                tessstep_math::Angle::radians(0.1).expect("finite angle"),
            )
            .expect("valid tolerance"),
            tessellation: TessellationOptions::default(),
            minimum_model_tolerance: 1e-7,
            mesh: tessstep_mesh::Limits::default(),
        }
    }
}

/// A linked assembly and the import outcome of every discovered shape root.
#[derive(Clone, Debug)]
pub struct ImportedAssembly {
    pub assembly: LinkedAssembly,
    pub roots: Vec<RootImport>,
}

/// Discover every shape root, select one representation per shape, import it with
/// the units of its representation context, and link the meshes to the product
/// structure. A root whose context declares no units is not imported: no unit is
/// assumed. Failed roots are reported, not placed; their instances are listed as
/// unimported.
pub fn import_assembly(
    document: &Document,
    options: AssemblyImportOptions,
) -> Result<ImportedAssembly, Error> {
    let import = options.assembly.import;
    if !options.minimum_model_tolerance.is_finite() || options.minimum_model_tolerance <= 0. {
        return Err(Error {
            kind: ErrorKind::InvalidOptions,
            stage: Stage::Profile,
            entity: None,
            source: None,
            message: "minimum model tolerance must be positive and finite".into(),
        });
    }
    let solids = discover_solids(document, import)?;
    let tessellations = discover_tessellations(document, import)?;
    let choices = select_representations(
        document,
        &solids,
        &tessellations,
        options.preference,
        import,
    )?;
    let mut shapes = Vec::new();
    let mut roots = Vec::new();
    for choice in &choices {
        let members = std::iter::once(choice.selected).chain(choice.alternatives.iter().copied());
        for (k, member) in members.enumerate() {
            let (root, kind, units) = match member {
                RootRef::Solid(i) => (
                    solids[i].entity,
                    ShapeRootKind::Solid(solids[i].kind),
                    &solids[i].units,
                ),
                RootRef::Tessellation(i) => (
                    tessellations[i].entity,
                    ShapeRootKind::Tessellated(tessellations[i].kind),
                    &tessellations[i].units,
                ),
            };
            let mesh = units
                .clone()
                .and_then(|u| import_root(document, root, kind, u, options));
            let ok = mesh.is_ok();
            roots.push(RootImport {
                root,
                kind,
                selected: k == 0,
                result: mesh.as_ref().map(|_| ()).map_err(Clone::clone),
            });
            if let Ok(mesh) = mesh {
                shapes.push(ShapeAsset {
                    root,
                    mesh: Arc::new(mesh),
                });
            }
            if ok {
                break;
            }
        }
    }
    let mut held: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    let found = solids
        .iter()
        .map(|s| (s.entity, &s.representations))
        .chain(tessellations.iter().map(|t| (t.entity, &t.representations)));
    for (root, representations) in found {
        for &r in representations {
            held.entry(r).or_default().push(root);
        }
    }
    let assembly = link(document, &shapes, &held, options.assembly)?;
    Ok(ImportedAssembly { assembly, roots })
}

fn import_root(
    document: &Document,
    root: EntityId,
    kind: ShapeRootKind,
    units: ContextUnits,
    options: AssemblyImportOptions,
) -> Result<Mesh, Error> {
    let import = options.assembly.import;
    match kind {
        ShapeRootKind::Tessellated(_) => {
            import_tessellated(document, root, units.length, import, options.mesh)
                .map(ImportedTessellation::into_mesh)
        }
        ShapeRootKind::Solid(solid) => {
            let distance = units
                .distance_uncertainty
                .unwrap_or(options.minimum_model_tolerance)
                .max(options.minimum_model_tolerance);
            let invalid = |message: &str| Error {
                kind: ErrorKind::InvalidOptions,
                stage: Stage::Profile,
                entity: Some(root),
                source: None,
                message: message.into(),
            };
            let model = ModelTolerance::new(
                tessstep_math::Length::metres(distance)
                    .map_err(|_| invalid("model tolerance out of range"))?,
                tessstep_math::Angle::radians(1e-8).expect("finite angle"),
            )
            .map_err(|_| invalid("model tolerance out of range"))?;
            let imported = if solid == SolidKind::FacetedBrep {
                import_faceted_solid(document, root, units.length, model, import)?
            } else {
                import_brep_solid(
                    document,
                    root,
                    units.length,
                    units.plane_angle,
                    model,
                    import,
                )?
            };
            imported.tessellate(options.tolerance, options.tessellation)
        }
    }
}
