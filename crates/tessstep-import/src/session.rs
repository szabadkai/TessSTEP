//! Document import sessions: one parsed document shared by every root, assembly and
//! appearance import, so that discovery and decoding cost the document's size once.
//!
//! An [`ImportSession`] borrows a parsed [`Document`] and caches what does not belong
//! to a single root: root and unit discovery, representation selection, and profile
//! decodes shared by several roots. [`ImportSession::prepare`] decodes the reference
//! closures of several roots with one profile together, once; each root's import then
//! adapts its records from that shared decode. Every result equals the corresponding
//! free function's. A shared decode is kept only when it succeeds within the per-root
//! work budget (`ImportOptions::max_work`); a root's closure is a subset of the shared
//! one, the structural checks of one record never depend on other roots, and decoding
//! work is additive over the records, so decoding the root alone would succeed with
//! the same records within the same budget. Otherwise every root of the group is
//! decoded alone, exactly as the free functions do. Adapters follow references from
//! their root only, and the curved B-rep adapter scans exactly the root's closure for
//! supplied pcurves, so a superset of records changes nothing they read.
//!
//! Budgets: every decode, adapter and tessellation keeps its per-root budget
//! ([`ImportOptions`], `TessellationOptions`). [`SessionBudgets`] bound what a session
//! accumulates over all of its roots, one budget per stage. A stage is admitted while
//! the session's running total is below its budget, so a total never exceeds its
//! budget by more than one root's share; an exceeded budget is a `ResourceLimit`
//! error naming the budget and its value. Parsing is outside the session: the
//! document's `ParseLimits` bound it. Each discovery pass runs once per session under
//! the per-call `max_work` budget.
//!
//! Ownership: the session borrows the document for `'a`, and its decoded records
//! borrow the document. Discovery results are owned by the session; every import
//! result (solids, tessellations, meshes, presentations, assemblies, appearances) is
//! owned and independent of the session and the document. A shared decode is released
//! once each of its roots has been imported, so a session retains at most the union
//! of its prepared, not yet imported closures; importing a root again decodes it
//! alone. A session caches through interior mutability: it is `Send` but not `Sync`,
//! so threads importing from one document use one session each. No Rust type crosses
//! the C ABI: a C or C++ binding should create a session inside one call (as
//! `import_assembly` does), or keep it in a handle that also retains its document,
//! and expose only owned results.
use super::*;
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeSet;
use tessstep_math::AngleUnit;
use tessstep_mesh::Mesh;
use tessstep_schema::{DeclarationId, DeclarationKind, SchemaSet};

/// The bounded import profiles a session decodes roots with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImportProfile {
    /// `FACETED_BREP` roots, as [`import_faceted_solid`] decodes them.
    Faceted,
    /// Edge-based planar roots, as [`import_planar_solid`] decodes them.
    Planar,
    /// Curved B-rep roots, as [`import_brep_solid`] decodes them.
    Brep,
    /// Existing tessellations and tessellated annotation occurrences, as
    /// [`import_tessellated`] and [`import_presentation`] decode them.
    Tessellated,
}

/// What a session accumulates over all of its roots, one budget per stage, beside the
/// per-root budgets of [`ImportOptions`] and `TessellationOptions` that every root
/// keeps. A stage is admitted while its running total is below the budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionBudgets {
    /// Decode stage: decoded records over every shared and single-root decode.
    pub max_decoded_entities: usize,
    /// Adapter stage: records of the closures handed to adapters, one closure per
    /// import; roots that share records count them once each.
    pub max_adapted_entities: usize,
    /// Tessellation stage: triangles of the meshes the session tessellated or
    /// imported from existing tessellations.
    pub max_triangles: usize,
}
/// Sized from the Milestone 22.8 corpus measurement (docs/VALIDATION.md): importing every
/// root decoded at most one record per document entity, adapted at most 1.86, and
/// produced at most 99,434 triangles per document, 62.6 per entity, at the assembly
/// import chord.
impl Default for SessionBudgets {
    fn default() -> Self {
        Self {
            max_decoded_entities: 1_000_000,
            max_adapted_entities: 1_000_000,
            max_triangles: 20_000_000,
        }
    }
}
impl SessionBudgets {
    /// Decoded records allowed per document entity by [`Self::for_document`].
    pub const DECODED_PER_ENTITY: usize = 4;
    /// Adapted closure records allowed per document entity by [`Self::for_document`].
    pub const ADAPTED_PER_ENTITY: usize = 16;
    /// Mesh triangles allowed per document entity by [`Self::for_document`].
    pub const TRIANGLES_PER_ENTITY: usize = 256;
    /// The default budgets, raised for large documents: at least
    /// [`Self::DECODED_PER_ENTITY`] decoded and [`Self::ADAPTED_PER_ENTITY`] adapted
    /// records and [`Self::TRIANGLES_PER_ENTITY`] triangles per document entity. The
    /// budgets stay explicit in the returned value and are never derived again.
    pub fn for_document(document: &Document) -> Self {
        let entities = document.entities().len();
        let defaults = Self::default();
        Self {
            max_decoded_entities: defaults
                .max_decoded_entities
                .max(entities.saturating_mul(Self::DECODED_PER_ENTITY)),
            max_adapted_entities: defaults
                .max_adapted_entities
                .max(entities.saturating_mul(Self::ADAPTED_PER_ENTITY)),
            max_triangles: defaults
                .max_triangles
                .max(entities.saturating_mul(Self::TRIANGLES_PER_ENTITY)),
        }
    }
}

/// Work a session did and shared, for tests and benchmarks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionStatistics {
    /// Discovery passes run: solids, tessellations, presentations and each
    /// representation preference, at most once each.
    pub discoveries: usize,
    /// Decodes of several roots' closures together that succeeded.
    pub shared_decodes: usize,
    /// Shared decodes that failed (a record outside the profile, or more work than
    /// one root's budget), after which those roots were decoded alone.
    pub shared_fallbacks: usize,
    /// Roots prepared into a successful shared decode.
    pub shared_roots: usize,
    /// Decodes of a single root's closure, as the free functions decode, successful
    /// or not.
    pub root_decodes: usize,
    /// Records decoded by shared and single-root decodes together.
    pub decoded_entities: usize,
    /// Records of the closures handed to adapters. Above `decoded_entities` when
    /// roots share records that a shared decode decoded once.
    pub adapted_entities: usize,
    /// Root imports, successful or not.
    pub imports: usize,
    /// Triangles of meshes tessellated or imported through the session.
    pub triangles: usize,
}

/// One parsed document shared by every root, assembly and appearance import: see the
/// module documentation for the sharing rule, budgets and ownership.
pub struct ImportSession<'a> {
    document: &'a Document,
    options: ImportOptions,
    budgets: SessionBudgets,
    solids: OnceCell<Result<Vec<SolidRoot>, Error>>,
    tessellations: OnceCell<Result<Vec<TessellatedRoot>, Error>>,
    presentations: OnceCell<Result<Vec<PresentationRoot>, Error>>,
    /// Representation choices preferring exact roots, then tessellations.
    choices: [OnceCell<Result<Vec<RepresentationChoice>, Error>>; 2],
    shared: RefCell<Shared<'a>>,
    statistics: Cell<SessionStatistics>,
}
/// A summary: the decoded records a session holds are not printed.
impl std::fmt::Debug for ImportSession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportSession")
            .field("entities", &self.document.entities().len())
            .field("options", &self.options)
            .field("budgets", &self.budgets)
            .field("statistics", &self.statistics.get())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Shared<'a> {
    /// Shared decodes; `None` once every root it served was imported.
    decodes: Vec<Option<DecodedDocument<'a>>>,
    /// Roots each shared decode still serves.
    pending: Vec<usize>,
    /// The shared decode of each prepared root not yet imported.
    roots: BTreeMap<(ImportProfile, EntityId), usize>,
}

impl<'a> ImportSession<'a> {
    /// A session with [`SessionBudgets::for_document`].
    pub fn new(document: &'a Document, options: ImportOptions) -> Self {
        Self::with_budgets(document, options, SessionBudgets::for_document(document))
    }
    /// `options` apply to every discovery pass, decode and import of the session.
    pub fn with_budgets(
        document: &'a Document,
        options: ImportOptions,
        budgets: SessionBudgets,
    ) -> Self {
        Self {
            document,
            options,
            budgets,
            solids: OnceCell::new(),
            tessellations: OnceCell::new(),
            presentations: OnceCell::new(),
            choices: [OnceCell::new(), OnceCell::new()],
            shared: RefCell::default(),
            statistics: Cell::default(),
        }
    }
    pub fn document(&self) -> &'a Document {
        self.document
    }
    pub fn options(&self) -> ImportOptions {
        self.options
    }
    pub fn budgets(&self) -> SessionBudgets {
        self.budgets
    }
    pub fn statistics(&self) -> SessionStatistics {
        self.statistics.get()
    }

    /// [`discover_solids`], run once per session.
    pub fn solids(&self) -> Result<&[SolidRoot], Error> {
        cached(&self.solids, || {
            self.count(|s| s.discoveries += 1);
            discover_solids(self.document, self.options)
        })
    }
    /// [`discover_tessellations`], run once per session.
    pub fn tessellations(&self) -> Result<&[TessellatedRoot], Error> {
        cached(&self.tessellations, || {
            self.count(|s| s.discoveries += 1);
            discover_tessellations(self.document, self.options)
        })
    }
    /// [`discover_presentations`], run once per session.
    pub fn presentations(&self) -> Result<&[PresentationRoot], Error> {
        cached(&self.presentations, || {
            self.count(|s| s.discoveries += 1);
            discover_presentations(self.document, self.options)
        })
    }
    /// [`select_representations`] over the session's solids and tessellations, run
    /// once per preference.
    pub fn representation_choices(
        &self,
        preference: RepresentationPreference,
    ) -> Result<&[RepresentationChoice], Error> {
        let solids = self.solids()?;
        let tessellations = self.tessellations()?;
        let index = match preference {
            RepresentationPreference::Exact => 0,
            RepresentationPreference::Tessellated => 1,
        };
        cached(&self.choices[index], || {
            self.count(|s| s.discoveries += 1);
            select_representations(
                self.document,
                solids,
                tessellations,
                preference,
                self.options,
            )
        })
    }

    /// Decode the closures of `roots` with `profile` together, once, for their later
    /// imports, and return how many roots the shared decode serves. Nothing is shared
    /// (0) for fewer than two new roots, when the decode budget is spent, or when the
    /// shared decode fails; those roots are then decoded alone when imported, so
    /// their results, including errors, are the free functions'. Roots already
    /// prepared for `profile` and not yet imported keep their decode.
    pub fn prepare(&self, profile: ImportProfile, roots: &[EntityId]) -> usize {
        let mut shared = self.shared.borrow_mut();
        let mut unique = BTreeSet::new();
        let group: Vec<EntityId> = roots
            .iter()
            .copied()
            .filter(|&r| !shared.roots.contains_key(&(profile, r)) && unique.insert(r))
            .collect();
        if group.len() < 2
            || self.statistics().decoded_entities >= self.budgets.max_decoded_entities
        {
            return 0;
        }
        let Ok(decoded) = decode(profile, self.document, &group, self.options) else {
            self.count(|s| s.shared_fallbacks += 1);
            return 0;
        };
        self.count(|s| {
            s.shared_decodes += 1;
            s.shared_roots += group.len();
            s.decoded_entities += decoded.entities().len();
        });
        let index = shared.decodes.len();
        shared.decodes.push(Some(decoded));
        shared.pending.push(group.len());
        for &root in &group {
            shared.roots.insert((profile, root), index);
        }
        group.len()
    }

    /// [`import_faceted_solid`] with the session's options and decodes.
    pub fn import_faceted_solid(
        &self,
        root: EntityId,
        unit: LengthUnit,
        tolerance: ModelTolerance,
    ) -> Result<ImportedSolid, Error> {
        let options = self.options;
        self.import_with(ImportProfile::Faceted, root, |decoded, _| {
            adapt_solid(decoded, root, unit, tolerance, options, false)
        })
    }
    /// [`import_planar_solid`] with the session's options and decodes.
    pub fn import_planar_solid(
        &self,
        root: EntityId,
        unit: LengthUnit,
        tolerance: ModelTolerance,
    ) -> Result<ImportedSolid, Error> {
        let options = self.options;
        self.import_with(ImportProfile::Planar, root, |decoded, _| {
            adapt_solid(decoded, root, unit, tolerance, options, true)
        })
    }
    /// [`import_brep_solid`] with the session's options and decodes.
    pub fn import_brep_solid(
        &self,
        root: EntityId,
        length: LengthUnit,
        angle: AngleUnit,
        tolerance: ModelTolerance,
    ) -> Result<ImportedSolid, Error> {
        let options = self.options;
        self.import_with(ImportProfile::Brep, root, |decoded, closure| {
            brep::adapt(decoded, closure, root, length, angle, tolerance, options, None)
        })
    }
    /// [`import_tessellated`] with the session's options and decodes; the mesh counts
    /// against the triangle budget.
    pub fn import_tessellated(
        &self,
        root: EntityId,
        unit: LengthUnit,
        mesh_limits: tessstep_mesh::Limits,
    ) -> Result<ImportedTessellation, Error> {
        self.admit_triangles(Stage::Geometry, root)?;
        let options = self.options;
        let imported = self.import_with(ImportProfile::Tessellated, root, |decoded, _| {
            tessellated::adapt(decoded, root, unit, options, mesh_limits)
        })?;
        self.count(|s| s.triangles += imported.mesh().data().triangles.len());
        Ok(imported)
    }
    /// [`import_presentation`] with the session's options and decodes.
    pub fn import_presentation(
        &self,
        root: EntityId,
        unit: LengthUnit,
        limits: PresentationLimits,
    ) -> Result<ImportedPresentation, Error> {
        let options = self.options;
        self.import_with(ImportProfile::Tessellated, root, |decoded, _| {
            tessellated::adapt_presentation(decoded, root, unit, options, limits)
        })
    }
    /// [`ImportedSolid::tessellate`], counted against the triangle budget.
    pub fn tessellate(
        &self,
        solid: &ImportedSolid,
        tolerance: TessellationTolerance,
        options: TessellationOptions,
    ) -> Result<Mesh, Error> {
        self.admit_triangles(Stage::Tessellation, solid.root)?;
        let mesh = solid.tessellate(tolerance, options)?;
        self.count(|s| s.triangles += mesh.data().triangles.len());
        Ok(mesh)
    }

    /// [`link_assembly`] over the session's discovery; the session's import options
    /// replace `options.import`.
    pub fn link_assembly(
        &self,
        shapes: &[ShapeAsset],
        mut options: AssemblyOptions,
    ) -> Result<LinkedAssembly, Error> {
        options.import = self.options;
        assembly::link_with(self, shapes, options)
    }
    /// [`import_assembly`] over the session's discovery, with the selected roots of
    /// each profile prepared together; the session's import options replace
    /// `options.assembly.import`.
    pub fn import_assembly(
        &self,
        mut options: AssemblyImportOptions,
    ) -> Result<ImportedAssembly, Error> {
        options.assembly.import = self.options;
        assembly::import_with(self, options)
    }
    /// [`import_appearance`] of the session's document; the session's import options
    /// replace `options.import`.
    pub fn import_appearance(
        &self,
        assembly: &LinkedAssembly,
        mut options: StyleOptions,
    ) -> Result<ImportedAppearance, Error> {
        options.import = self.options;
        import_appearance(self.document, assembly, options)
    }

    fn count(&self, f: impl FnOnce(&mut SessionStatistics)) {
        let mut statistics = self.statistics.get();
        f(&mut statistics);
        self.statistics.set(statistics);
    }
    fn exceeded(
        &self,
        stage: Stage,
        root: EntityId,
        budget: &str,
        field: &str,
        value: usize,
    ) -> Error {
        Error {
            kind: ErrorKind::ResourceLimit,
            stage,
            entity: Some(root),
            source: self.document.entities().get(root).map(|e| e.source),
            message: format!("session {budget} budget exceeded ({field} {value})"),
        }
    }
    fn admit_triangles(&self, stage: Stage, root: EntityId) -> Result<(), Error> {
        let max = self.budgets.max_triangles;
        if self.statistics().triangles >= max {
            return Err(self.exceeded(stage, root, "triangle", "max_triangles", max));
        }
        Ok(())
    }
    fn admit_adapter(&self, root: EntityId, records: usize) -> Result<(), Error> {
        let max = self.budgets.max_adapted_entities;
        if self.statistics().adapted_entities >= max {
            return Err(self.exceeded(
                Stage::Geometry,
                root,
                "adapter",
                "max_adapted_entities",
                max,
            ));
        }
        self.count(|s| s.adapted_entities += records);
        Ok(())
    }

    /// Adapt `root` from its shared decode when it was prepared, else from a decode of
    /// its own closure. `adapt` receives the root's closure (in declaration order for
    /// the curved B-rep profile) when the records are a shared superset.
    fn import_with<T>(
        &self,
        profile: ImportProfile,
        root: EntityId,
        adapt: impl FnOnce(&DecodedDocument<'a>, Option<&[EntityId]>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.count(|s| s.imports += 1);
        let index = self.shared.borrow_mut().roots.remove(&(profile, root));
        let Some(index) = index else {
            let max = self.budgets.max_decoded_entities;
            if self.statistics().decoded_entities >= max {
                return Err(self.exceeded(
                    Stage::Profile,
                    root,
                    "decode",
                    "max_decoded_entities",
                    max,
                ));
            }
            self.count(|s| s.root_decodes += 1);
            let decoded = decode(profile, self.document, &[root], self.options)?;
            self.count(|s| s.decoded_entities += decoded.entities().len());
            self.admit_adapter(root, decoded.entities().len())?;
            return adapt(&decoded, None);
        };
        let result = {
            let shared = self.shared.borrow();
            let decoded = shared.decodes[index]
                .as_ref()
                .expect("a shared decode is kept while it serves roots");
            let mut closure = closure(decoded, root, &links(profile));
            if profile == ImportProfile::Brep {
                // The curved adapter scans the closure in declaration order, the order
                // of the root's own decode.
                let entities = self.document.entities();
                closure.sort_by_cached_key(|&id| entities.position(id));
            }
            self.admit_adapter(root, closure.len())
                .and_then(|()| adapt(decoded, Some(&closure)))
        };
        let mut shared = self.shared.borrow_mut();
        shared.pending[index] -= 1;
        if shared.pending[index] == 0 {
            shared.decodes[index] = None;
        }
        result
    }
}

fn cached<T>(
    cell: &OnceCell<Result<Vec<T>, Error>>,
    init: impl FnOnce() -> Result<Vec<T>, Error>,
) -> Result<&[T], Error> {
    match cell.get_or_init(init) {
        Ok(values) => Ok(values),
        Err(e) => Err(e.clone()),
    }
}

fn decode<'a>(
    profile: ImportProfile,
    document: &'a Document,
    roots: &[EntityId],
    options: ImportOptions,
) -> Result<DecodedDocument<'a>, Error> {
    match profile {
        ImportProfile::Faceted => decode_solid(document, roots, options, false),
        ImportProfile::Planar => decode_solid(document, roots, options, true),
        ImportProfile::Brep => brep::decode_roots(document, roots, options),
        ImportProfile::Tessellated => tessellated::decode_profile(document, roots, options),
    }
}

fn links(profile: ImportProfile) -> Vec<decode::LinkSlot> {
    match profile {
        ImportProfile::Faceted | ImportProfile::Planar => Vec::new(),
        ImportProfile::Brep => brep::links(),
        ImportProfile::Tessellated => tessellated::links(),
    }
}

/// The declarations in `entity`'s hierarchy that declare an attribute `name`.
fn declaring_owners(schemas: &SchemaSet, entity: DeclarationId, name: &str) -> Vec<DeclarationId> {
    let mut owners = Vec::new();
    let mut seen = BTreeSet::from([entity]);
    let mut stack = vec![entity];
    while let Some(id) = stack.pop() {
        let Some(DeclarationKind::Entity {
            supertypes,
            attributes,
            ..
        }) = schemas.declaration(id).map(|d| d.kind)
        else {
            continue;
        };
        if attributes.iter().any(|a| a.name == name) {
            owners.push(id);
        }
        stack.extend(supertypes.iter().filter(|&&s| seen.insert(s)));
    }
    owners
}

/// The root's reference closure within a shared decode, in entity ID order: the
/// records a decode of the root alone would hold. Link slots are not followed, as in
/// decoding: an attribute is a link when its record is a member of the link's entity
/// and the attribute is the one that entity's single declaring owner declares.
fn closure(
    decoded: &DecodedDocument<'_>,
    root: EntityId,
    links: &[decode::LinkSlot],
) -> Vec<EntityId> {
    let schemas = decoded.schemas();
    let links: Vec<(DeclarationId, DeclarationId, &str)> = links
        .iter()
        .filter_map(
            |l| match declaring_owners(schemas, l.entity, l.attribute)[..] {
                [owner] => Some((l.entity, owner, l.attribute)),
                _ => None,
            },
        )
        .collect();
    let mut seen = BTreeSet::from([root]);
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(view) = decoded.get(id) else {
            continue;
        };
        for attribute in &view.attributes {
            let link = links.iter().any(|&(entity, owner, name)| {
                attribute.owner == owner
                    && attribute.declaration.name == name
                    && view.types.contains(&entity)
            });
            if link {
                continue;
            }
            let mut values = vec![attribute.value];
            while let Some(value) = values.pop() {
                match &value.kind {
                    ValueKind::Reference(target) => {
                        if seen.insert(*target) {
                            stack.push(*target);
                        }
                    }
                    ValueKind::Aggregate(items) => values.extend(items),
                    ValueKind::Typed { value, .. } => values.push(value),
                    _ => {}
                }
            }
        }
    }
    seen.into_iter().collect()
}
