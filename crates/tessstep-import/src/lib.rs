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
mod planar;
mod tessellated;
pub use planar::import_planar_solid;
pub use tessellated::{ImportedTessellation, TessellatedFace, TessellatedKind, import_tessellated};

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
pub struct ImportLimits {
    /// Independent profile-decoding and adapter work budgets, not a process RSS cap.
    pub max_work: usize,
    pub max_records: usize,
}
impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_work: 5_000_000,
            max_records: 100_000,
        }
    }
}
/// Owned validated geometry and the physical STEP identity of each model-local face.
#[derive(Clone, Debug)]
pub struct ImportedSolid {
    brep: NormalizedBrep,
    faces: Vec<EntityId>,
    root: EntityId,
}
impl ImportedSolid {
    pub fn brep(&self) -> &NormalizedBrep {
        &self.brep
    }
    pub fn face_entities(&self) -> &[EntityId] {
        &self.faces
    }
    /// Returns an owned mesh in metres. Triangle face_ids are physical STEP IDs.
    pub fn tessellate(
        &self,
        tolerance: TessellationTolerance,
        options: TessellationOptions,
    ) -> Result<tessstep_mesh::Mesh, Error> {
        let mesh =
            tessstep_tessellate::tessellate_solid(&self.brep, SolidId(0), tolerance, options)
                .map_err(|e| Error {
                    kind: if e.kind == tessstep_tessellate::TessellationErrorKind::InvalidOptions {
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
                })?;
        let mut data = mesh.into_data();
        for id in &mut data.face_ids {
            *id = self.faces[*id as usize].get();
        }
        tessstep_mesh::Mesh::new(data, options.mesh).map_err(|e| Error {
            kind: if e == tessstep_mesh::Error::ResourceLimit {
                ErrorKind::ResourceLimit
            } else {
                ErrorKind::InvalidGeometry
            },
            stage: Stage::Tessellation,
            entity: Some(self.root),
            source: None,
            message: e.to_string(),
        })
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
/// Shared points/edges are identified by STEP point IDs, never by proximity.
/// No unit inference, assembly placement, repair, curved edges or surfaces is performed.
pub fn import_faceted_solid(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    tolerance: ModelTolerance,
    limits: ImportLimits,
) -> Result<ImportedSolid, Error> {
    import_solid(document, root, unit, tolerance, limits, false)
}

fn import_solid(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    tolerance: ModelTolerance,
    limits: ImportLimits,
    planar: bool,
) -> Result<ImportedSolid, Error> {
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
        },
        decode::OmittedSlot {
            entity: oriented,
            attribute: "EDGE_END",
        },
    ];
    let decoded = decode::decode_reachable_profile(
        document,
        schema,
        if planar {
            "tessstep_planar"
        } else {
            "tessstep_faceted"
        },
        &[root],
        if planar { &slots } else { &[] },
        decode::Limits {
            max_work: limits.max_work,
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
        message: e.to_string(),
    })?;
    let mut c = Context {
        decoded: &decoded,
        raw: RawBrep::default(),
        points: BTreeMap::new(),
        point_entities: Vec::new(),
        edges: BTreeMap::new(),
        edge_entities: BTreeMap::new(),
        remaining: limits.max_work,
        max_records: limits.max_records.min(1_000_000),
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
    c.raw.solids.push(Solid { shell: ShellId(0) });
    let brep = c
        .raw
        .validate(
            tolerance,
            ValidationLimits {
                max_records: limits.max_records,
                max_work: limits.max_work,
            },
        )
        .map_err(|e| Error {
            kind: if e.defect == Defect::ResourceLimit {
                ErrorKind::ResourceLimit
            } else {
                ErrorKind::InvalidGeometry
            },
            stage: Stage::Topology,
            entity: Some(root),
            source: document.entities().get(root).map(|e| e.source),
            message: e.to_string(),
        })?
        .normalize();
    Ok(ImportedSolid { brep, faces, root })
}
struct Context<'d, 'a> {
    decoded: &'d DecodedDocument<'a>,
    raw: RawBrep,
    points: BTreeMap<EntityId, VertexId>,
    point_entities: Vec<EntityId>,
    edges: BTreeMap<(VertexId, VertexId), EdgeId>,
    edge_entities: BTreeMap<EntityId, (EdgeId, bool)>,
    remaining: usize,
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
        self.remaining = self
            .remaining
            .checked_sub(n)
            .ok_or_else(|| self.error(ErrorKind::ResourceLimit, "adapter work budget"))?;
        Ok(())
    }
    fn record(&mut self) -> Result<(), Error> {
        self.charge(1)?;
        if self.records >= self.max_records {
            return Err(self.error(ErrorKind::ResourceLimit, "topology record budget"));
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
