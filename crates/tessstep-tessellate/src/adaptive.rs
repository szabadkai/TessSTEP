use crate::{
    BoundaryError, PlanarError, PlanarOptions, SamplingLimits, SharedEdges, sample_edges_seeded,
};
use std::collections::{BTreeMap, BTreeSet};
use tessstep_curves::spline::KnotSide;
use tessstep_math::{Direction3, ModelSpace, NumericalTolerance, Point3, TessellationTolerance};
use tessstep_mesh::{Mesh, MeshData};
use tessstep_surfaces::AxisDomain;
use tessstep_topology::{
    EdgeId, FaceId, NormalizedBrep, ShellId, SolidId, SurfaceGeometry, VertexId,
};

#[derive(Clone, Copy, Debug)]
pub struct TessellationOptions {
    pub sampling: SamplingLimits,
    pub planar: PlanarOptions,
    /// Per-face conforming refinement rounds, capped at 48.
    pub max_rounds: usize,
    /// Global boundary resampling passes, capped at 24.
    pub max_boundary_passes: usize,
    /// Adaptive probes and edge sampling across faces/restarts; trim budgets are separate.
    pub max_evaluations: usize,
    /// Refinement/assembly work shared across faces and restarts.
    pub max_work: usize,
    pub mesh: tessstep_mesh::Limits,
}
impl Default for TessellationOptions {
    fn default() -> Self {
        Self {
            sampling: SamplingLimits::default(),
            planar: PlanarOptions::default(),
            max_rounds: 32,
            max_boundary_passes: 16,
            max_evaluations: 10_000_000,
            max_work: 50_000_000,
            mesh: tessstep_mesh::Limits::default(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum TessellationErrorKind {
    InvalidOptions,
    InvalidHandle,
    DuplicateFace,
    ResourceLimit,
    Geometry,
    SingularSurface,
    UnresolvedTolerance,
    Sampling(crate::Error),
    Boundary(BoundaryError),
    Polygon(PlanarError),
    Mesh(tessstep_mesh::Error),
}
#[derive(Clone, Debug, PartialEq)]
pub struct TessellationError {
    pub face: Option<FaceId>,
    pub kind: TessellationErrorKind,
}
impl std::fmt::Display for TessellationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tessellation {:?}: {:?}", self.face, self.kind)
    }
}
impl std::error::Error for TessellationError {}
fn error(kind: TessellationErrorKind) -> TessellationError {
    TessellationError { face: None, kind }
}
type Failure = TessellationErrorKind;
type Key = (u32, u32);
fn key(a: u32, b: u32) -> Key {
    (a.min(b), a.max(b))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    Vertex(VertexId),
    Edge(EdgeId, usize),
    Interior(FaceId, usize),
}
#[derive(Clone, Copy)]
struct Vertex {
    uv: [f64; 2],
    position: Point3<ModelSpace>,
    /// None only at singular vertices (poles, apices), whose normal depends on the
    /// direction of approach; triangles evaluate it from inside at completion.
    normal: Option<Direction3<ModelSpace>>,
    source: Source,
    /// For a sample of a collapsed edge (a pole copy), the chart axis along its
    /// singular line: its coordinate there is arbitrary, so triangles use the one of
    /// their opposite edge.
    free: Option<usize>,
}
struct FaceMesh {
    vertices: Vec<Vertex>,
    triangles: Vec<[u32; 3]>,
    /// Per-corner normals, oriented with the surface.
    normals: Vec<[Direction3<ModelSpace>; 3]>,
    /// Per-corner chart points; a pole copy takes its triangle's polar coordinate.
    uvs: Vec<[[f64; 2]; 3]>,
}
struct Work {
    remaining: usize,
    evaluations: usize,
}
impl Work {
    fn charge(&mut self, n: usize) -> Result<(), Failure> {
        self.remaining = self
            .remaining
            .checked_sub(n)
            .ok_or(Failure::ResourceLimit)?;
        Ok(())
    }
    fn eval(
        &mut self,
        surface: &SurfaceGeometry,
        uv: [f64; 2],
        sides: [KnotSide; 2],
    ) -> Result<(Point3<ModelSpace>, Direction3<ModelSpace>), Failure> {
        let (position, normal) = self.eval_any(surface, uv, sides)?;
        Ok((position, normal.ok_or(Failure::SingularSurface)?))
    }
    /// Position and, unless the parameterization is singular there, the normal.
    fn eval_any(
        &mut self,
        surface: &SurfaceGeometry,
        uv: [f64; 2],
        sides: [KnotSide; 2],
    ) -> Result<(Point3<ModelSpace>, Option<Direction3<ModelSpace>>), Failure> {
        self.charge(1)?;
        self.evaluations = self
            .evaluations
            .checked_sub(1)
            .ok_or(Failure::ResourceLimit)?;
        let jet = match surface {
            SurfaceGeometry::Analytic(s) => s.evaluate(uv[0], uv[1]),
            SurfaceGeometry::Nurbs(s) => s.evaluate_on_sides(uv[0], uv[1], sides),
        }
        .map_err(|_| Failure::Geometry)?;
        Ok((jet.position, jet.normal(NumericalTolerance::default()).ok()))
    }
}
impl Work {
    /// Parametric speeds |S_u| and |S_v| at a UV point.
    fn speeds(&mut self, surface: &SurfaceGeometry, uv: [f64; 2]) -> Result<[f64; 2], Failure> {
        self.charge(1)?;
        self.evaluations = self
            .evaluations
            .checked_sub(1)
            .ok_or(Failure::ResourceLimit)?;
        let jet = surface.evaluate(uv).map_err(|_| Failure::Geometry)?;
        Ok([
            jet.du.norm().map_err(|_| Failure::Geometry)?,
            jet.dv.norm().map_err(|_| Failure::Geometry)?,
        ])
    }
}
/// Per-face diagonal UV metric: RMS parametric speeds over (at most 64) boundary
/// vertices. Scaling UV by it approximates arc length to first order, so edge
/// selection and flips do not create needles when one parameter spans much more
/// surface than the other. Degenerate speeds fall back to the unscaled chart.
fn face_metric(
    surface: &SurfaceGeometry,
    vertices: &[Vertex],
    work: &mut Work,
) -> Result<[f64; 2], Failure> {
    let step = vertices.len().div_ceil(64).max(1);
    let mut sum = [0.; 2];
    let mut count = 0.;
    for v in vertices.iter().step_by(step) {
        let s = work.speeds(surface, v.uv)?;
        sum[0] += s[0] * s[0];
        sum[1] += s[1] * s[1];
        count += 1.;
    }
    let metric = sum.map(|x| (x / count).sqrt());
    if metric.iter().all(|m| m.is_finite() && *m > 0.) {
        Ok(metric)
    } else {
        Ok([1.; 2])
    }
}
/// Tessellate selected constructed faces and weld positions only by topology/cache
/// identity. Surface normals and UVs remain per corner, preserving seams and creases.
pub fn tessellate_faces(
    brep: &NormalizedBrep,
    faces: &[FaceId],
    tolerance: TessellationTolerance,
    options: TessellationOptions,
) -> Result<Mesh, TessellationError> {
    if options.max_rounds > 48
        || options.max_boundary_passes > 24
        || options.planar.max_vertices > 1_000_000
        || options.planar.max_triangles > 2_000_000
    {
        return Err(error(Failure::InvalidOptions));
    }
    let mut work = Work {
        remaining: options.max_work,
        evaluations: options.max_evaluations,
    };
    work.charge(faces.len()).map_err(error)?;
    let mut unique = BTreeSet::new();
    for &face in faces {
        if face.0 >= brep.data().faces.len() {
            return Err(error(Failure::InvalidHandle));
        }
        if !unique.insert(face) {
            return Err(error(Failure::DuplicateFace));
        }
    }
    let mut forced: Vec<Vec<f64>> = Vec::new();
    for pass in 0..=options.max_boundary_passes {
        let mut sampling = options.sampling;
        sampling.max_evaluations = sampling.max_evaluations.min(work.evaluations);
        let shared = sample_edges_seeded(brep, tolerance, sampling, &forced)
            .map_err(|e| error(Failure::Sampling(e)))?;
        work.evaluations = work
            .evaluations
            .checked_sub(shared.evaluations())
            .ok_or_else(|| error(Failure::ResourceLimit))?;
        let mut requests = Vec::new();
        let mut completed = Vec::new();
        let mut vertices = 0usize;
        let mut triangles = 0usize;
        for &face in faces {
            match face_mesh(&shared, face, tolerance, options, &mut work).map_err(|kind| {
                TessellationError {
                    face: Some(face),
                    kind,
                }
            })? {
                Outcome::Refine(mut needs) => requests.append(&mut needs),
                Outcome::Complete(mesh) => {
                    vertices = vertices
                        .checked_add(mesh.vertices.len())
                        .ok_or_else(|| error(Failure::ResourceLimit))?;
                    triangles = triangles
                        .checked_add(mesh.triangles.len())
                        .ok_or_else(|| error(Failure::ResourceLimit))?;
                    if vertices > options.mesh.max_vertices.min(u32::MAX as usize)
                        || triangles > options.mesh.max_triangles
                    {
                        return Err(error(Failure::ResourceLimit));
                    }
                    completed.push((face, mesh));
                }
            }
        }
        if requests.is_empty() {
            return assemble(brep, completed, options, &mut work).map_err(error);
        }
        if pass == options.max_boundary_passes {
            return Err(error(Failure::UnresolvedTolerance));
        }
        work.charge(shared.edges().iter().map(|e| e.samples().len()).sum())
            .map_err(error)?;
        forced = shared
            .edges()
            .iter()
            .map(|e| e.samples().iter().map(|s| s.parameter).collect())
            .collect();
        for (edge, u) in requests {
            work.charge(1).map_err(error)?;
            forced[edge.0].push(u);
        }
        for params in &mut forced {
            params.sort_by(f64::total_cmp);
            params.dedup();
        }
    }
    unreachable!("bounded pass loop returns")
}
pub fn tessellate_shell(
    brep: &NormalizedBrep,
    shell: ShellId,
    tolerance: TessellationTolerance,
    options: TessellationOptions,
) -> Result<Mesh, TessellationError> {
    let shell = brep
        .data()
        .shells
        .get(shell.0)
        .ok_or_else(|| error(Failure::InvalidHandle))?;
    let mesh = tessellate_faces(brep, &shell.faces, tolerance, options)?;
    if shell.closed && !mesh.is_watertight() {
        return Err(error(Failure::Mesh(tessstep_mesh::Error::Open)));
    }
    if mesh.statistics().components != 1 {
        return Err(error(Failure::Mesh(tessstep_mesh::Error::Disconnected)));
    }
    Ok(mesh)
}
pub fn tessellate_solid(
    brep: &NormalizedBrep,
    solid: SolidId,
    tolerance: TessellationTolerance,
    options: TessellationOptions,
) -> Result<Mesh, TessellationError> {
    let solid = brep
        .data()
        .solids
        .get(solid.0)
        .ok_or_else(|| error(Failure::InvalidHandle))?;
    let mesh = tessellate_shell(brep, solid.shell, tolerance, options)?;
    mesh.require_solid().map_err(|e| error(Failure::Mesh(e)))?;
    Ok(mesh)
}
fn assemble(
    brep: &NormalizedBrep,
    faces: Vec<(FaceId, FaceMesh)>,
    options: TessellationOptions,
    work: &mut Work,
) -> Result<Mesh, Failure> {
    let mut data = MeshData::default();
    let mut ids = BTreeMap::<Source, u32>::new();
    for (face, mesh) in faces {
        work.charge(mesh.vertices.len() + mesh.triangles.len())?;
        let mut local = Vec::with_capacity(mesh.vertices.len());
        for v in &mesh.vertices {
            let next = data.positions.len() as u32;
            let index = *ids.entry(v.source).or_insert_with(|| {
                data.positions.push(v.position.coordinates());
                next
            });
            if data.positions[index as usize] != v.position.coordinates() {
                return Err(Failure::Geometry);
            }
            local.push(index);
        }
        let reversed = brep.data().faces[face.0].orientation.is_reversed();
        for ((mut tri, mut normals), mut uvs) in
            mesh.triangles.into_iter().zip(mesh.normals).zip(mesh.uvs)
        {
            if reversed {
                tri.swap(1, 2);
                normals.swap(1, 2);
                uvs.swap(1, 2);
            }
            data.triangles.push(tri.map(|i| local[i as usize]));
            data.uvs.push(uvs);
            data.normals.push(normals.map(|n| {
                n.vector()
                    .components()
                    .map(|x| if reversed { -x } else { x })
            }));
            data.face_ids.push(face.0 as u64);
        }
    }
    Mesh::new(data, options.mesh).map_err(Failure::Mesh)
}
enum Outcome {
    Complete(FaceMesh),
    Refine(Vec<(EdgeId, f64)>),
}
fn face_mesh(
    shared: &SharedEdges<'_>,
    face: FaceId,
    tolerance: TessellationTolerance,
    options: TessellationOptions,
    work: &mut Work,
) -> Result<Outcome, Failure> {
    let raw = shared.brep.data();
    let surface = &raw.faces[face.0].surface;
    let mapped = shared
        .face_boundary(
            face,
            options.planar.trim,
            options.planar.max_vertices.saturating_mul(2),
        )
        .map_err(Failure::Boundary)?;
    let mut vertices = Vec::new();
    let mut boundaries = Vec::new();
    let mut constraints = BTreeMap::<Key, (EdgeId, f64)>::new();
    for uses in std::iter::once(&mapped.outer).chain(&mapped.holes) {
        let first = vertices.len() as u32;
        let mut segments = Vec::new();
        for use_ in uses {
            let c = &raw.coedges[use_.coedge.0];
            let edge = &raw.edges[c.edge.0];
            let samples = shared.edges[c.edge.0].samples();
            for (j, pair) in use_.samples.windows(2).enumerate() {
                if vertices.len() >= options.planar.max_vertices {
                    return Err(Failure::ResourceLimit);
                }
                let index = if c.orientation.is_reversed() {
                    samples.len() - 1 - j
                } else {
                    j
                };
                // Every sample of a collapsed edge is its (singular) vertex.
                let source = if index == 0 || shared.brep.is_collapsed(c.edge) {
                    Source::Vertex(edge.vertices[0])
                } else if index == samples.len() - 1 {
                    Source::Vertex(edge.vertices[1])
                } else {
                    Source::Edge(c.edge, index)
                };
                let uv = pair[0].uv;
                // A singular vertex's normal depends on the direction of approach, and a
                // nearly degenerate parameterization can return an arbitrary one there.
                let singular =
                    matches!(source, Source::Vertex(v) if shared.brep.is_singular_vertex(v));
                let normal = if singular {
                    None
                } else {
                    Some(work.eval(surface, uv, [KnotSide::Right; 2])?.1)
                };
                vertices.push(Vertex {
                    uv,
                    position: pair[0].edge_sample.position,
                    normal,
                    source,
                    free: None,
                });
                let a = pair[0].edge_sample.parameter;
                let b = pair[1].edge_sample.parameter;
                let mid = a + (b - a) * 0.5;
                if mid == a || mid == b {
                    return Err(Failure::UnresolvedTolerance);
                }
                segments.push((c.edge, mid));
            }
        }
        let poly: Vec<_> = (first..vertices.len() as u32).collect();
        for (i, request) in segments.into_iter().enumerate() {
            constraints.insert(key(poly[i], poly[(i + 1) % poly.len()]), request);
        }
        boundaries.push(poly);
    }
    // Samples on collapsed edges are pole copies, free along their singular line.
    for (&(a, b), request) in &constraints {
        if shared.brep.is_collapsed(request.0) {
            let (p, q) = (vertices[a as usize].uv, vertices[b as usize].uv);
            let axis = usize::from((p[1] - q[1]).abs() > (p[0] - q[0]).abs());
            vertices[a as usize].free = Some(axis);
            vertices[b as usize].free = Some(axis);
        }
    }
    let uv: Vec<_> = vertices.iter().map(|v| v.uv).collect();
    let metric = face_metric(surface, &vertices, work)?;

    let mut triangles = crate::planar::triangulate_uv(&uv, &boundaries, options.planar, metric)
        .map_err(Failure::Polygon)?;
    for round in 0..=options.max_rounds {
        improve(&vertices, &mut triangles, &constraints, metric, work)?;
        let mut split = BTreeSet::new();
        let mut requests = Vec::new();
        for &tri in &triangles {
            work.charge(1)?;
            if acceptable(surface, tri.map(|i| vertices[i as usize]), tolerance, work)? {
                continue;
            }
            let mut longest = (0., key(tri[0], tri[1]));
            for k in 0..3 {
                let length = edge_length(&vertices, metric, tri[k], tri[(k + 1) % 3])?;
                if length > longest.0 {
                    longest = (length, key(tri[k], tri[(k + 1) % 3]));
                }
            }
            if let Some(&request) = constraints.get(&longest.1) {
                requests.push(request);
            } else {
                split.insert(longest.1);
            }
        }
        if !requests.is_empty() {
            return Ok(Outcome::Refine(requests));
        }
        if split.is_empty() {
            return finish(surface, vertices, triangles, work).map(Outcome::Complete);
        }
        if round == options.max_rounds {
            return Err(Failure::UnresolvedTolerance);
        }
        let mut midpoints = BTreeMap::new();
        // Each requested midpoint is shared by every incident triangle, preventing T-junctions.
        for (a, b) in split {
            if vertices.len() >= options.planar.max_vertices {
                return Err(Failure::ResourceLimit);
            }
            let (p, q) = (vertices[a as usize], vertices[b as usize]);
            let mut uv = [0, 1].map(|k| p.uv[k] * 0.5 + q.uv[k] * 0.5);
            // An edge from a pole copy is split on the ray to its other end.
            match (p.free, q.free) {
                (Some(k), None) => uv[k] = q.uv[k],
                (None, Some(k)) => uv[k] = p.uv[k],
                _ => {}
            }
            if uv == p.uv || uv == q.uv {
                return Err(Failure::UnresolvedTolerance);
            }
            let (position, normal) = work.eval(surface, uv, [KnotSide::Right; 2])?;
            let i = vertices.len();
            vertices.push(Vertex {
                uv,
                position,
                normal: Some(normal),
                source: Source::Interior(face, i),
                free: None,
            });
            midpoints.insert((a, b), i as u32);
        }
        let mut refined = Vec::new();
        for tri in triangles {
            let mut pieces = vec![tri];
            for k in 0..3 {
                let a = tri[k];
                let b = tri[(k + 1) % 3];
                if let Some(&m) = midpoints.get(&key(a, b)) {
                    let index = pieces
                        .iter()
                        .position(|t| t.contains(&a) && t.contains(&b))
                        .ok_or(Failure::Geometry)?;
                    let t = pieces.swap_remove(index);
                    let j = t.iter().position(|&v| v == a).unwrap();
                    if t[(j + 1) % 3] != b {
                        return Err(Failure::Geometry);
                    }
                    pieces.push([a, m, t[(j + 2) % 3]]);
                    pieces.push([m, b, t[(j + 2) % 3]]);
                }
            }
            work.charge(pieces.len())?;
            if refined.len() + pieces.len() > options.planar.max_triangles {
                return Err(Failure::ResourceLimit);
            }
            refined.extend(pieces);
        }
        triangles = refined;
    }
    unreachable!("bounded refinement loop returns")
}
/// Length of a mesh edge in the face's metric chart, or its 3D chord when it ends at
/// a pole copy, whose chart point is arbitrary along the pole line.
fn edge_length(vertices: &[Vertex], metric: [f64; 2], a: u32, b: u32) -> Result<f64, Failure> {
    let (a, b) = (vertices[a as usize], vertices[b as usize]);
    let length = if a.free.is_some() || b.free.is_some() {
        a.position
            .distance(b.position)
            .map_err(|_| Failure::Geometry)?
    } else {
        ((a.uv[0] - b.uv[0]) * metric[0]).hypot((a.uv[1] - b.uv[1]) * metric[1])
    };
    if length.is_finite() {
        Ok(length)
    } else {
        Err(Failure::Geometry)
    }
}
/// Chart parameterization of a triangle. At a single pole copy the coordinate along
/// the pole line follows the opposite edge, so the triangle is the polar wedge its 3D
/// facet spans and adjacent fans share their rays from the pole.
struct Patch {
    uv: [[f64; 2]; 3],
    polar: Option<(usize, usize)>,
}
impl Patch {
    fn new(vertices: &[Vertex; 3]) -> Self {
        let mut uv = vertices.map(|v| v.uv);
        let free: Vec<usize> = (0..3).filter(|&i| vertices[i].free.is_some()).collect();
        let polar = match free.as_slice() {
            &[s] => {
                let k = vertices[s].free.expect("free corner");
                uv[s][k] = 0.5 * (uv[(s + 1) % 3][k] + uv[(s + 2) % 3][k]);
                Some((s, k))
            }
            _ => None,
        };
        Self { uv, polar }
    }
    fn at(&self, weights: [f64; 3]) -> [f64; 2] {
        let mut p = self.linear(weights);
        if let Some((s, k)) = self.polar {
            let rest = 1. - weights[s];
            if rest > 0. {
                p[k] = (0..3)
                    .filter(|&i| i != s)
                    .map(|i| weights[i] * self.uv[i][k])
                    .sum::<f64>()
                    / rest;
            }
        }
        p
    }
    fn linear(&self, weights: [f64; 3]) -> [f64; 2] {
        [0, 1].map(|k| (0..3).map(|i| weights[i] * self.uv[i][k]).sum())
    }
}
/// Whether two corners are copies of one pole: the triangle is a UV sliver along a
/// pole or apex line whose 3D image is an edge of the neighbouring fan triangles.
fn collapsed(vertices: &[Vertex; 3]) -> bool {
    (0..3).any(|k| {
        let (a, b) = (vertices[k], vertices[(k + 1) % 3]);
        a.free.is_some() && b.free.is_some() && a.source == b.source
    })
}
/// Complete a face: collapsed slivers are dropped (their 3D image is an edge of the
/// neighbouring fan triangles) and singular corners take the limit normal from
/// inside each triangle.
fn finish(
    surface: &SurfaceGeometry,
    vertices: Vec<Vertex>,
    triangles: Vec<[u32; 3]>,
    work: &mut Work,
) -> Result<FaceMesh, Failure> {
    let mut kept = Vec::with_capacity(triangles.len());
    let mut normals = Vec::with_capacity(triangles.len());
    let mut uvs = Vec::with_capacity(triangles.len());
    for tri in triangles {
        work.charge(1)?;
        let corners = tri.map(|i| vertices[i as usize]);
        if collapsed(&corners) {
            continue;
        }
        let patch = Patch::new(&corners);
        let mut n = [None; 3];
        for k in 0..3 {
            n[k] = corners[k].normal;
            for t in [1e-6, 1e-4, 1e-2] {
                if n[k].is_some() {
                    break;
                }
                let mut weights = [t / 3.; 3];
                weights[k] = 1. - 2. * t / 3.;
                n[k] = work
                    .eval_any(surface, patch.at(weights), [KnotSide::Right; 2])?
                    .1;
            }
        }
        let [Some(a), Some(b), Some(c)] = n else {
            return Err(Failure::SingularSurface);
        };
        kept.push(tri);
        normals.push([a, b, c]);
        uvs.push(patch.uv);
    }
    Ok(FaceMesh {
        vertices,
        triangles: kept,
        normals,
        uvs,
    })
}
fn probe(
    surface: &SurfaceGeometry,
    vertices: &[Vertex; 3],
    uv: [f64; 2],
    sides: [KnotSide; 2],
    normal: Direction3<ModelSpace>,
    tolerance: TessellationTolerance,
    work: &mut Work,
) -> Result<bool, Failure> {
    let (position, n) = work.eval_any(surface, uv, sides)?;
    let chord = tolerance.chord().as_metres();
    // Geometric chord error: distance from the surface probe to the facet plane.
    // A tangential parametric offset (for example across a cone generator) is not
    // surface deviation, so the probe is not compared with its barycentric image;
    // the normal-angle check below still rejects folded or misoriented facets.
    let d = position
        .difference(vertices[0].position)
        .and_then(|v| v.dot(normal.vector()))
        .map_err(|_| Failure::Geometry)?
        .abs();
    let Some(n) = n else {
        // Only a singular corner (pole or apex) has no normal; it is a mesh vertex.
        if vertices.iter().any(|v| v.normal.is_none()) {
            return Ok(d <= chord);
        }
        return Err(Failure::SingularSurface);
    };
    Ok(d <= chord
        && n.angle_to(normal)
            .map_err(|_| Failure::Geometry)?
            .as_radians()
            <= tolerance
                .normal_angle()
                .as_radians()
                .min(std::f64::consts::FRAC_PI_2 - 1e-12))
}
fn acceptable(
    surface: &SurfaceGeometry,
    vertices: [Vertex; 3],
    tolerance: TessellationTolerance,
    work: &mut Work,
) -> Result<bool, Failure> {
    // A sliver along a pole line has no 3D extent; the fans around it cover the
    // surface there, each probed over its own polar wedge.
    if collapsed(&vertices) {
        return Ok(true);
    }
    let patch = Patch::new(&vertices);
    // Guard periodic aliasing even when a long chord has coincident endpoints.
    for (axis, domain) in surface.domain().into_iter().enumerate() {
        if let AxisDomain::Periodic { period } = domain {
            let (lo, hi) = patch
                .uv
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p[axis]), hi.max(p[axis]))
                });
            if hi - lo > period * 0.25 {
                return Ok(false);
            }
        }
    }
    let [a, b, c] = vertices.map(|v| v.position);
    let cross = b
        .difference(a)
        .and_then(|ab| c.difference(a).and_then(|ac| ab.cross(ac)))
        .map_err(|_| Failure::Geometry)?;
    let Ok(normal) = cross.normalized() else {
        return Ok(false);
    };
    for weights in [
        [1., 0., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [0.5, 0.5, 0.],
        [0., 0.5, 0.5],
        [0.5, 0., 0.5],
        [1. / 3.; 3],
        [0.5, 0.25, 0.25],
        [0.25, 0.5, 0.25],
        [0.25, 0.25, 0.5],
    ] {
        if !probe(
            surface,
            &vertices,
            patch.at(weights),
            [KnotSide::Right; 2],
            normal,
            tolerance,
            work,
        )? {
            return Ok(false);
        }
    }
    if let SurfaceGeometry::Nurbs(s) = surface {
        // Every intersected knot cell is probed, including both one-sided limits.
        // Small knot spans cannot disappear between a fixed triangle's usual probes.
        let mut cuts = [Vec::new(), Vec::new()];
        for (axis, cut) in cuts.iter_mut().enumerate() {
            let lo = patch
                .uv
                .iter()
                .map(|v| v[axis])
                .fold(f64::INFINITY, f64::min);
            let hi = patch
                .uv
                .iter()
                .map(|v| v[axis])
                .fold(f64::NEG_INFINITY, f64::max);
            let knots = s.knot_vectors()[axis].knots();
            work.charge(knots.len())?;
            cut.push(lo);
            if s.periodic_axes()[axis] {
                // Unwrapped charts repeat the knot lines once per period.
                let [a, b] = s.knot_vectors()[axis].domain();
                let period = b - a;
                let first = ((lo - a) / period).floor() as i64;
                let last = ((hi - a) / period).ceil() as i64;
                if last - first > 4 {
                    return Ok(false);
                }
                for n in first..=last {
                    work.charge(knots.len())?;
                    let shift = n as f64 * period;
                    cut.extend(
                        knots
                            .iter()
                            .map(|&k| k + shift)
                            .filter(|&k| k > lo && k < hi),
                    );
                }
                cut.sort_by(f64::total_cmp);
            } else {
                cut.extend(knots.iter().copied().filter(|&k| k > lo && k < hi));
            }
            cut.push(hi);
            cut.dedup();
        }
        for u in cuts[0].windows(2) {
            for v in cuts[1].windows(2) {
                let mut polygon = vec![[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
                for (axis, value, above) in [
                    (0, u[0], true),
                    (0, u[1], false),
                    (1, v[0], true),
                    (1, v[1], false),
                ] {
                    polygon = clip(&patch, polygon, axis, value, above, work)?;
                    if polygon.is_empty() {
                        break;
                    }
                }
                if polygon.is_empty() {
                    continue;
                }
                let center = [0, 1, 2]
                    .map(|k| polygon.iter().map(|p| p[k]).sum::<f64>() / polygon.len() as f64);
                let check = |p: [f64; 3], work: &mut Work| -> Result<bool, Failure> {
                    let uv = patch.linear(p);
                    let sides = [
                        if uv[0] >= u[1] {
                            KnotSide::Left
                        } else {
                            KnotSide::Right
                        },
                        if uv[1] >= v[1] {
                            KnotSide::Left
                        } else {
                            KnotSide::Right
                        },
                    ];
                    probe(surface, &vertices, uv, sides, normal, tolerance, work)
                };
                if !check(center, work)? {
                    return Ok(false);
                }
                for i in 0..polygon.len() {
                    let a = polygon[i];
                    let b = polygon[(i + 1) % polygon.len()];
                    if !check(a, work)? || !check([0, 1, 2].map(|k| (a[k] + b[k]) * 0.5), work)? {
                        return Ok(false);
                    }
                }
            }
        }
    }
    Ok(true)
}
fn clip(
    patch: &Patch,
    polygon: Vec<[f64; 3]>,
    axis: usize,
    value: f64,
    above: bool,
    work: &mut Work,
) -> Result<Vec<[f64; 3]>, Failure> {
    work.charge(polygon.len())?;
    let mut out = Vec::new();
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        let av = patch.linear(a)[axis];
        let bv = patch.linear(b)[axis];
        let inside = |v: f64| if above { v >= value } else { v <= value };
        if inside(av) {
            out.push(a);
        }
        if inside(av) != inside(bv) {
            let t = (value - av) / (bv - av);
            if !t.is_finite() {
                return Err(Failure::Geometry);
            }
            out.push([0, 1, 2].map(|k| a[k] + t * (b[k] - a[k])));
        }
    }
    Ok(out)
}
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn improve(
    vertices: &[Vertex],
    triangles: &mut [[u32; 3]],
    constraints: &BTreeMap<Key, (EdgeId, f64)>,
    metric: [f64; 2],
    work: &mut Work,
) -> Result<(), Failure> {
    // Queue-based Lawson flips in the metric-scaled UV chart: only edges around a
    // flip are revisited, so an ear-clipping fan unwinds in one pass instead of one
    // sweep per fan triangle. Constraints are never flipped. A strict angular margin
    // avoids cycling on cocircular floating-point inputs, and the flip count is
    // bounded; shape improvement is optional because the error checks still run.
    let mut incident = BTreeMap::<Key, Vec<usize>>::new();
    for (i, t) in triangles.iter().enumerate() {
        work.charge(3)?;
        for k in 0..3 {
            incident
                .entry(key(t[k], t[(k + 1) % 3]))
                .or_default()
                .push(i);
        }
    }
    let mut pending: Vec<Key> = incident
        .iter()
        .filter(|(edge, tris)| tris.len() == 2 && !constraints.contains_key(edge))
        .map(|(&edge, _)| edge)
        .collect();
    let mut queued: BTreeSet<Key> = pending.iter().copied().collect();
    let mut flips = 0usize;
    let limit = triangles.len().saturating_mul(64).saturating_add(1024);
    while let Some(edge) = pending.pop() {
        work.charge(1)?;
        queued.remove(&edge);
        let Some(tris) = incident.get(&edge) else {
            continue;
        };
        if tris.len() != 2 {
            continue;
        }
        let (i, j) = (tris[0], tris[1]);
        // Orient the shared edge a->b as it appears in triangle i.
        let ti = triangles[i];
        let Some(k) = (0..3).find(|&k| key(ti[k], ti[(k + 1) % 3]) == edge) else {
            return Err(Failure::Geometry);
        };
        let (a, b, c) = (ti[k], ti[(k + 1) % 3], ti[(k + 2) % 3]);
        let Some(&d) = triangles[j].iter().find(|&&v| v != a && v != b) else {
            return Err(Failure::Geometry);
        };
        if c == d {
            continue;
        }
        // Chart geometry at a pole copy is arbitrary along its pole line.
        if [a, b, c, d]
            .iter()
            .any(|&i| vertices[i as usize].free.is_some())
        {
            continue;
        }
        let [pa, pb, pc, pd] = [a, b, c, d].map(|i| {
            let uv = vertices[i as usize].uv;
            [uv[0] * metric[0], uv[1] * metric[1]]
        });
        let x = orient(pc, pd, pb);
        let y = orient(pd, pc, pa);
        if !x.is_finite() || !y.is_finite() {
            return Err(Failure::Geometry);
        }
        if x <= 0. || y <= 0. {
            continue;
        }
        let angle = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
            let a = [p[0] - r[0], p[1] - r[1]];
            let b = [q[0] - r[0], q[1] - r[1]];
            let cross = a[0] * b[1] - a[1] * b[0];
            let dot = a[0] * b[0] + a[1] * b[1];
            if !cross.is_finite() || !dot.is_finite() {
                return Err(Failure::Geometry);
            }
            Ok(cross.abs().atan2(dot))
        };
        let sum = angle(pa, pb, pc)? + angle(pa, pb, pd)?;
        if !sum.is_finite() {
            return Err(Failure::Geometry);
        }
        if sum <= std::f64::consts::PI + 1e-12 || incident.contains_key(&key(c, d)) {
            continue;
        }
        if flips >= limit {
            break;
        }
        flips += 1;
        work.charge(8)?;
        triangles[i] = [c, d, b];
        triangles[j] = [d, c, a];
        incident.remove(&edge);
        incident.insert(key(c, d), vec![i, j]);
        for (e, from, to) in [(key(d, b), j, i), (key(c, a), i, j)] {
            if let Some(list) = incident.get_mut(&e) {
                for t in list.iter_mut() {
                    if *t == from {
                        *t = to;
                    }
                }
            }
        }
        for e in [key(b, c), key(c, a), key(a, d), key(d, b)] {
            if !constraints.contains_key(&e) && queued.insert(e) {
                pending.push(e);
            }
        }
    }
    Ok(())
}
