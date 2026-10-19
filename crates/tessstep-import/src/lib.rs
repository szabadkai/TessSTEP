//! Explicit, bounded STEP import profiles. No full AP schema conformance is implied.
//! Faceted and edge-based planar profiles follow one selected solid's reference closure,
//! validates named attributes, constructs planar geometry, and uses the independent
//! topology/UV/tessellation pipeline. The tessellated profile imports existing
//! triangulations directly as owned meshes. Units and tolerances are caller supplied.
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[rustfmt::skip]
mod profile;
#[allow(dead_code)]
#[rustfmt::skip]
mod planar_profile;
#[allow(dead_code)]
#[rustfmt::skip]
mod tessellated_profile;
#[allow(dead_code)]
#[rustfmt::skip]
mod brep_profile;
#[allow(dead_code)]
#[rustfmt::skip]
mod context_profile;
#[allow(dead_code)]
#[rustfmt::skip]
mod product_profile;
#[allow(dead_code)]
#[rustfmt::skip]
mod pcurve_profile;
#[allow(dead_code)]
#[rustfmt::skip]
mod style_profile;
mod assembly;
mod brep;
mod discovery;
mod encodings;
mod pcurve;
mod planar;
mod repair;
mod schema;
mod session;
mod style;
mod tessellated;
mod uv_loops;
pub use assembly::{
    AssemblyImportOptions, AssemblyNode, AssemblyOptions, AssetClass, ExcludedRecord,
    ImportedAssembly, LinkedAssembly, ProductInfo, RootImport, ShapeAsset, ShapeRootKind,
    import_assembly, link_assembly,
};
pub use brep::{Adaptations, import_brep_solid};
pub use brep::{ImportedSurfaceModel, SurfaceShell, import_surface_model};
pub use discovery::{
    ContextUnits, Pairing, PresentationRoot, RepresentationChoice, RepresentationPreference,
    RootRef, SolidKind, SolidRoot, TessellatedRoot, discover_presentations, discover_solids,
    discover_tessellations, select_representations,
};
pub use discovery::{SurfaceModelRoot, discover_surface_models};

pub use encodings::ToleratedEncodings;
pub use planar::import_planar_solid;
pub use repair::{RepairOptions, RepairReport};
pub use schema::{DeclaredSchema, Protocol, declared_schema};
pub use session::{ImportProfile, ImportSession, SessionBudgets, SessionStatistics};
pub use style::{ImportedAppearance, StyleCounts, StyleOptions, import_appearance};
pub use tessellated::{
    EdgeConnection, ImportedPresentation, ImportedTessellation, PresentationItem,
    PresentationItemKind, PresentationLimits, TessellatedEdge, TessellatedFace, TessellatedKind,
    TessellatedVertex, import_presentation, import_tessellated,
};

use std::collections::BTreeMap;
use tessstep_curves::{Curve, PlaneFrame};
pub use tessstep_math::TessellationTolerance;
use tessstep_math::{LengthUnit, ModelSpace, ModelTolerance, NumericalTolerance, Point, Vector};
use tessstep_model::{
    Document,
    decode::{self, DecodedDocument, EntityView},
};
use tessstep_part21::{EntityId, SourceSpan, StepValue, ValueKind};
pub use tessstep_tessellate::TessellationOptions;
use tessstep_topology::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Profile,
    Geometry,
    Topology,
    Tessellation,
    /// Product structure: definitions, occurrences and their placements.
    Product,
    /// Presentation styles: colours and transparency of shape items.
    Presentation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidOptions,
    MissingEntity,
    Unsupported,
    InvalidGeometry,
    ResourceLimit,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub stage: Stage,
    pub entity: Option<EntityId>,
    pub source: Option<SourceSpan>,
    pub message: String,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} {:?} at {:?}: {}",
            self.stage, self.kind, self.entity, self.message
        )
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Copy, Debug)]
pub struct ImportOptions {
    /// Independent profile-decoding and adapter work budgets, not a process RSS cap.
    pub max_work: usize,
    pub max_records: usize,
    /// Reject common exporter deviations that the default accepts without changing
    /// geometry: `$` instead of `*` in derived ORIENTED_EDGE endpoint slots, and the
    /// unit, measure and complex-instance encodings counted by [`ToleratedEncodings`].
    /// Strict import also accepts no tolerance deviation, whatever `deviation_factor` says,
    /// and keeps a `FACE_OUTER_BOUND` written on a hole loop
    /// ([`Adaptations::relabelled_outer_bounds`]), which tessellation then rejects.
    pub strict: bool,
    /// Opt-in topology repair of solid B-reps (sewing and orientation); off by
    /// default, so topology defects are rejected. See [`RepairOptions`].
    pub repair: RepairOptions,
    /// Curved B-rep tolerance policy: the largest distance accepted between an edge
    /// curve and its face surfaces, and between a vertex and its edge curve, as a
    /// multiple (at least 1) of the model tolerance. A distance above the model
    /// tolerance and within this multiple is a counted adaptation
    /// ([`Adaptations::off_surface_edges`], [`Adaptations::off_curve_vertices`]); a
    /// larger one is a typed rejection naming the distance and the bound. 1 accepts
    /// no deviation. The planar and faceted profiles do not use it. See
    /// docs/CURVED_IMPORT.md#tolerance-policy.
    pub deviation_factor: f64,
    /// The accepted distance in metres is never below this, whatever the model
    /// tolerance and `deviation_factor` make of it: a consumer passes the chord
    /// tolerance it will mesh with, since its mesh cannot show a smaller
    /// inconsistency (the C API does). Finite and at least 0; the default 0 leaves the
    /// factor alone. A factor of 1, or `strict`, still accepts no deviation.
    pub deviation_floor: f64,
}
impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            max_work: 5_000_000,
            max_records: 100_000,
            strict: false,
            repair: RepairOptions::default(),
            deviation_factor: Self::DEVIATION_FACTOR,
            deviation_floor: 0.,
        }
    }
}
impl ImportOptions {
    /// Logical work units allowed per document entity by [`Self::for_document`].
    pub const WORK_PER_ENTITY: usize = 512;
    /// The default [`Self::deviation_factor`].
    pub const DEVIATION_FACTOR: f64 = 16.;
    /// The default budgets, raised so that a root whose reference closure spans the
    /// whole document decodes and adapts: `max_work` is at least
    /// [`Self::WORK_PER_ENTITY`] per entity and `max_records` at least one per
    /// entity (the topology ceiling of one million records still applies). The
    /// budgets stay explicit in the returned options and are never derived again.
    pub fn for_document(document: &Document) -> Self {
        let entities = document.entities().len();
        let defaults = Self::default();
        Self {
            max_work: defaults
                .max_work
                .max(entities.saturating_mul(Self::WORK_PER_ENTITY)),
            max_records: defaults.max_records.max(entities),
            ..defaults
        }
    }
}
/// Owned validated geometry and the physical STEP identity of each model-local face.
#[derive(Clone, Debug)]
pub struct ImportedSolid {
    brep: NormalizedBrep,
    faces: Vec<EntityId>,
    root: EntityId,
    adaptations: Adaptations,
    /// STEP identity of edges and vertices, for locating tessellation errors.
    topology: uv_loops::TopologyEntities,
    repairs: RepairReport,
    deviation: f64,
}
impl ImportedSolid {
    /// Topology repairs applied by the opt-in repair stage ([`ImportOptions::repair`]);
    /// all zero without it.
    pub fn repairs(&self) -> RepairReport {
        self.repairs
    }
    pub(crate) fn topology(&self) -> &uv_loops::TopologyEntities {
        &self.topology
    }
    /// Representation adaptations applied by the importer; all zero for planar profiles.
    pub fn adaptations(&self) -> Adaptations {
        self.adaptations
    }
    /// The largest accepted distance in metres of an edge from its face surface or of
    /// a vertex from its edge curve beyond the model tolerance, or zero when none was
    /// accepted ([`ImportOptions::deviation_factor`]). The B-rep is then validated,
    /// trimmed and tessellated with the working tolerance reported by
    /// `brep().model_tolerance()`: the model tolerance plus twice this distance.
    pub fn largest_deviation(&self) -> f64 {
        self.deviation
    }
    pub fn brep(&self) -> &NormalizedBrep {
        &self.brep
    }
    pub fn face_entities(&self) -> &[EntityId] {
        &self.faces
    }
    /// Returns an owned mesh in metres. Triangle face_ids are physical STEP IDs.
    /// The mesh boundary follows each edge's 3D curve, so beside an edge accepted off
    /// its surface the facets lie up to [`Self::largest_deviation`] from that surface
    /// in addition to the chord; a chord below the deviation is still honoured on the
    /// surfaces themselves.
    pub fn tessellate(
        &self,
        tolerance: TessellationTolerance,
        options: TessellationOptions,
    ) -> Result<tessstep_mesh::Mesh, Error> {
        let mesh =
            tessstep_tessellate::tessellate_solid(&self.brep, SolidId(0), tolerance, options)
                .map_err(|e| {
                    // UV loop failures are located at their edge and name their cause.
                    if let Some((kind, entity, message)) =
                        e.face.and_then(|f| uv_loops::describe(self, f, &e.kind))
                    {
                        return Error {
                            kind,
                            stage: Stage::Tessellation,
                            entity: entity.or(Some(self.root)),
                            source: None,
                            message,
                        };
                    }
                    Error {
                        kind: if e.kind
                            == tessstep_tessellate::TessellationErrorKind::InvalidOptions
                        {
                            ErrorKind::InvalidOptions
                        } else if tessellation_limit(&e.kind) {
                            ErrorKind::ResourceLimit
                        } else {
                            ErrorKind::InvalidGeometry
                        },
                        stage: Stage::Tessellation,
                        entity: e
                            .face
                            .and_then(|f| self.faces.get(f.0).copied())
                            .or(Some(self.root)),
                        source: None,
                        message: e.to_string(),
                    }
                })?;
        // Model-local face indices become physical STEP IDs; labels need no new check.
        Ok(mesh.relabel_faces(|id| self.faces[id as usize].get()))
    }
}
fn boundary_limit(e: &tessstep_tessellate::BoundaryError) -> bool {
    matches!(e, tessstep_tessellate::BoundaryError::ResourceLimit)
        || matches!(e, tessstep_tessellate::BoundaryError::Trim(e) if e.kind == tessstep_trim::ErrorKind::Limit)
}
fn tessellation_limit(e: &tessstep_tessellate::TessellationErrorKind) -> bool {
    use tessstep_tessellate::{PlanarError as P, TessellationErrorKind as T};
    match e {
        T::ResourceLimit | T::Mesh(tessstep_mesh::Error::ResourceLimit) => true,
        T::Sampling(e) => e.kind == tessstep_tessellate::ErrorKind::ResourceLimit,
        T::Boundary(e) | T::Polygon(P::Boundary(e)) => boundary_limit(e),
        T::Polygon(P::ResourceLimit) => true,
        T::Polygon(P::InvalidPolygon(e)) => e.kind == tessstep_trim::ErrorKind::Limit,
        _ => false,
    }
}

/// Import a selected FACETED_BREP with one CLOSED_SHELL, planar FACE_SURFACEs
/// (including ADVANCED_FACE), explicit FACE_OUTER_BOUND and POLY_LOOP boundaries.
/// Only this root's reference closure is structurally checked against the bundled
/// original reduced profile. Unrelated records and FILE_SCHEMA are not validated.
/// Shared points/edges are identified by STEP point IDs, never by proximity, unless
/// the opt-in [`ImportOptions::repair`] sews them (counted in [`ImportedSolid::repairs`]).
/// No unit inference, assembly placement, curved edges or surfaces is performed.
pub fn import_faceted_solid(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    tolerance: ModelTolerance,
    options: ImportOptions,
) -> Result<ImportedSolid, Error> {
    import_solid(document, root, unit, tolerance, options, false)
}

/// Reduced import profiles are not STEP schemas: an undeclared name is outside the
/// profile, which says nothing about whether the application protocol defines it.
fn outside_profile(
    document: &Document,
    schemas: &tessstep_schema::SchemaSet,
    schema_name: &str,
    entity: Option<EntityId>,
) -> String {
    let Some(records) = entity
        .and_then(|id| document.entities().get(id))
        .map(|e| e.kind.records())
    else {
        return format!("entity outside the {schema_name} import profile");
    };
    let declared = |name: &str| {
        schemas.schema(schema_name).is_some_and(|s| {
            s.symbols.iter().any(|symbol| {
                symbol.name.eq_ignore_ascii_case(name)
                    && schemas.declaration(symbol.declaration).is_some_and(|d| {
                        matches!(d.kind, tessstep_schema::DeclarationKind::Entity { .. })
                    })
            })
        })
    };
    let outside: Vec<&str> = records
        .iter()
        .map(|r| r.name.as_ref())
        .filter(|name| !declared(name))
        .collect();
    if records.len() == 1 {
        format!(
            "{} is outside the {schema_name} import profile",
            records[0].name.as_ref()
        )
    } else {
        let all: Vec<&str> = records.iter().map(|r| r.name.as_ref()).collect();
        format!(
            "complex instance ({}) has components outside the {schema_name} import profile: {}",
            all.join(", "),
            outside.join(", ")
        )
    }
}

fn import_solid(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    tolerance: ModelTolerance,
    options: ImportOptions,
    planar: bool,
) -> Result<ImportedSolid, Error> {
    let decoded = decode_solid(document, &[root], options, planar)?;
    adapt_solid(&decoded, root, unit, tolerance, options, planar)
}

/// Decode the reference closure of `roots` with the faceted or edge-based planar
/// profile. A session decodes several roots at once; see `session.rs`.
pub(crate) fn decode_solid<'a>(
    document: &'a Document,
    roots: &[EntityId],
    options: ImportOptions,
    planar: bool,
) -> Result<DecodedDocument<'a>, Error> {
    use tessstep_schema::EntityBinding;
    let schema = if planar {
        &planar_profile::SCHEMA_SET
    } else {
        &profile::SCHEMA_SET
    };
    let oriented = planar_profile::schema_tessstep_planar::Entity_ORIENTED_EDGE::DECLARATION;
    let slots = [
        decode::OmittedSlot {
            entity: oriented,
            attribute: "EDGE_START",
            allow_unset: !options.strict,
            allow_value: false,
        },
        decode::OmittedSlot {
            entity: oriented,
            attribute: "EDGE_END",
            allow_unset: !options.strict,
            allow_value: false,
        },
    ];
    let schema_name = if planar {
        "tessstep_planar"
    } else {
        "tessstep_faceted"
    };
    decode::decode_reachable_profile(
        document,
        schema,
        schema_name,
        roots,
        if planar { &slots } else { &[] },
        decode::Limits {
            max_work: options.max_work,
            ..decode::Limits::default()
        },
    )
    .map_err(|e| Error {
        kind: match e.kind {
            decode::ErrorKind::ResourceLimit => ErrorKind::ResourceLimit,
            decode::ErrorKind::UnknownEntity | decode::ErrorKind::Unsupported => {
                ErrorKind::Unsupported
            }
            decode::ErrorKind::MissingReference => ErrorKind::MissingEntity,
            _ => ErrorKind::InvalidGeometry,
        },
        stage: Stage::Profile,
        entity: e.entity,
        source: e.source,
        message: if e.kind == decode::ErrorKind::UnknownEntity {
            outside_profile(document, schema, schema_name, e.entity)
        } else {
            e.to_string()
        },
    })
}

/// Adapt one decoded root. `decoded` holds the root's closure, or a superset that a
/// session decoded for several roots: the adapter only follows references from the
/// root, so the result is the same.
pub(crate) fn adapt_solid(
    decoded: &DecodedDocument<'_>,
    root: EntityId,
    unit: LengthUnit,
    tolerance: ModelTolerance,
    options: ImportOptions,
    planar: bool,
) -> Result<ImportedSolid, Error> {
    let document = decoded.document();
    let mut c = Context {
        decoded,
        raw: RawBrep::default(),
        points: BTreeMap::new(),
        point_entities: Vec::new(),
        edges: BTreeMap::new(),
        edge_entities: BTreeMap::new(),
        remaining: options.max_work,
        max_work: options.max_work,
        max_records: options.max_records.min(1_000_000),
        records: 0,
        current: root,
        unit,
        tolerance,
    };
    let solid = c.view(root)?;
    if !c.is(
        solid,
        if planar {
            "MANIFOLD_SOLID_BREP"
        } else {
            "FACETED_BREP"
        },
    ) {
        return Err(c.error(
            ErrorKind::Unsupported,
            "selected root does not match the requested solid profile",
        ));
    }
    let shell = c.reference(solid, "OUTER")?;
    let face_refs = c.aggregate(shell, "CFS_FACES")?;
    let mut faces = Vec::new();
    for face_ref in face_refs {
        c.charge(1)?;
        let face = c.target(face_ref)?;
        c.current = face.id;
        let surface = c.reference(face, "FACE_GEOMETRY")?;
        let placement = c.reference(surface, "POSITION")?;
        let frame = c.frame(placement)?;
        let same = c.boolean(face, "SAME_SENSE")?;
        let mut outer = None;
        let mut holes = Vec::new();
        let bounds = c.aggregate(face, "BOUNDS")?;
        for bound_ref in bounds {
            c.charge(1)?;
            let bound = c.target(bound_ref)?;
            c.current = bound.id;
            let is_outer = c.is(bound, "FACE_OUTER_BOUND") || (planar && bounds.len() == 1);
            let orientation = c.boolean(bound, "ORIENTATION")?;
            let wire = if planar {
                c.edge_wire(c.reference(bound, "BOUND")?, frame, orientation != same)?
            } else {
                let poly = c.reference(bound, "BOUND")?;
                let mut vertices = Vec::new();
                for point_ref in c.aggregate(poly, "POLYGON")? {
                    c.charge(1)?;
                    let point = c.target(point_ref)?;
                    let vertex = c.vertex(point)?;
                    vertices.push(vertex);
                }
                // The kernel stores UV boundaries oriented with the surface; the face
                // orientation restores STEP's same_sense in shell incidence/triangles.
                if orientation != same {
                    vertices.reverse();
                }
                c.wire(&vertices, frame)?
            };
            if is_outer {
                if outer.replace(wire).is_some() {
                    return Err(c.error(ErrorKind::InvalidGeometry, "multiple outer bounds"));
                }
            } else {
                holes.push(wire);
            }
        }
        c.current = face.id;
        let outer = outer.ok_or_else(|| {
            c.error(
                ErrorKind::Unsupported,
                "an explicit outer bound is required",
            )
        })?;
        c.record()?;
        c.raw.faces.push(Face {
            surface: SurfaceGeometry::Analytic(tessstep_surfaces::Surface::plane(frame)),
            outer,
            holes,
            orientation: if same {
                Orientation::Forward
            } else {
                Orientation::Reversed
            },
        });
        faces.push(face.id);
    }
    c.current = root;
    c.record()?;
    c.record()?;
    c.raw.shells.push(Shell {
        faces: (0..faces.len()).map(FaceId).collect(),
        closed: true,
    });
    c.raw.solids.push(Solid::new(ShellId(0)));
    let mut edges = vec![None; c.raw.edges.len()];
    for (&entity, &(edge, _)) in &c.edge_entities {
        if let Some(slot) = edges.get_mut(edge.0) {
            *slot = Some(entity);
        }
    }
    let mut locator = repair::Locator {
        document,
        root,
        edges,
        vertices: c.point_entities.clone(),
    };
    let (raw, report, tolerance) =
        repair::sew_raw(std::mem::take(&mut c.raw), tolerance, options, &mut locator)?;
    let topology = uv_loops::TopologyEntities {
        edges: locator.edges.clone(),
        vertices: locator.vertices.clone(),
    };
    let (brep, repairs) = repair::finish(raw, tolerance, options, report, &locator)?;
    Ok(ImportedSolid {
        brep,
        faces,
        root,
        adaptations: Adaptations::default(),
        topology,
        repairs,
        deviation: 0.,
    })
}
struct Context<'d, 'a> {
    decoded: &'d DecodedDocument<'a>,
    raw: RawBrep,
    points: BTreeMap<EntityId, VertexId>,
    point_entities: Vec<EntityId>,
    edges: BTreeMap<(VertexId, VertexId), EdgeId>,
    edge_entities: BTreeMap<EntityId, (EdgeId, bool)>,
    remaining: usize,
    max_work: usize,
    max_records: usize,
    records: usize,
    current: EntityId,
    unit: LengthUnit,
    tolerance: ModelTolerance,
}
impl<'d, 'a> Context<'d, 'a> {
    fn error(&self, kind: ErrorKind, message: impl ToString) -> Error {
        Error {
            kind,
            stage: Stage::Geometry,
            entity: Some(self.current),
            source: self
                .decoded
                .document()
                .entities()
                .get(self.current)
                .map(|e| e.source),
            message: message.to_string(),
        }
    }
    fn checked<T, E: std::fmt::Display>(&self, value: Result<T, E>) -> Result<T, Error> {
        value.map_err(|e| self.error(ErrorKind::InvalidGeometry, e))
    }
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        self.remaining = self.remaining.checked_sub(n).ok_or_else(|| {
            self.error(
                ErrorKind::ResourceLimit,
                format!("adapter work budget exceeded (max_work {})", self.max_work),
            )
        })?;
        Ok(())
    }
    fn record(&mut self) -> Result<(), Error> {
        self.charge(1)?;
        if self.records >= self.max_records {
            return Err(self.error(
                ErrorKind::ResourceLimit,
                format!(
                    "topology record budget exceeded (max_records {})",
                    self.max_records
                ),
            ));
        }
        self.records += 1;
        Ok(())
    }
    fn is(&self, v: &EntityView<'_>, name: &str) -> bool {
        v.types.iter().any(|id| {
            self.decoded
                .schemas()
                .declaration(*id)
                .is_some_and(|d| d.name == name)
        })
    }
    fn view(&self, id: EntityId) -> Result<&'d EntityView<'a>, Error> {
        self.decoded
            .get(id)
            .ok_or_else(|| self.error(ErrorKind::MissingEntity, "entity outside selected closure"))
    }
    fn attr(&self, v: &'d EntityView<'a>, name: &str) -> Result<&'a StepValue, Error> {
        v.attributes
            .iter()
            .find(|a| a.declaration.name == name)
            .map(|a| a.value)
            .ok_or_else(|| self.error(ErrorKind::Unsupported, "profile attribute absent"))
    }
    fn target(&self, v: &StepValue) -> Result<&'d EntityView<'a>, Error> {
        if let ValueKind::Reference(id) = v.kind {
            self.view(id)
        } else {
            Err(self.error(ErrorKind::InvalidGeometry, "expected entity reference"))
        }
    }
    fn reference(&self, v: &'d EntityView<'a>, name: &str) -> Result<&'d EntityView<'a>, Error> {
        self.target(self.attr(v, name)?)
    }
    fn aggregate(&self, v: &'d EntityView<'a>, name: &str) -> Result<&'a [StepValue], Error> {
        if let ValueKind::Aggregate(values) = &self.attr(v, name)?.kind {
            Ok(values)
        } else {
            Err(self.error(ErrorKind::InvalidGeometry, "expected aggregate"))
        }
    }
    fn boolean(&self, v: &'d EntityView<'a>, name: &str) -> Result<bool, Error> {
        match &self.attr(v, name)?.kind {
            ValueKind::Enumeration(s) if s.as_ref() == "T" => Ok(true),
            ValueKind::Enumeration(s) if s.as_ref() == "F" => Ok(false),
            _ => Err(self.error(ErrorKind::InvalidGeometry, "expected boolean")),
        }
    }
    fn triple(&self, v: &'d EntityView<'a>, name: &str) -> Result<[f64; 3], Error> {
        let values = self.aggregate(v, name)?;
        if values.len() != 3 {
            return Err(self.error(ErrorKind::InvalidGeometry, "expected three coordinates"));
        }
        let mut xyz = [0.; 3];
        for (out, value) in xyz.iter_mut().zip(values) {
            *out = match value.kind {
                ValueKind::Real(v) => v,
                ValueKind::Integer(v) => v as f64,
                _ => {
                    return Err(
                        self.error(ErrorKind::InvalidGeometry, "expected numeric coordinate")
                    );
                }
            };
        }
        Ok(xyz)
    }
    fn point(&self, v: &'d EntityView<'a>) -> Result<Point<ModelSpace, 3>, Error> {
        let mut xyz = self.triple(v, "COORDINATES")?;
        for x in &mut xyz {
            *x = self.checked(self.unit.to_metres(*x))?;
        }
        self.checked(Point::new(xyz))
    }
    fn direction(&self, v: &'d EntityView<'a>) -> Result<Vector<ModelSpace, 3>, Error> {
        let v = self.checked(Vector::new(self.triple(v, "DIRECTION_RATIOS")?))?;
        Ok(self.checked(v.normalized())?.vector())
    }
    fn frame(&self, v: &'d EntityView<'a>) -> Result<PlaneFrame<ModelSpace, 3>, Error> {
        let origin = self.point(self.reference(v, "LOCATION")?)?;
        let axis = self.attr(v, "AXIS")?;
        let z = if matches!(axis.kind, ValueKind::Null) {
            self.checked(Vector::new([0., 0., 1.]))?
        } else {
            self.direction(self.target(axis)?)?
        };
        let reference = self.attr(v, "REF_DIRECTION")?;
        let default = matches!(reference.kind, ValueKind::Null);
        let mut x = if default {
            self.checked(Vector::new([1., 0., 0.]))?
        } else {
            self.direction(self.target(reference)?)?
        };
        if self.checked(self.checked(z.cross(x))?.norm())?
            <= NumericalTolerance::default().relative()
        {
            if !default {
                return Err(self.error(ErrorKind::InvalidGeometry, "parallel placement axes"));
            }
            x = self.checked(Vector::new([0., 1., 0.]))?;
        }
        let projection = self.checked(z.scaled(self.checked(z.dot(x))?))?;
        x = self.checked(x.subtracted(projection))?;
        let y = self.checked(z.cross(x))?;
        self.checked(PlaneFrame::new(origin, x, y, NumericalTolerance::default()))
    }
    fn vertex(&mut self, v: &'d EntityView<'a>) -> Result<VertexId, Error> {
        if let Some(&id) = self.points.get(&v.id) {
            return Ok(id);
        }
        self.current = v.id;
        self.record()?;
        let id = VertexId(self.raw.vertices.len());
        self.raw.vertices.push(Vertex {
            position: self.point(v)?,
        });
        self.points.insert(v.id, id);
        self.point_entities.push(v.id);
        Ok(id)
    }
    fn uv(
        &self,
        point: Point<ModelSpace, 3>,
        frame: PlaneFrame<ModelSpace, 3>,
    ) -> Result<[f64; 2], Error> {
        let d = self.checked(point.difference(frame.origin()))?;
        let normal = self.checked(frame.x().cross(frame.y()))?;
        if self.checked(d.dot(normal))?.abs() > self.tolerance.distance().as_metres() {
            return Err(self.error(
                ErrorKind::InvalidGeometry,
                "polygon point is off its declared plane",
            ));
        }
        Ok([
            self.checked(d.dot(frame.x()))?,
            self.checked(d.dot(frame.y()))?,
        ])
    }
    fn wire(
        &mut self,
        vertices: &[VertexId],
        frame: PlaneFrame<ModelSpace, 3>,
    ) -> Result<WireId, Error> {
        let mut coedges = Vec::new();
        for i in 0..vertices.len() {
            self.charge(1)?;
            let a = vertices[i];
            let b = vertices[(i + 1) % vertices.len()];
            let key = (a.min(b), a.max(b));
            let edge = if let Some(&edge) = self.edges.get(&key) {
                edge
            } else {
                self.record()?;
                let p0 = self.raw.vertices[a.0].position;
                let p1 = self.raw.vertices[b.0].position;
                let curve = self.checked(Curve::line(p0, self.checked(p1.difference(p0))?))?;
                let edge = EdgeId(self.raw.edges.len());
                self.raw.edges.push(Edge {
                    vertices: [a, b],
                    curve: CurveGeometry::Analytic(curve),
                    range: [0., 1.],
                });
                self.edges.insert(key, edge);
                edge
            };
            let [start, end] = self.raw.edges[edge.0].vertices;
            self.current = self.point_entities[start.0];
            let u0 = self.uv(self.raw.vertices[start.0].position, frame)?;
            self.current = self.point_entities[end.0];
            let u1 = self.uv(self.raw.vertices[end.0].position, frame)?;
            let curve = self.checked(Curve::line(
                self.checked(Point::new(u0))?,
                self.checked(Vector::new([u1[0] - u0[0], u1[1] - u0[1]]))?,
            ))?;
            self.record()?;
            coedges.push(CoedgeId(self.raw.coedges.len()));
            self.raw.coedges.push(Coedge {
                edge,
                orientation: if start == a {
                    Orientation::Forward
                } else {
                    Orientation::Reversed
                },
                pcurve: Some(Pcurve {
                    curve: CurveGeometry::Analytic(curve),
                    range: [0., 1.],
                }),
            });
        }
        self.record()?;
        let wire = WireId(self.raw.wires.len());
        self.raw.wires.push(Wire { coedges });
        Ok(wire)
    }
}
