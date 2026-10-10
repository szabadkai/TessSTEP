//! Canonical shared-edge sampling from immutable normalized topology.
//! Shared boundaries, adaptive regular face triangles and manifold shell/solid assembly.
#![forbid(unsafe_code)]
mod adaptive;
mod metric;
mod planar;
pub use adaptive::{
    Budget, TessellationError, TessellationErrorKind, TessellationOptions, tessellate_faces,
    tessellate_shell, tessellate_solid,
};
pub use planar::{PlanarError, PlanarMesh, PlanarOptions};
use tessstep_curves::{Evaluation, spline::KnotSide};
use tessstep_math::{ModelSpace, Point3, TessellationTolerance};
use tessstep_topology::{CoedgeId, EdgeId, NormalizedBrep};
#[derive(Clone, Copy, Debug)]
pub struct SamplingLimits {
    pub max_samples: usize,
    pub max_evaluations: usize,
    pub max_depth: usize,
}
impl Default for SamplingLimits {
    fn default() -> Self {
        Self {
            max_samples: 1_000_000,
            max_evaluations: 10_000_000,
            max_depth: 32,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidLimits,
    ResourceLimit,
    Geometry,
    SingularTangent,
    DiscontinuousCurve,
    EndpointTolerance,
    UnresolvedTolerance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub edge: EdgeId,
    pub kind: ErrorKind,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "edge {} sampling {:?}", self.edge.0, self.kind)
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub parameter: f64,
    pub position: Point3<ModelSpace>,
}
#[derive(Clone, Debug)]
pub struct EdgePolyline {
    edge: EdgeId,
    samples: Vec<Sample>,
}
impl EdgePolyline {
    pub fn edge(&self) -> EdgeId {
        self.edge
    }
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }
}
/// The borrow ties indices/views to the exact immutable model used for sampling.
#[derive(Debug)]
pub struct SharedEdges<'a> {
    brep: &'a NormalizedBrep,
    edges: Vec<EdgePolyline>,
    evaluations: usize,
}
impl SharedEdges<'_> {
    pub fn edges(&self) -> &[EdgePolyline] {
        &self.edges
    }
    pub fn evaluations(&self) -> usize {
        self.evaluations
    }
    pub fn edge(&self, id: EdgeId) -> Option<&EdgePolyline> {
        self.edges.get(id.0)
    }
    pub fn coedge(&self, id: CoedgeId) -> Option<OrientedSamples<'_>> {
        let c = self.brep.data().coedges.get(id.0)?;
        Some(OrientedSamples {
            samples: &self.edges[c.edge.0].samples,
            reversed: c.orientation.is_reversed(),
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct OrientedSamples<'a> {
    samples: &'a [Sample],
    reversed: bool,
}
impl<'a> OrientedSamples<'a> {
    /// Same stored samples for every use; reversal allocates and evaluates nothing.
    pub fn iter(self) -> impl DoubleEndedIterator<Item = &'a Sample> + ExactSizeIterator {
        (0..self.samples.len()).map(move |i| {
            &self.samples[if self.reversed {
                self.samples.len() - 1 - i
            } else {
                i
            }]
        })
    }
}
/// Which sampling budget a `ResourceLimit` exhausted, for the tessellation diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exhausted {
    Samples,
    Evaluations,
}
struct SampleBudget {
    samples: usize,
    evaluations: usize,
    exhausted: Option<Exhausted>,
}
impl SampleBudget {
    fn evaluate(&mut self) -> Result<(), ErrorKind> {
        self.evaluations = self.evaluations.checked_sub(1).ok_or_else(|| {
            self.exhausted = Some(Exhausted::Evaluations);
            ErrorKind::ResourceLimit
        })?;
        Ok(())
    }
    fn exhaust_samples(&mut self) -> ErrorKind {
        self.exhausted = Some(Exhausted::Samples);
        ErrorKind::ResourceLimit
    }
    fn push(&mut self, out: &mut Vec<Sample>, sample: Sample) -> Result<(), ErrorKind> {
        self.samples = match self.samples.checked_sub(1) {
            Some(rest) => rest,
            None => return Err(self.exhaust_samples()),
        };
        out.push(sample);
        Ok(())
    }
}
pub fn sample_edges(
    brep: &NormalizedBrep,
    tolerance: TessellationTolerance,
    limits: SamplingLimits,
) -> Result<SharedEdges<'_>, Error> {
    sample_edges_seeded(brep, tolerance, limits, &[], &mut None)
}
fn sample_edges_seeded<'a>(
    brep: &'a NormalizedBrep,
    tolerance: TessellationTolerance,
    limits: SamplingLimits,
    forced: &[Vec<f64>],
    exhausted: &mut Option<Exhausted>,
) -> Result<SharedEdges<'a>, Error> {
    if limits.max_depth > 48 || limits.max_samples > 10_000_000 {
        return Err(Error {
            edge: EdgeId(0),
            kind: ErrorKind::InvalidLimits,
        });
    }
    let mut budget = SampleBudget {
        samples: limits.max_samples,
        evaluations: limits.max_evaluations,
        exhausted: None,
    };
    let mut edges = Vec::new();
    for (i, edge) in brep.data().edges.iter().enumerate() {
        let sample = |budget: &mut SampleBudget| -> Result<Vec<Sample>, ErrorKind> {
            let chord = tolerance.chord().as_metres();
            let angle = tolerance.normal_angle().as_radians();
            let mut seeds = match edge.curve.break_parameters(edge.range, budget.samples) {
                Ok(seeds) => seeds,
                Err(_) => return Err(budget.exhaust_samples()),
            };
            if let Some(extra) = forced.get(i) {
                if extra.len() > limits.max_samples {
                    return Err(budget.exhaust_samples());
                }
                seeds.extend_from_slice(extra);
                seeds.sort_by(f64::total_cmp);
                seeds.dedup();
                if seeds.len() > budget.samples {
                    return Err(budget.exhaust_samples());
                }
            }
            // Seeds within numerical noise of each other (a range starting at -5e-17
            // beside the period break at 0) would span no length and never satisfy
            // the chord direction; keep the range ends and drop their neighbours.
            let [t0, t1] = edge.range;
            let noise = 1e-9 * (t1 - t0).abs();
            let mut cleaned: Vec<f64> = Vec::with_capacity(seeds.len());
            for &s in &seeds {
                let near_end = (s - t0).abs() <= noise || (t1 - s).abs() <= noise;
                if s != t0 && s != t1 && near_end {
                    continue;
                }
                if cleaned
                    .last()
                    .is_some_and(|&last| (s - last).abs() <= noise)
                {
                    continue;
                }
                cleaned.push(s);
            }
            seeds = cleaned;
            let evaluate = |u: f64,
                            side: KnotSide,
                            budget: &mut SampleBudget|
             -> Result<Evaluation<ModelSpace, 3>, ErrorKind> {
                budget.evaluate()?;
                edge.curve
                    .evaluate_on_side(u, side)
                    .map_err(|_| ErrorKind::Geometry)
            };
            let endpoint =
                |u: f64, p: Point3<ModelSpace>| -> Result<Point3<ModelSpace>, ErrorKind> {
                    let index = if u == edge.range[0] {
                        Some(0)
                    } else if u == edge.range[1] {
                        Some(1)
                    } else {
                        None
                    };
                    if let Some(k) = index {
                        // A vertex may sit off its curve end by up to the model tolerance
                        // (the B-rep accepted it); the end segments then deviate by that
                        // much in addition to the chord.
                        let v = brep.data().vertices[edge.vertices[k].0].position;
                        if v.distance(p).map_err(|_| ErrorKind::Geometry)?
                            > brep.model_tolerance().max(chord)
                        {
                            return Err(ErrorKind::EndpointTolerance);
                        }
                        Ok(v)
                    } else {
                        Ok(p)
                    }
                };
            let mut out = Vec::new();
            if brep.is_collapsed(EdgeId(i)) {
                // A collapsed edge is a point: its samples only carry parameters, so
                // face charts can place UV samples along their singular line.
                let at = brep.data().vertices[edge.vertices[0].0].position;
                for &parameter in &seeds {
                    budget.push(
                        &mut out,
                        Sample {
                            parameter,
                            position: at,
                        },
                    )?;
                }
                return Ok(out);
            }
            let first = evaluate(edge.range[0], KnotSide::Right, budget)?;
            budget.push(
                &mut out,
                Sample {
                    parameter: edge.range[0],
                    position: endpoint(edge.range[0], first.position)?,
                },
            )?;
            for (span, w) in seeds.windows(2).enumerate() {
                let a = evaluate(w[0], KnotSide::Right, budget)?;
                let b = evaluate(w[1], KnotSide::Left, budget)?;
                if span > 0
                    && a.position
                        .distance(out.last().expect("first sample").position)
                        .map_err(|_| ErrorKind::Geometry)?
                        > brep.model_tolerance().min(chord)
                {
                    return Err(ErrorKind::DiscontinuousCurve);
                }
                let mut stack = vec![(w[0], w[1], a, b, 0)];
                while let Some((u, v, a, b, depth)) = stack.pop() {
                    // Chord and tangent criteria measure the curve itself; only the
                    // stored end samples are moved onto their vertices.
                    let (pa, pb) = (a.position, b.position);
                    let probes = [
                        evaluate(u + (v - u) * 0.25, KnotSide::Right, budget)?,
                        evaluate(u + (v - u) * 0.5, KnotSide::Right, budget)?,
                        evaluate(u + (v - u) * 0.75, KnotSide::Right, budget)?,
                    ];
                    for jet in std::iter::once(&a)
                        .chain(probes.iter())
                        .chain(std::iter::once(&b))
                    {
                        jet.first
                            .normalized()
                            .map_err(|_| ErrorKind::SingularTangent)?;
                    }
                    let delta = pb.difference(pa).map_err(|_| ErrorKind::Geometry)?;
                    let chord_direction = delta.normalized();
                    let mut good = chord_direction.is_ok();
                    if let Ok(dir) = chord_direction {
                        for jet in std::iter::once(&a)
                            .chain(probes.iter())
                            .chain(std::iter::once(&b))
                        {
                            let tangent = jet
                                .first
                                .normalized()
                                .map_err(|_| ErrorKind::SingularTangent)?;
                            if tangent
                                .angle_to(dir)
                                .map_err(|_| ErrorKind::Geometry)?
                                .as_radians()
                                > angle
                            {
                                good = false;
                            }
                            if segment_distance(jet.position, pa, pb)? > chord {
                                good = false;
                            }
                        }
                    }
                    if good {
                        budget.push(
                            &mut out,
                            Sample {
                                parameter: v,
                                position: endpoint(v, pb)?,
                            },
                        )?;
                    } else {
                        if depth >= limits.max_depth {
                            return Err(ErrorKind::UnresolvedTolerance);
                        }
                        let mid = u + (v - u) * 0.5;
                        if mid == u || mid == v {
                            return Err(ErrorKind::UnresolvedTolerance);
                        }
                        let middle = probes[1];
                        stack.push((mid, v, middle, b, depth + 1));
                        stack.push((u, mid, a, middle, depth + 1));
                    }
                }
            }
            Ok(out)
        };
        let samples = sample(&mut budget).map_err(|kind| {
            *exhausted = budget.exhausted;
            Error {
                edge: EdgeId(i),
                kind,
            }
        })?;
        edges.push(EdgePolyline {
            edge: EdgeId(i),
            samples,
        });
    }
    Ok(SharedEdges {
        brep,
        edges,
        evaluations: limits.max_evaluations - budget.evaluations,
    })
}
fn segment_distance(
    p: Point3<ModelSpace>,
    a: Point3<ModelSpace>,
    b: Point3<ModelSpace>,
) -> Result<f64, ErrorKind> {
    let d = b.difference(a).map_err(|_| ErrorKind::Geometry)?;
    let length = d.norm().map_err(|_| ErrorKind::Geometry)?;
    if length == 0. {
        return p.distance(a).map_err(|_| ErrorKind::Geometry);
    }
    let unit = d.normalized().map_err(|_| ErrorKind::Geometry)?.vector();
    let t = p
        .difference(a)
        .and_then(|v| v.dot(unit))
        .map_err(|_| ErrorKind::Geometry)?
        .clamp(0., length);
    p.distance(
        a.translated(unit.scaled(t).map_err(|_| ErrorKind::Geometry)?)
            .map_err(|_| ErrorKind::Geometry)?,
    )
    .map_err(|_| ErrorKind::Geometry)
}

/// The distance below which UV boundary points are not resolved apart: relative to
/// the boundary extent and coordinate magnitude (floating-point sign ambiguity), and
/// at most `uv_tolerance`. `None` for non-finite points.
pub(crate) fn resolution_tolerance(
    points: impl Iterator<Item = [f64; 2]>,
    uv_tolerance: f64,
) -> Option<f64> {
    let (mut lo, mut hi, mut magnitude) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2], 0_f64);
    for p in points {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
            magnitude = magnitude.max(p[k].abs());
        }
    }
    let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    let eps = 1e-12 * extent + 16. * f64::EPSILON * magnitude;
    eps.is_finite()
        .then(|| eps.min(uv_tolerance).max(f64::MIN_POSITIVE))
}
#[derive(Clone, Debug, PartialEq)]
pub enum BoundaryError {
    Trim(tessstep_trim::Error),
    ResourceLimit,
    Geometry(CoedgeId),
    CurveSurfaceMismatch(CoedgeId),
    /// The loops at the shared edge sample resolution are not simple, correctly
    /// wound and nested at floating-point resolution although their trim
    /// reconstruction passed; located at the coedges involved (for example a coarse
    /// chord of one edge crossing a nearby edge).
    Resolution(tessstep_trim::Error),
}
impl std::fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shared face boundary: {self:?}")
    }
}
impl std::error::Error for BoundaryError {}
#[derive(Clone, Copy, Debug)]
pub struct BoundarySample<'a> {
    pub edge_sample: &'a Sample,
    pub uv: [f64; 2],
}
#[derive(Debug)]
pub struct BoundaryPolyline<'a> {
    pub coedge: CoedgeId,
    pub samples: Vec<BoundarySample<'a>>,
}
#[derive(Debug)]
pub struct FaceBoundary<'a> {
    pub outer: Vec<BoundaryPolyline<'a>>,
    pub holes: Vec<Vec<BoundaryPolyline<'a>>>,
}
/// A face boundary mapped to its chart, before its resolution check counts.
pub(crate) struct MappedBoundary<'a> {
    pub(crate) boundary: FaceBoundary<'a>,
    /// The floating-point resolution the check used.
    pub(crate) eps: f64,
    pub(crate) checked: Result<(), BoundaryError>,
}
impl SharedEdges<'_> {
    /// Reconstruct the chart, then map each canonical edge sample to every face use.
    /// Position references point into this cache, including both uses of a seam.
    /// Input for planar triangulation, also usable as a standalone boundary view.
    pub fn face_boundary(
        &self,
        face: tessstep_topology::FaceId,
        options: tessstep_trim::Options,
        max_samples: usize,
    ) -> Result<FaceBoundary<'_>, BoundaryError> {
        let mapped = self.mapped_boundary(face, options, max_samples)?;
        mapped.checked.map(|()| mapped.boundary)
    }
    /// The mapped boundary with, separately, the result of its resolution check and
    /// the resolution it used, so that a failed check can be located on its segments.
    pub(crate) fn mapped_boundary(
        &self,
        face: tessstep_topology::FaceId,
        options: tessstep_trim::Options,
        max_samples: usize,
    ) -> Result<MappedBoundary<'_>, BoundaryError> {
        let trim =
            tessstep_trim::reconstruct(self.brep, face, options).map_err(BoundaryError::Trim)?;
        let surface = &self.brep.data().faces[face.0].surface;
        let mut remaining = max_samples.min(10_000_000);
        let mut map_loop = |boundary: &tessstep_trim::TrimLoop| {
            let mut out = Vec::new();
            for use_ in boundary.uses() {
                let c = &self.brep.data().coedges[use_.coedge.0];
                let edge = &self.brep.data().edges[c.edge.0];
                let pc = c.pcurve.as_ref().expect("reconstructed pcurve");
                let view = self.coedge(use_.coedge).expect("validated coedge");
                remaining = remaining
                    .checked_sub(view.samples.len())
                    .ok_or(BoundaryError::ResourceLimit)?;
                let mut samples = Vec::with_capacity(view.samples.len());
                for sample in view.iter() {
                    let fraction =
                        (sample.parameter - edge.range[0]) / (edge.range[1] - edge.range[0]);
                    let u = if fraction == 0. {
                        pc.range[0]
                    } else if fraction == 1. {
                        pc.range[1]
                    } else {
                        pc.range[0] + fraction * (pc.range[1] - pc.range[0])
                    };
                    let mut uv = pc
                        .curve
                        .evaluate(u)
                        .map_err(|_| BoundaryError::Geometry(use_.coedge))?
                        .position
                        .coordinates();
                    for (k, offset) in use_.period_shift.iter().enumerate() {
                        uv[k] += offset;
                    }
                    let jet = surface
                        .evaluate(uv)
                        .map_err(|_| BoundaryError::Geometry(use_.coedge))?;
                    // Singular normals are permitted on collapsed edges and at their
                    // vertices, where the chart meets a pole or apex.
                    let singular = self.brep.is_collapsed(c.edge)
                        || (sample.parameter == edge.range[0]
                            && self.brep.is_singular_vertex(edge.vertices[0]))
                        || (sample.parameter == edge.range[1]
                            && self.brep.is_singular_vertex(edge.vertices[1]));
                    if !singular {
                        jet.normal(tessstep_math::NumericalTolerance::default())
                            .map_err(|_| BoundaryError::Geometry(use_.coedge))?;
                    }
                    if jet
                        .position
                        .distance(sample.position)
                        .map_err(|_| BoundaryError::Geometry(use_.coedge))?
                        > self.brep.model_tolerance()
                    {
                        return Err(BoundaryError::CurveSurfaceMismatch(use_.coedge));
                    }
                    samples.push(BoundarySample {
                        edge_sample: sample,
                        uv,
                    });
                }
                out.push(BoundaryPolyline {
                    coedge: use_.coedge,
                    samples,
                });
            }
            Ok(out)
        };
        let outer = map_loop(trim.outer())?;
        let holes: Vec<Vec<BoundaryPolyline<'_>>> = trim
            .holes()
            .iter()
            .map(&mut map_loop)
            .collect::<Result<_, _>>()?;
        // The polygons a mesh uses (each use's samples but its last, as joins keep the
        // next use's start) must be simple and correctly nested at the resolution the
        // triangulator resolves; failures are located at their coedges.
        let loops: Vec<Vec<([f64; 2], CoedgeId)>> = std::iter::once(&outer)
            .chain(&holes)
            .map(|uses| {
                uses.iter()
                    .flat_map(|u| {
                        u.samples[..u.samples.len() - 1]
                            .iter()
                            .map(|s| (s.uv, u.coedge))
                    })
                    .collect()
            })
            .collect();
        let eps = resolution_tolerance(loops.iter().flatten().map(|p| p.0), options.uv_tolerance)
            .ok_or(BoundaryError::Geometry(trim.outer().uses()[0].coedge))?;
        let checked = trim
            .validate_resolution(&loops, eps, options.max_work)
            .map_err(BoundaryError::Resolution);
        Ok(MappedBoundary {
            boundary: FaceBoundary { outer, holes },
            eps,
            checked,
        })
    }
}
