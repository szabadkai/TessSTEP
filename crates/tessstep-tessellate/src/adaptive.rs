use crate::{
    BoundaryError, PlanarError, PlanarOptions, SamplingLimits, SharedEdges, metric,
    sample_edges_seeded,
};
use std::collections::{BTreeMap, BTreeSet};
use tessstep_curves::spline::KnotSide;
use tessstep_math::{Direction3, ModelSpace, NumericalTolerance, Point3, TessellationTolerance};
use tessstep_mesh::{Mesh, MeshData};
use tessstep_surfaces::{AxisDomain, Evaluation};
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
    /// Threads that mesh the faces of one boundary pass (1, the default, meshes them
    /// in order on the calling thread). Results, budgets and diagnostics do not depend
    /// on it: faces are meshed against the pass's remaining budgets and their charges
    /// are then applied in face order; if any budget would have run out, the call is
    /// repeated on one thread.
    pub threads: usize,
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
            threads: 1,
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
    /// A cavity shell does not face into its cavity (non-negative volume), or the
    /// cavities outweigh the outer shell.
    CavityOrientation,
    Sampling(crate::Error),
    Boundary(BoundaryError),
    Polygon(PlanarError),
    Mesh(tessstep_mesh::Error),
}
/// A tessellation budget and its configured value, named in the diagnostic of the
/// failure that exhausted it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// The option's path in [`TessellationOptions`], for example `planar.max_work`.
    pub name: &'static str,
    pub limit: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TessellationError {
    pub face: Option<FaceId>,
    pub kind: TessellationErrorKind,
    /// The exhausted budget, for resource limits and for refinement rounds or boundary
    /// passes that ran out (`UnresolvedTolerance`).
    pub budget: Option<Budget>,
}
impl std::fmt::Display for TessellationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tessellation {:?}: {:?}", self.face, self.kind)?;
        if let Some(b) = self.budget {
            write!(f, " (budget {} {} exhausted)", b.name, b.limit)?;
        }
        Ok(())
    }
}
impl std::error::Error for TessellationError {}
fn error(kind: TessellationErrorKind) -> TessellationError {
    TessellationError {
        face: None,
        kind,
        budget: None,
    }
}
fn budget(name: &'static str, limit: usize) -> Option<Budget> {
    Some(Budget { name, limit })
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
    /// The surface at `uv`. A canonical boundary sample may sit off it by up to the
    /// model tolerance; facets are measured through the lifted corners.
    lifted: Point3<ModelSpace>,
    /// None only at singular vertices (poles, apices), whose normal depends on the
    /// direction of approach; triangles evaluate it from inside at completion.
    normal: Option<Direction3<ModelSpace>>,
    source: Source,
    /// For a sample of a collapsed edge (a pole copy), the chart axis along its
    /// singular line: its coordinate there is arbitrary, so triangles use the one of
    /// their opposite edge.
    free: Option<usize>,
    /// Curvature metric and its 3D density (see [`metric`]); None where singular.
    metric: Option<(metric::Tensor, f64)>,
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
    options: TessellationOptions,
    /// The budget behind the pending `ResourceLimit`, recorded where it is exhausted.
    exhausted: Option<Budget>,
}
impl Work {
    fn new(options: TessellationOptions) -> Self {
        Self {
            remaining: options.max_work,
            evaluations: options.max_evaluations,
            options,
            exhausted: None,
        }
    }
    /// Record an exhausted budget and fail with a resource limit.
    fn exhaust(&mut self, name: &'static str, limit: usize) -> Failure {
        self.exhausted = budget(name, limit);
        Failure::ResourceLimit
    }
    fn charge(&mut self, n: usize) -> Result<(), Failure> {
        match self.remaining.checked_sub(n) {
            Some(rest) => self.remaining = rest,
            None => return Err(self.exhaust("max_work", self.options.max_work)),
        }
        Ok(())
    }
    fn evaluate(&mut self, n: usize) -> Result<(), Failure> {
        match self.evaluations.checked_sub(n) {
            Some(rest) => self.evaluations = rest,
            None => return Err(self.exhaust("max_evaluations", self.options.max_evaluations)),
        }
        Ok(())
    }
    /// Position and, unless the parameterization is singular there, the normal.
    fn eval_any(
        &mut self,
        surface: &SurfaceGeometry,
        uv: [f64; 2],
        sides: [KnotSide; 2],
    ) -> Result<(Point3<ModelSpace>, Option<Direction3<ModelSpace>>), Failure> {
        let jet = self.jet(surface, uv, sides)?;
        Ok((jet.position, jet.normal(NumericalTolerance::default()).ok()))
    }
    fn jet(
        &mut self,
        surface: &SurfaceGeometry,
        uv: [f64; 2],
        sides: [KnotSide; 2],
    ) -> Result<Evaluation<ModelSpace>, Failure> {
        self.charge(1)?;
        self.evaluate(1)?;
        match surface {
            SurfaceGeometry::Analytic(s) => s.evaluate(uv[0], uv[1]),
            SurfaceGeometry::Nurbs(s) => s.evaluate_on_sides(uv[0], uv[1], sides),
        }
        .map_err(|_| Failure::Geometry)
    }
}
struct RegularPoint {
    position: Point3<ModelSpace>,
    normal: Direction3<ModelSpace>,
    metric: Option<(metric::Tensor, f64)>,
}
impl RegularPoint {
    fn new(jet: &Evaluation<ModelSpace>, sizing: metric::Sizing) -> Result<Self, Failure> {
        let normal = jet
            .normal(NumericalTolerance::default())
            .map_err(|_| Failure::SingularSurface)?;
        Ok(Self {
            position: jet.position,
            normal,
            metric: metric::curvature(jet, normal, sizing),
        })
    }
}
/// Surface evaluations one face mesh reuses: the right-sided jet at each probed edge
/// midpoint, keyed by edge with its chart point. Both facets on an edge probe that
/// point, and a split of the edge places its vertex there.
#[derive(Default)]
struct Probes {
    midpoints: BTreeMap<Key, ([f64; 2], Evaluation<ModelSpace>)>,
}
/// What a face keeps across boundary passes, keyed by exact chart coordinates: the
/// jets of its vertices and the facets (between regular corners) that passed.
/// Meshing a face again after a boundary pass mostly reproduces its interior, and an
/// evaluation or an acceptance test depends only on those coordinates, so reuse
/// returns exactly what repeating them would.
#[derive(Default)]
struct FaceMemory {
    jets: BTreeMap<[u64; 2], Evaluation<ModelSpace>>,
    passed: BTreeSet<[[u64; 2]; 3]>,
}
impl FaceMemory {
    /// A regular vertex at `uv`, evaluated once per face.
    fn regular(
        &mut self,
        surface: &SurfaceGeometry,
        uv: [f64; 2],
        sizing: metric::Sizing,
        work: &mut Work,
    ) -> Result<RegularPoint, Failure> {
        let key = uv.map(f64::to_bits);
        if let Some(jet) = self.jets.get(&key) {
            work.charge(1)?;
            return RegularPoint::new(jet, sizing);
        }
        let jet = work.jet(surface, uv, [KnotSide::Right; 2])?;
        self.jets.insert(key, jet);
        RegularPoint::new(&jet, sizing)
    }
}
/// The chart points of a facet between regular corners, rotated to start at the
/// smallest; `None` when a corner is singular or a pole copy.
fn chart_key(corners: &[Vertex; 3]) -> Option<[[u64; 2]; 3]> {
    if corners
        .iter()
        .any(|v| v.normal.is_none() || v.free.is_some())
    {
        return None;
    }
    let keys = corners.map(|v| v.uv.map(f64::to_bits));
    let k = (0..3).min_by_key(|&k| keys[k]).expect("three corners");
    Some([keys[k], keys[(k + 1) % 3], keys[(k + 2) % 3]])
}
impl Work {
    /// Parametric speeds |S_u| and |S_v| at a UV point.
    fn speeds(&mut self, surface: &SurfaceGeometry, uv: [f64; 2]) -> Result<[f64; 2], Failure> {
        self.charge(1)?;
        self.evaluate(1)?;
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
// The error carries the located trim failure and the exhausted budget; it is
// returned once per call, so its size is not on a hot path.
#[allow(clippy::result_large_err)]
pub fn tessellate_faces(
    brep: &NormalizedBrep,
    faces: &[FaceId],
    tolerance: TessellationTolerance,
    options: TessellationOptions,
) -> Result<Mesh, TessellationError> {
    if options.threads > 1 {
        let mut speculative = false;
        let result = run(
            brep,
            faces,
            tolerance,
            options,
            options.threads,
            &mut speculative,
        );
        if !speculative {
            return result;
        }
    }
    run(brep, faces, tolerance, options, 1, &mut false)
}
/// One face meshed against the pass's remaining budgets.
struct Job {
    face: FaceId,
    memory: FaceMemory,
    work: Work,
    result: Option<Result<Outcome, Failure>>,
}
/// Mesh the faces of `jobs` on `threads` threads, each against its own budgets.
fn mesh_jobs(
    shared: &SharedEdges<'_>,
    jobs: &mut [Job],
    tolerance: TessellationTolerance,
    options: TessellationOptions,
    threads: usize,
) {
    let next = std::sync::Mutex::new(jobs.iter_mut());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let job = match next.lock() {
                        Ok(mut jobs) => jobs.next(),
                        Err(_) => None,
                    };
                    let Some(job) = job else { break };
                    job.result = Some(face_mesh(
                        shared,
                        job.face,
                        tolerance,
                        options,
                        &mut job.memory,
                        &mut job.work,
                    ));
                }
            });
        }
    });
}
/// The boundary passes. With `threads > 1` each pass meshes its faces in parallel and
/// sets `speculative` (returning early) when applying their charges in face order
/// shows that a budget would have run out, which only a run on one thread places.
#[allow(clippy::result_large_err)]
fn run(
    brep: &NormalizedBrep,
    faces: &[FaceId],
    tolerance: TessellationTolerance,
    options: TessellationOptions,
    threads: usize,
    speculative: &mut bool,
) -> Result<Mesh, TessellationError> {
    if options.max_rounds > 48
        || options.max_boundary_passes > 24
        || options.planar.max_vertices > 1_000_000
        || options.planar.max_triangles > 2_000_000
    {
        return Err(error(Failure::InvalidOptions));
    }
    let mut work = Work::new(options);
    // Every failure carries the budget that ran out, when one did.
    let fail = |work: &mut Work, face: Option<FaceId>, kind: Failure| TessellationError {
        budget: work.exhausted.take().or_else(|| match &kind {
            // Vertex and triangle counts are checked before assembly; the rest is
            // the mesh validation work.
            Failure::Mesh(tessstep_mesh::Error::ResourceLimit) => {
                budget("mesh.max_work", options.mesh.max_work)
            }
            _ => None,
        }),
        face,
        kind,
    };
    work.charge(faces.len())
        .map_err(|k| fail(&mut work, None, k))?;
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
    // Faces completed in the previous pass, reused while their edges' samples are
    // unchanged: meshing is deterministic, so a remesh would reproduce them exactly.
    let mut cached = BTreeMap::<FaceId, FaceMesh>::new();
    let mut memory = BTreeMap::<FaceId, FaceMemory>::new();
    let mut previous: Vec<Vec<f64>> = Vec::new();
    for pass in 0..=options.max_boundary_passes {
        let mut sampling = options.sampling;
        sampling.max_evaluations = sampling.max_evaluations.min(work.evaluations);
        let mut short = None;
        let shared =
            sample_edges_seeded(brep, tolerance, sampling, &forced, &mut short).map_err(|e| {
                work.exhausted = match short {
                    Some(crate::Exhausted::Samples) => {
                        budget("sampling.max_samples", options.sampling.max_samples)
                    }
                    Some(crate::Exhausted::Evaluations)
                        if options.sampling.max_evaluations <= work.evaluations =>
                    {
                        budget("sampling.max_evaluations", options.sampling.max_evaluations)
                    }
                    Some(crate::Exhausted::Evaluations) => {
                        budget("max_evaluations", options.max_evaluations)
                    }
                    None => None,
                };
                fail(&mut work, None, Failure::Sampling(e))
            })?;
        work.evaluate(shared.evaluations())
            .map_err(|k| fail(&mut work, None, k))?;
        work.charge(shared.edges().iter().map(|e| e.samples().len()).sum())
            .map_err(|k| fail(&mut work, None, k))?;
        let parameters: Vec<Vec<f64>> = shared
            .edges()
            .iter()
            .map(|e| e.samples().iter().map(|s| s.parameter).collect())
            .collect();
        let unchanged = |edge: EdgeId| previous.get(edge.0) == parameters.get(edge.0);
        let mut requests = Vec::new();
        let mut completed = Vec::new();
        let mut vertices = 0usize;
        let mut triangles = 0usize;
        let reused: Vec<Option<FaceMesh>> = faces
            .iter()
            .map(|face| {
                cached
                    .remove(face)
                    .filter(|_| face_edges(brep, *face).all(unchanged))
            })
            .collect();
        // Parallel passes mesh every other face first, each against the budgets left
        // now; the charges are applied in face order below.
        let mut jobs: BTreeMap<FaceId, Job> = BTreeMap::new();
        let start = (work.remaining, work.evaluations);
        let pending = reused.iter().filter(|r| r.is_none()).count();
        if threads > 1 && pending > 1 {
            let mut list: Vec<Job> = faces
                .iter()
                .zip(&reused)
                .filter(|(_, r)| r.is_none())
                .map(|(&face, _)| Job {
                    face,
                    memory: memory.remove(&face).unwrap_or_default(),
                    work: Work {
                        exhausted: None,
                        ..work
                    },
                    result: None,
                })
                .collect();
            mesh_jobs(&shared, &mut list, tolerance, options, threads.min(pending));
            jobs = list.into_iter().map(|job| (job.face, job)).collect();
        }
        for (&face, reused) in faces.iter().zip(reused) {
            let outcome = match reused {
                Some(mesh) => Outcome::Complete(mesh),
                None => match jobs.remove(&face) {
                    Some(job) => {
                        // The pass's budgets the job started from, less what it left.
                        let (spent, evaluated) =
                            (start.0 - job.work.remaining, start.1 - job.work.evaluations);
                        memory.insert(face, job.memory);
                        // A face that ran out of its budgets, or whose charges exceed
                        // what the faces before it left, is placed by one thread.
                        if job.work.exhausted.is_some()
                            || spent > work.remaining
                            || evaluated > work.evaluations
                        {
                            *speculative = true;
                            return Err(error(Failure::ResourceLimit));
                        }
                        work.remaining -= spent;
                        work.evaluations -= evaluated;
                        job.result
                            .expect("meshed")
                            .map_err(|kind| fail(&mut work, Some(face), kind))?
                    }
                    None => {
                        let memory = memory.entry(face).or_default();
                        face_mesh(&shared, face, tolerance, options, memory, &mut work)
                            .map_err(|kind| fail(&mut work, Some(face), kind))?
                    }
                },
            };
            match outcome {
                Outcome::Refine(mut needs) => requests.append(&mut needs),
                Outcome::Complete(mesh) => {
                    vertices = vertices.saturating_add(mesh.vertices.len());
                    triangles = triangles.saturating_add(mesh.triangles.len());
                    let max_vertices = options.mesh.max_vertices.min(u32::MAX as usize);
                    if vertices > max_vertices {
                        let kind = work.exhaust("mesh.max_vertices", max_vertices);
                        return Err(fail(&mut work, None, kind));
                    }
                    if triangles > options.mesh.max_triangles {
                        let kind = work.exhaust("mesh.max_triangles", options.mesh.max_triangles);
                        return Err(fail(&mut work, None, kind));
                    }
                    completed.push((face, mesh));
                }
            }
        }
        if requests.is_empty() {
            return assemble(brep, completed, options, &mut work)
                .map_err(|k| fail(&mut work, None, k));
        }
        if pass == options.max_boundary_passes {
            work.exhausted = budget("max_boundary_passes", options.max_boundary_passes);
            return Err(fail(&mut work, None, Failure::UnresolvedTolerance));
        }
        cached = completed.into_iter().collect();
        forced = parameters.clone();
        previous = parameters;
        for (edge, u) in requests {
            work.charge(1).map_err(|k| fail(&mut work, None, k))?;
            forced[edge.0].push(u);
        }
        for params in &mut forced {
            params.sort_by(f64::total_cmp);
            params.dedup();
        }
    }
    unreachable!("bounded pass loop returns")
}
/// The edges bounding a face.
fn face_edges(brep: &NormalizedBrep, face: FaceId) -> impl Iterator<Item = EdgeId> + '_ {
    let raw = brep.data();
    let f = &raw.faces[face.0];
    std::iter::once(f.outer)
        .chain(f.holes.iter().copied())
        .flat_map(move |wire| raw.wires[wire.0].coedges.iter())
        .map(move |c| raw.coedges[c.0].edge)
}
// The error carries the located trim failure and the exhausted budget; it is
// returned once per call, so its size is not on a hot path.
#[allow(clippy::result_large_err)]
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
/// A closed oriented mesh of a solid: one component per boundary shell, a positive
/// outer volume and a negative volume for every cavity shell (which faces into its
/// cavity). Cavity containment and disjointness are not checked.
// The error carries the located trim failure and the exhausted budget; it is
// returned once per call, so its size is not on a hot path.
#[allow(clippy::result_large_err)]
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
    if solid.voids.is_empty() {
        let mesh = tessellate_shell(brep, solid.shell, tolerance, options)?;
        mesh.require_solid().map_err(|e| error(Failure::Mesh(e)))?;
        return Ok(mesh);
    }
    let shells = brep.data();
    let mut owner = BTreeMap::new();
    let mut faces = Vec::new();
    for (k, shell) in std::iter::once(&solid.shell)
        .chain(&solid.voids)
        .enumerate()
    {
        let shell = shells
            .shells
            .get(shell.0)
            .ok_or_else(|| error(Failure::InvalidHandle))?;
        for &face in &shell.faces {
            owner.insert(face.0 as u64, k);
            faces.push(face);
        }
    }
    let mesh = tessellate_faces(brep, &faces, tolerance, options)?;
    if !mesh.is_watertight() {
        return Err(error(Failure::Mesh(tessstep_mesh::Error::Open)));
    }
    if mesh.statistics().components != 1 + solid.voids.len() {
        return Err(error(Failure::Mesh(tessstep_mesh::Error::Disconnected)));
    }
    // Per-shell algebraic volume, relative to the first position like the mesh total.
    let d = mesh.data();
    let origin = d.positions[0];
    let mut volumes = vec![0.; 1 + solid.voids.len()];
    for (tri, face) in d.triangles.iter().zip(&d.face_ids) {
        let [a, b, c] = tri.map(|i| {
            let p = d.positions[i as usize];
            [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]]
        });
        let cross = [
            b[1] * c[2] - b[2] * c[1],
            b[2] * c[0] - b[0] * c[2],
            b[0] * c[1] - b[1] * c[0],
        ];
        volumes[owner[face]] += (a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]) / 6.;
    }
    if volumes.iter().any(|v| !v.is_finite()) {
        return Err(error(Failure::Mesh(tessstep_mesh::Error::NonFinite)));
    }
    if volumes[0] <= 0. {
        return Err(error(Failure::Mesh(
            tessstep_mesh::Error::NonPositiveVolume,
        )));
    }
    if volumes[1..].iter().any(|&v| v >= 0.) || mesh.statistics().signed_volume <= 0. {
        return Err(error(Failure::CavityOrientation));
    }
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
mod pole;
fn face_mesh(
    shared: &SharedEdges<'_>,
    face: FaceId,
    tolerance: TessellationTolerance,
    options: TessellationOptions,
    memory: &mut FaceMemory,
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
        .map_err(|e| {
            work.exhausted = match &e {
                BoundaryError::ResourceLimit => {
                    budget("planar.max_vertices", options.planar.max_vertices)
                }
                BoundaryError::Trim(t) if t.kind == tessstep_trim::ErrorKind::Limit => {
                    trim_budget(shared.brep, face, options.planar.trim)
                }
                _ => None,
            };
            Failure::Boundary(e)
        })?;
    let sizing = sizing(&mapped, tolerance, shared.brep.model_tolerance())?;
    let mut vertices = Vec::new();
    let mut boundaries = Vec::new();
    let mut constraints = BTreeMap::<Key, (EdgeId, f64)>::new();
    let mut poles = Vec::new();
    for uses in std::iter::once(&mapped.outer).chain(&mapped.holes) {
        let first = vertices.len() as u32;
        let mut segments = Vec::new();
        for use_ in uses {
            let c = &raw.coedges[use_.coedge.0];
            let edge = &raw.edges[c.edge.0];
            let samples = shared.edges[c.edge.0].samples();
            for (j, pair) in use_.samples.windows(2).enumerate() {
                if vertices.len() >= options.planar.max_vertices {
                    return Err(work.exhaust("planar.max_vertices", options.planar.max_vertices));
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
                let position = pair[0].edge_sample.position;
                let (normal, metric, lifted) = if singular {
                    (None, None, position)
                } else {
                    let point = memory.regular(surface, uv, sizing, work)?;
                    (Some(point.normal), point.metric, point.position)
                };
                vertices.push(Vertex {
                    uv,
                    position,
                    lifted,
                    normal,
                    source,
                    free: None,
                    metric,
                });
                let a = pair[0].edge_sample.parameter;
                let b = pair[1].edge_sample.parameter;
                let mid = a + (b - a) * 0.5;
                if mid == a || mid == b {
                    return Err(Failure::UnresolvedTolerance);
                }
                segments.push((c.edge, mid));
                if shared.brep.is_collapsed(c.edge) {
                    poles.push(PoleSegment {
                        pole: source,
                        edge: c.edge,
                        parameters: [a, b],
                        uv: [pair[0].uv, pair[1].uv],
                    });
                }
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
    let metric = face_metric(surface, &vertices, work)?;
    let boundary_rays: BTreeMap<_, _> = constraints
        .iter()
        .filter_map(|(&(a, b), &request)| Some((ray(&vertices, a, b)?, request)))
        .collect();
    // Pole runs get explicit monotone fans; the rest is ear clipped. A ring that
    // leaves an invalid remainder falls back to the plain triangulation.
    let boundary_count = vertices.len();
    let pole::Caps {
        mut triangles,
        loops,
    } = pole::caps(
        surface,
        face,
        &mut vertices,
        &boundaries,
        sizing,
        options.planar.max_vertices,
        memory,
        work,
    )?;
    let uv: Vec<_> = vertices.iter().map(|v| v.uv).collect();
    match crate::planar::triangulate_uv(&uv, &loops, options.planar, metric) {
        Ok(rest) => triangles.extend(rest),
        Err(_) if !triangles.is_empty() => {
            vertices.truncate(boundary_count);
            let uv: Vec<_> = vertices.iter().map(|v| v.uv).collect();
            triangles = crate::planar::triangulate_uv(&uv, &boundaries, options.planar, metric)
                .map_err(|e| polygon_failure(e, &boundaries, options.planar, work))?;
        }
        Err(e) => return Err(polygon_failure(e, &loops, options.planar, work)),
    }
    let resolution = shared.brep.model_tolerance();
    let mut requested = BTreeSet::new();
    let mut passed = BTreeSet::new();
    let mut probes = Probes::default();
    // Whether each triangle is known to pass, and which ones the last split created.
    // Triangles keep their slots across rounds (a split's first piece takes its
    // parent's), so the edge incidence is maintained rather than rebuilt.
    let mut known = vec![false; triangles.len()];
    let mut fresh: Option<Vec<bool>> = None;
    let mut incident = EdgeMap::<Incident>::default();
    incident.reserve(triangles.len() * 3 / 2 + 1);
    for slot in 0..triangles.len() {
        work.charge(3)?;
        attach(&mut incident, &triangles, slot);
    }
    for round in 0..=options.max_rounds {
        let emptied = improve(
            &vertices,
            &mut triangles,
            &mut incident,
            &constraints,
            metric,
            fresh.as_deref(),
            &mut known,
            work,
        )?;
        let mut split = BTreeSet::new();
        let mut requests = Vec::new();
        let mut folded = false;
        for (slot, &tri) in triangles.iter().enumerate() {
            work.charge(1)?;
            if known[slot] {
                continue;
            }
            // A facet that passed keeps passing: its corners determine every probe.
            let corners = canonical(tri);
            let chart = chart_key(&tri.map(|i| vertices[i as usize]));
            if passed.contains(&corners) || chart.is_some_and(|c| memory.passed.contains(&c)) {
                known[slot] = true;
                continue;
            }
            if acceptable(
                surface,
                tri.map(|i| vertices[i as usize]),
                tri,
                tolerance,
                resolution,
                &mut probes,
                work,
            )? {
                passed.insert(corners);
                if let Some(c) = chart {
                    memory.passed.insert(c);
                }
                known[slot] = true;
                continue;
            }
            if let Some(request) = fold(tri.map(|i| vertices[i as usize]), &poles) {
                requests.push(request);
                folded = true;
                continue;
            }
            let mut longest = (0., key(tri[0], tri[1]));
            for k in 0..3 {
                let length = edge_length(&vertices, sizing, tri[k], tri[(k + 1) % 3])?;
                if length > longest.0 {
                    longest = (length, key(tri[k], tri[(k + 1) % 3]));
                }
            }
            // A ray from a pole that a boundary segment also runs along is refined on
            // the boundary: its interior copies would otherwise split off a sliver.
            let ray = ray(&vertices, longest.1.0, longest.1.1);
            if let Some(&request) = constraints
                .get(&longest.1)
                .or_else(|| ray.and_then(|r| boundary_rays.get(&r)))
            {
                requests.push(request);
            } else {
                split.insert(longest.1);
            }
        }
        // Boundary requests wait until the interior has converged or no further
        // boundary segment is requested, so that one boundary pass resolves every
        // facet the coarse boundary leaves failing; facets beside a requested segment
        // would otherwise keep splitting into slivers along it. A folded fan only
        // arises in the initial triangulation, but splits of its neighbours would
        // propagate the fold, so it restarts the pass at once.
        let now: BTreeSet<(EdgeId, u64)> = requests.iter().map(|r| (r.0, r.1.to_bits())).collect();
        let settled = !now.is_empty() && now == requested;
        requested = now;
        if split.is_empty() || round == options.max_rounds || folded || settled {
            if !requests.is_empty() {
                return Ok(Outcome::Refine(requests));
            }
            if split.is_empty() {
                return finish(surface, vertices, triangles, work).map(Outcome::Complete);
            }
            work.exhausted = budget("max_rounds", options.max_rounds);
            return Err(Failure::UnresolvedTolerance);
        }
        let mut midpoints = BTreeMap::new();
        // Edges from different copies of one pole to the same vertex are one 3D edge
        // (the pole's ray to that vertex), split at one shared midpoint.
        let mut rays = BTreeMap::new();
        // Each requested midpoint is shared by every incident triangle, preventing T-junctions.
        for (a, b) in split {
            let ray = ray(&vertices, a, b);
            if let Some(&m) = ray.and_then(|r| rays.get(&r)) {
                midpoints.insert((a, b), m);
                continue;
            }
            if vertices.len() >= options.planar.max_vertices {
                return Err(work.exhaust("planar.max_vertices", options.planar.max_vertices));
            }
            let (p, q) = (vertices[a as usize], vertices[b as usize]);
            // An edge from a pole copy is split on the ray to its other end.
            let uv = match (p.free, q.free) {
                (Some(k), None) => ray_midpoint(surface, p, q, k, work)?,
                (None, Some(k)) => ray_midpoint(surface, q, p, k, work)?,
                _ => [0, 1].map(|k| p.uv[k] * 0.5 + q.uv[k] * 0.5),
            };
            if uv == p.uv || uv == q.uv {
                return Err(Failure::UnresolvedTolerance);
            }
            let point = match probes.midpoints.get(&key(a, b)) {
                Some((at, jet)) if *at == uv => {
                    memory.jets.insert(uv.map(f64::to_bits), *jet);
                    RegularPoint::new(jet, sizing)?
                }
                _ => memory.regular(surface, uv, sizing, work)?,
            };
            let i = vertices.len();
            vertices.push(Vertex {
                uv,
                position: point.position,
                lifted: point.position,
                normal: Some(point.normal),
                source: Source::Interior(face, i),
                free: None,
                metric: point.metric,
            });
            midpoints.insert((a, b), i as u32);
            if let Some(r) = ray {
                rays.insert(r, i as u32);
            }
        }
        let mut created = vec![false; triangles.len()];
        for slot in 0..triangles.len() {
            let tri = triangles[slot];
            let mut pieces = vec![tri];
            for k in 0..3 {
                let a = tri[k];
                let b = tri[(k + 1) % 3];
                let edge = key(a, b);
                let shared = || {
                    let r = ray(&vertices, a, b)?;
                    (!constraints.contains_key(&edge)).then(|| rays.get(&r))?
                };
                if let Some(&m) = midpoints.get(&edge).or_else(shared) {
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
            // Both pole edges of a collapsed sliver share their ray's midpoint, which
            // leaves pieces with a repeated corner; the sliver's rest covers them.
            pieces.retain(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0]);
            if pieces.len() == 1 && pieces[0] == tri {
                continue;
            }
            work.charge(3 * (pieces.len() + 1))?;
            if triangles.len() + pieces.len() > options.planar.max_triangles + 1 {
                return Err(work.exhaust("planar.max_triangles", options.planar.max_triangles));
            }
            detach(&mut incident, &triangles, slot);
            let mut slots = Vec::with_capacity(pieces.len());
            for (n, piece) in pieces.into_iter().enumerate() {
                let at = if n == 0 {
                    triangles[slot] = piece;
                    slot
                } else {
                    triangles.push(piece);
                    known.push(false);
                    created.push(false);
                    triangles.len() - 1
                };
                known[at] = false;
                created[at] = true;
                slots.push(at);
            }
            // At most two edges of a triangle share a ray midpoint, so a piece remains.
            if slots.is_empty() {
                return Err(Failure::Geometry);
            }
            for at in slots {
                attach(&mut incident, &triangles, at);
            }
        }
        if triangles.len() > options.planar.max_triangles {
            return Err(work.exhaust("planar.max_triangles", options.planar.max_triangles));
        }
        // After a flip ceiling the next call starts from the full queue again.
        fresh = emptied.then_some(created);
    }
    unreachable!("bounded refinement loop returns")
}
/// Names the budget behind a planar triangulation failure: the triangle count is
/// checked up front, everything else (validation, bridges, ears) charges `max_work`.
fn polygon_failure(
    e: PlanarError,
    loops: &[Vec<u32>],
    planar: PlanarOptions,
    work: &mut Work,
) -> Failure {
    let limit = match &e {
        PlanarError::ResourceLimit => true,
        PlanarError::InvalidPolygon(t) => t.kind == tessstep_trim::ErrorKind::Limit,
        _ => false,
    };
    if limit {
        let expected = loops.iter().map(Vec::len).sum::<usize>() + 2 * loops.len();
        work.exhausted = if expected.saturating_sub(4) > planar.max_triangles {
            budget("planar.max_triangles", planar.max_triangles)
        } else {
            budget("planar.max_work", planar.max_work)
        };
    }
    Failure::Polygon(e)
}
/// Trim reconstruction reports one `Limit` for its sample and work budgets; a rerun
/// with the sample ceiling raised tells them apart (only on this failure path).
fn trim_budget(
    brep: &NormalizedBrep,
    face: FaceId,
    trim: tessstep_trim::Options,
) -> Option<Budget> {
    let raised = tessstep_trim::Options {
        max_samples: 1_000_000,
        ..trim
    };
    let samples = trim.max_samples < raised.max_samples
        && !matches!(
            tessstep_trim::reconstruct(brep, face, raised),
            Err(e) if e.kind == tessstep_trim::ErrorKind::Limit
        );
    if samples {
        budget("planar.trim.max_samples", trim.max_samples)
    } else {
        budget("planar.trim.max_work", trim.max_work)
    }
}
/// The isotropic metric floor from the face's 3D boundary extent: flat directions
/// are measured relative to the face size.
fn sizing(
    mapped: &crate::FaceBoundary<'_>,
    tolerance: TessellationTolerance,
    resolution: f64,
) -> Result<metric::Sizing, Failure> {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for use_ in mapped.outer.iter().chain(mapped.holes.iter().flatten()) {
        for s in &use_.samples {
            for (k, x) in s.edge_sample.position.coordinates().into_iter().enumerate() {
                lo[k] = lo[k].min(x);
                hi[k] = hi[k].max(x);
            }
        }
    }
    let extent = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f64>();
    let chord = tolerance.chord().as_metres();
    let floor = 1. / extent.max(chord * chord);
    let ceiling = (1. / (resolution * resolution)).max(floor);
    if !floor.is_finite() {
        return Err(Failure::Geometry);
    }
    Ok(metric::Sizing {
        chord,
        angle: tolerance.normal_angle().as_radians(),
        floor,
        ceiling,
    })
}
/// A segment of a collapsed edge between two pole copies.
struct PoleSegment {
    pole: Source,
    edge: EdgeId,
    parameters: [f64; 2],
    uv: [[f64; 2]; 2],
}
/// A fan triangle whose pole copy lies outside the range its two rays span along
/// the pole line is folded: its polar wedge runs backwards and its 3D facet faces
/// away from the surface, and no interior split changes that. It requests a pole
/// sample between its rays instead (the collapsed edge is parameter-only, so the
/// request only reshapes the fans of the next boundary pass).
fn fold(corners: [Vertex; 3], poles: &[PoleSegment]) -> Option<(EdgeId, f64)> {
    let patch = Patch::new(&corners);
    let (s, k) = patch.polar?;
    if orient(patch.uv[0], patch.uv[1], patch.uv[2]) > 0. {
        return None;
    }
    let target = 0.5 * (corners[(s + 1) % 3].uv[k] + corners[(s + 2) % 3].uv[k]);
    let line = corners[s].uv[1 - k];
    poles.iter().find_map(|p| {
        let [a, b] = p.uv.map(|uv| uv[k]);
        let on_line = p.uv.iter().all(|uv| uv[1 - k] == line);
        if p.pole != corners[s].source || !on_line || !(a.min(b) < target && target < a.max(b)) {
            return None;
        }
        let [t0, t1] = p.parameters;
        let t = t0 + (target - a) / (b - a) * (t1 - t0);
        (t.is_finite() && t != t0 && t != t1).then_some((p.edge, t))
    })
}
/// The point on the ray from a pole copy to `other` halfway to it in 3D. Charts are
/// often far from uniform next to a pole (a B-spline row collapsing slowly), where
/// the chart midpoint can sit next to the pole and split off a sliver; a short
/// bisection on the ray parameter keeps the split between 3/8 and 5/8 of the chord.
fn ray_midpoint(
    surface: &SurfaceGeometry,
    pole: Vertex,
    other: Vertex,
    k: usize,
    work: &mut Work,
) -> Result<[f64; 2], Failure> {
    let at = |t: f64| {
        let mut uv = other.uv;
        uv[1 - k] = pole.uv[1 - k] + t * (other.uv[1 - k] - pole.uv[1 - k]);
        uv
    };
    let full = pole
        .position
        .distance(other.position)
        .map_err(|_| Failure::Geometry)?;
    let (mut lo, mut hi, mut t) = (0., 1., 0.5);
    for _ in 0..24 {
        let (position, _) = work.eval_any(surface, at(t), [KnotSide::Right; 2])?;
        let d = position
            .distance(pole.position)
            .map_err(|_| Failure::Geometry)?;
        if full.is_nan() || full <= 0. || (0.375 * full..=0.625 * full).contains(&d) {
            break;
        }
        if d < 0.5 * full {
            lo = t;
        } else {
            hi = t;
        }
        t = 0.5 * (lo + hi);
    }
    Ok(at(t))
}
/// A triangle rotated to start at its smallest index, keeping its orientation.
fn canonical(t: [u32; 3]) -> [u32; 3] {
    let k = (0..3).min_by_key(|&k| t[k]).expect("three corners");
    [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
}
/// The pole and other end of an edge from a pole copy to a regular vertex.
fn ray(vertices: &[Vertex], a: u32, b: u32) -> Option<(Source, u32)> {
    let (p, q) = (vertices[a as usize], vertices[b as usize]);
    match (p.free, q.free) {
        (Some(_), None) => Some((p.source, b)),
        (None, Some(_)) => Some((q.source, a)),
        _ => None,
    }
}
/// Length of a mesh edge in the curvature metric (the mean of its ends' tensors), or
/// its 3D chord scaled by the finer end's density when it ends at a pole copy, whose
/// chart point is arbitrary along the pole line, or at a singular corner.
fn edge_length(
    vertices: &[Vertex],
    sizing: metric::Sizing,
    a: u32,
    b: u32,
) -> Result<f64, Failure> {
    let (a, b) = (vertices[a as usize], vertices[b as usize]);
    let length = match (a.metric, b.metric) {
        (Some((ta, _)), Some((tb, _))) if a.free.is_none() && b.free.is_none() => {
            let t = metric::mean([ta, tb]).ok_or(Failure::Geometry)?;
            metric::length(t, [b.uv[0] - a.uv[0], b.uv[1] - a.uv[1]])
        }
        (ma, mb) => {
            let density = [ma, mb]
                .into_iter()
                .flatten()
                .map(|(_, d)| d)
                .fold(sizing.floor.sqrt(), f64::max);
            a.position
                .distance(b.position)
                .map_err(|_| Failure::Geometry)?
                * density
        }
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
        // A facet within the model tolerance of degenerate has no resolved plane (see
        // `acceptable`); a corner whose surface normal faces away from that plane
        // takes the plane's normal, so corner normals always face the facet's side.
        let [p, q, r] = corners.map(|v| v.position);
        let facet = q
            .difference(p)
            .and_then(|pq| r.difference(p).and_then(|pr| pq.cross(pr)))
            .and_then(|n| n.normalized());
        let mut corner = [a, b, c];
        if let Ok(facet) = facet {
            for n in &mut corner {
                if n.vector()
                    .dot(facet.vector())
                    .map_err(|_| Failure::Geometry)?
                    <= 0.
                {
                    *n = facet;
                }
            }
        }
        kept.push(tri);
        normals.push(corner);
        uvs.push(patch.uv);
    }
    Ok(FaceMesh {
        vertices,
        triangles: kept,
        normals,
        uvs,
    })
}
/// The surface position and (unless singular) normal at a probe. A right-sided probe
/// at a regular corner is that vertex's own evaluation, and one at the midpoint of an
/// edge between regular corners is shared through `probes`; anything else is
/// evaluated. Reuse returns exactly what a new evaluation would.
#[allow(clippy::too_many_arguments)]
fn sample(
    surface: &SurfaceGeometry,
    corners: &[Vertex; 3],
    tri: [u32; 3],
    patch: &Patch,
    uv: [f64; 2],
    sides: [KnotSide; 2],
    edge: Option<(usize, usize)>,
    probes: &mut Probes,
    work: &mut Work,
) -> Result<(Point3<ModelSpace>, Option<Direction3<ModelSpace>>), Failure> {
    let right = sides == [KnotSide::Right; 2];
    if right {
        for (k, v) in corners.iter().enumerate() {
            if v.normal.is_some() && v.free.is_none() && patch.uv[k] == uv {
                work.charge(1)?;
                return Ok((v.lifted, v.normal));
            }
        }
    }
    let shared = edge.filter(|&(i, j)| {
        right
            && [i, j]
                .iter()
                .all(|&k| corners[k].normal.is_some() && corners[k].free.is_none())
    });
    if let Some((i, j)) = shared {
        let key = key(tri[i], tri[j]);
        if let Some((at, jet)) = probes.midpoints.get(&key) {
            if *at == uv {
                work.charge(1)?;
                return Ok((jet.position, jet.normal(NumericalTolerance::default()).ok()));
            }
        }
        let jet = work.jet(surface, uv, sides)?;
        probes.midpoints.insert(key, (uv, jet));
        return Ok((jet.position, jet.normal(NumericalTolerance::default()).ok()));
    }
    work.eval_any(surface, uv, sides)
}
/// Whether one probe meets the facet's chord and angle limits.
fn judge(
    vertices: &[Vertex; 3],
    patch: &Patch,
    uv: [f64; 2],
    (position, n): (Point3<ModelSpace>, Option<Direction3<ModelSpace>>),
    facet: Facet,
) -> Result<bool, Failure> {
    let (normal, chord) = (facet.normal, facet.chord);
    // Geometric chord error: distance from the surface probe to the facet plane.
    // A tangential parametric offset (for example across a cone generator) is not
    // surface deviation, so the probe is not compared with its barycentric image;
    // the normal-angle check below still rejects folded or misoriented facets.
    let d = position
        .difference(vertices[0].lifted)
        .and_then(|v| v.dot(normal.vector()))
        .map_err(|_| Failure::Geometry)?
        .abs();
    // A singular corner (pole or apex) has no normal. A pole that a B-spline control
    // row only collapses to within tolerance still evaluates one, but it is noise.
    if (0..3).any(|k| vertices[k].normal.is_none() && patch.uv[k] == uv) {
        return Ok(d <= chord);
    }
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
            <= facet.angle)
}
/// A facet's plane normal and the chord and normal-angle limits its probes must meet.
#[derive(Clone, Copy)]
struct Facet {
    normal: Direction3<ModelSpace>,
    chord: f64,
    angle: f64,
}
/// Whether a facet meets the tolerances at every probe. `resolution` is the model
/// tolerance, the scale below which the model represents no geometry (corners may
/// sit that far off the surface): a facet no wider than it (twice its area over its
/// longest edge) must meet the chord, but its plane orientation is not resolved, so
/// it is not compared with surface normals.
#[allow(clippy::too_many_arguments)]
fn acceptable(
    surface: &SurfaceGeometry,
    vertices: [Vertex; 3],
    tri: [u32; 3],
    tolerance: TessellationTolerance,
    resolution: f64,
    probes: &mut Probes,
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
    let [a, b, c] = vertices.map(|v| v.lifted);
    let cross = b
        .difference(a)
        .and_then(|ab| c.difference(a).and_then(|ac| ab.cross(ac)))
        .map_err(|_| Failure::Geometry)?;
    let Ok(normal) = cross.normalized() else {
        return Ok(false);
    };
    let longest = [(a, b), (b, c), (c, a)]
        .into_iter()
        .map(|(p, q)| p.distance(q))
        .try_fold(0_f64, |m, d| d.map(|d| m.max(d)))
        .map_err(|_| Failure::Geometry)?;
    let width = cross.norm().map_err(|_| Failure::Geometry)? / longest;
    let facet = Facet {
        normal,
        chord: tolerance.chord().as_metres(),
        angle: if width <= resolution {
            f64::INFINITY
        } else {
            tolerance
                .normal_angle()
                .as_radians()
                .min(std::f64::consts::FRAC_PI_2 - 1e-12)
        },
    };
    for (weights, edge) in [
        ([1., 0., 0.], None),
        ([0., 1., 0.], None),
        ([0., 0., 1.], None),
        ([0.5, 0.5, 0.], Some((0, 1))),
        ([0., 0.5, 0.5], Some((1, 2))),
        ([0.5, 0., 0.5], Some((0, 2))),
        ([1. / 3.; 3], None),
        ([0.5, 0.25, 0.25], None),
        ([0.25, 0.5, 0.25], None),
        ([0.25, 0.25, 0.5], None),
    ] {
        let uv = patch.at(weights);
        let sides = [KnotSide::Right; 2];
        let edge = edge.filter(|_| patch.polar.is_none());
        let at = sample(
            surface, &vertices, tri, &patch, uv, sides, edge, probes, work,
        )?;
        if !judge(&vertices, &patch, uv, at, facet)? {
            return Ok(false);
        }
    }
    if let SurfaceGeometry::Nurbs(s) = surface {
        // Every intersected knot cell is probed, including both one-sided limits.
        // Small knot spans cannot disappear between a fixed triangle's usual probes.
        let mut cuts = [Vec::new(), Vec::new()];
        // Whether a knot line meets the closed extent: only there do one-sided limits
        // differ, so a facet inside one cell repeats its stencil probes and is done.
        let mut touches = [false; 2];
        // The knot lines through the closed extent, where one-sided limits differ.
        let mut lines = [Vec::new(), Vec::new()];
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
            // Knots are sorted and a constant shift keeps them sorted, so the knot
            // lines inside an extent are found by bisection.
            let mut within = |shift: f64, cut: &mut Vec<f64>, work: &mut Work| {
                let start = knots.partition_point(|&k| k + shift < lo);
                let open = knots.partition_point(|&k| k + shift <= lo);
                let end = knots.partition_point(|&k| k + shift < hi);
                let closed = knots.partition_point(|&k| k + shift <= hi);
                work.charge(2 + closed.saturating_sub(start))?;
                touches[axis] |= closed > start;
                lines[axis].extend(knots[start..closed.max(start)].iter().map(|&k| k + shift));
                cut.extend(knots[open..end.max(open)].iter().map(|&k| k + shift));
                Ok::<(), Failure>(())
            };
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
                    within(n as f64 * period, cut, work)?;
                }
                cut.sort_by(f64::total_cmp);
            } else {
                within(0., cut, work)?;
            }
            cut.push(hi);
            cut.dedup();
        }
        if patch.polar.is_none() && !touches[0] && !touches[1] {
            return Ok(true);
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
                // A left limit off every knot line is the ordinary evaluation.
                let mut check = |p: [f64; 3], work: &mut Work| -> Result<bool, Failure> {
                    let uv = patch.linear(p);
                    let sides = [0, 1].map(|axis| {
                        if uv[axis] >= [u[1], v[1]][axis] && lines[axis].contains(&uv[axis]) {
                            KnotSide::Left
                        } else {
                            KnotSide::Right
                        }
                    });
                    let at = sample(
                        surface, &vertices, tri, &patch, uv, sides, None, probes, work,
                    )?;
                    judge(&vertices, &patch, uv, at, facet)
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
/// A fixed multiplicative hash of an edge key. Maps keyed with it are only looked
/// up, never iterated for a decision, so results stay deterministic.
#[derive(Clone, Copy, Default)]
struct EdgeHasher(u64);
impl std::hash::Hasher for EdgeHasher {
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 29)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(b)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0.rotate_left(32) ^ u64::from(n)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}
type EdgeMap<V> = std::collections::HashMap<Key, V, std::hash::BuildHasherDefault<EdgeHasher>>;
/// The triangles on one edge (a third use only counts).
#[derive(Clone, Copy)]
struct Incident {
    count: u8,
    tris: [usize; 2],
}
/// Record the edges of the triangle in `slot`.
fn attach(incident: &mut EdgeMap<Incident>, triangles: &[[u32; 3]], slot: usize) {
    let t = triangles[slot];
    for k in 0..3 {
        let entry = incident
            .entry(key(t[k], t[(k + 1) % 3]))
            .or_insert(Incident {
                count: 0,
                tris: [usize::MAX; 2],
            });
        if entry.count < 2 {
            entry.tris[usize::from(entry.count)] = slot;
        }
        entry.count = entry.count.saturating_add(1);
    }
}
/// Forget the edges of the triangle in `slot`.
fn detach(incident: &mut EdgeMap<Incident>, triangles: &[[u32; 3]], slot: usize) {
    let t = triangles[slot];
    for k in 0..3 {
        let edge = key(t[k], t[(k + 1) % 3]);
        if let Some(entry) = incident.get_mut(&edge) {
            if let Some(n) = entry.tris.iter().position(|&s| s == slot) {
                entry.tris[n] = entry.tris[1 - n];
                entry.tris[1] = usize::MAX;
            }
            entry.count = entry.count.saturating_sub(1);
            if entry.count == 0 {
                incident.remove(&edge);
            }
        }
    }
}
/// Queue-based Lawson flips in the quadrilaterals' curvature-metric charts: only edges
/// around a flip are revisited, so an ear-clipping fan unwinds in one pass instead of
/// one sweep per fan triangle. Constraints are never flipped. A strict angular margin
/// avoids cycling on cocircular floating-point inputs, and the flip count is bounded;
/// shape improvement is optional because the error checks still run.
///
/// The queue holds every interior edge in key order, popped from the largest, with
/// edges around a flip pushed on top unless already queued. With `fresh` (the
/// triangles the last split created) a call after a settled one processes only the
/// edges of fresh triangles and those a flip reaches before their turn, at their
/// place in that order: every other edge's quadrilateral is unchanged since it was
/// last found settled, so popping it would flip nothing and the flips are the same.
/// Flipped triangles are marked not `known` to pass. Returns whether the queue
/// emptied (rather than stopping at the flip ceiling).
#[allow(clippy::too_many_arguments)]
fn improve(
    vertices: &[Vertex],
    triangles: &mut [[u32; 3]],
    incident: &mut EdgeMap<Incident>,
    constraints: &BTreeMap<Key, (EdgeId, f64)>,
    metric: [f64; 2],
    fresh: Option<&[bool]>,
    known: &mut [bool],
    work: &mut Work,
) -> Result<bool, Failure> {
    let interior = |incident: &EdgeMap<Incident>, edge: &Key| {
        incident.get(edge).is_some_and(|e| e.count == 2) && !constraints.contains_key(edge)
    };
    // The edges that start in the queue and still have to be popped there, below
    // `cursor`; only those that may flip are listed.
    let mut initial = BTreeSet::new();
    for (i, t) in triangles.iter().enumerate() {
        if fresh.is_none_or(|f| f[i]) {
            work.charge(1)?;
            for k in 0..3 {
                let edge = key(t[k], t[(k + 1) % 3]);
                if interior(incident, &edge) {
                    initial.insert(edge);
                }
            }
        }
    }
    // Diagonals this call created; every other interior edge was in the starting
    // queue (flips keep the triangle count of the edges around them).
    let mut created = EdgeMap::<()>::default();
    let mut cursor: Option<Key> = None;
    let mut pushed: Vec<Key> = Vec::new();
    let mut queued_pushed = EdgeMap::<()>::default();
    let mut flips = 0usize;
    let limit = triangles.len().saturating_mul(64).saturating_add(1024);
    loop {
        let edge = if let Some(edge) = pushed.pop() {
            queued_pushed.remove(&edge);
            edge
        } else if let Some(edge) = initial.pop_last() {
            cursor = Some(edge);
            edge
        } else {
            return Ok(true);
        };
        work.charge(1)?;
        let Some(&Incident { count: 2, tris }) = incident.get(&edge) else {
            continue;
        };
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
        // The quadrilateral's mean curvature metric, else the face's diagonal one.
        let quad = metric::mean(
            [a, b, c, d]
                .into_iter()
                .filter_map(|i| vertices[i as usize].metric.map(|(t, _)| t)),
        )
        .unwrap_or([metric[0] * metric[0], 0., metric[1] * metric[1]]);
        let Some(chart) = metric::chart(quad) else {
            return Err(Failure::Geometry);
        };
        let [pa, pb, pc, pd] = [a, b, c, d].map(|i| chart(vertices[i as usize].uv));
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
            return Ok(false);
        }
        flips += 1;
        work.charge(8)?;
        triangles[i] = [c, d, b];
        triangles[j] = [d, c, a];
        known[i] = false;
        known[j] = false;
        incident.remove(&edge);
        incident.insert(
            key(c, d),
            Incident {
                count: 2,
                tris: [i, j],
            },
        );
        created.insert(key(c, d), ());
        for (e, from, to) in [(key(d, b), j, i), (key(c, a), i, j)] {
            if let Some(entry) = incident.get_mut(&e) {
                for t in entry.tris.iter_mut() {
                    if *t == from {
                        *t = to;
                    }
                }
            }
        }
        for e in [key(b, c), key(c, a), key(a, d), key(d, b)] {
            if constraints.contains_key(&e) || queued_pushed.contains_key(&e) {
                continue;
            }
            // Still waiting in the starting queue: it is processed at its place.
            let waiting = match fresh {
                Some(_) => {
                    interior(incident, &e)
                        && !created.contains_key(&e)
                        && cursor.is_none_or(|c| e < c)
                }
                None => initial.contains(&e),
            };
            if waiting {
                initial.insert(e);
                continue;
            }
            queued_pushed.insert(e, ());
            pushed.push(e);
        }
    }
}
