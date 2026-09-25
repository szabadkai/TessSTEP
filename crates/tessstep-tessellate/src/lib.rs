//! Canonical shared-edge sampling from immutable normalized topology.
//! This stage emits boundary polylines, not triangles or a watertight solid.
#![forbid(unsafe_code)]
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
struct Budget {
    samples: usize,
    evaluations: usize,
}
impl Budget {
    fn evaluate(&mut self) -> Result<(), ErrorKind> {
        self.evaluations = self
            .evaluations
            .checked_sub(1)
            .ok_or(ErrorKind::ResourceLimit)?;
        Ok(())
    }
    fn push(&mut self, out: &mut Vec<Sample>, sample: Sample) -> Result<(), ErrorKind> {
        self.samples = self
            .samples
            .checked_sub(1)
            .ok_or(ErrorKind::ResourceLimit)?;
        out.push(sample);
        Ok(())
    }
}
pub fn sample_edges(
    brep: &NormalizedBrep,
    tolerance: TessellationTolerance,
    limits: SamplingLimits,
) -> Result<SharedEdges<'_>, Error> {
    if limits.max_depth > 48 || limits.max_samples > 10_000_000 {
        return Err(Error {
            edge: EdgeId(0),
            kind: ErrorKind::InvalidLimits,
        });
    }
    let mut budget = Budget {
        samples: limits.max_samples,
        evaluations: limits.max_evaluations,
    };
    let mut edges = Vec::new();
    for (i, edge) in brep.data().edges.iter().enumerate() {
        let sample = |budget: &mut Budget| -> Result<Vec<Sample>, ErrorKind> {
            let chord = tolerance.chord().as_metres();
            let angle = tolerance.normal_angle().as_radians();
            let seeds = edge
                .curve
                .break_parameters(edge.range, budget.samples)
                .map_err(|_| ErrorKind::ResourceLimit)?;
            let evaluate = |u: f64,
                            side: KnotSide,
                            budget: &mut Budget|
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
                        let v = brep.data().vertices[edge.vertices[k].0].position;
                        if v.distance(p).map_err(|_| ErrorKind::Geometry)? > chord {
                            return Err(ErrorKind::EndpointTolerance);
                        }
                        Ok(v)
                    } else {
                        Ok(p)
                    }
                };
            let mut out = Vec::new();
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
                let span_start = out.last().expect("first sample").position;
                let mut stack = vec![(w[0], w[1], a, b, 0)];
                while let Some((u, v, a, b, depth)) = stack.pop() {
                    let pa = if u == w[0] {
                        span_start
                    } else {
                        endpoint(u, a.position)?
                    };
                    let pb = endpoint(v, b.position)?;
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
                                position: pb,
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
        let samples = sample(&mut budget).map_err(|kind| Error {
            edge: EdgeId(i),
            kind,
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

#[derive(Clone, Debug, PartialEq)]
pub enum BoundaryError {
    Trim(tessstep_trim::Error),
    ResourceLimit,
    Geometry(CoedgeId),
    CurveSurfaceMismatch(CoedgeId),
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
impl SharedEdges<'_> {
    /// Reconstruct the chart, then map each canonical edge sample to every face use.
    /// Position references point into this cache, including both uses of a seam.
    /// This is input for later constrained triangulation, not a triangle mesh.
    pub fn face_boundary(
        &self,
        face: tessstep_topology::FaceId,
        options: tessstep_trim::Options,
        max_samples: usize,
    ) -> Result<FaceBoundary<'_>, BoundaryError> {
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
                    jet.normal(tessstep_math::NumericalTolerance::default())
                        .map_err(|_| BoundaryError::Geometry(use_.coedge))?;
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
        let holes = trim
            .holes()
            .iter()
            .map(&mut map_loop)
            .collect::<Result<_, _>>()?;
        Ok(FaceBoundary { outer, holes })
    }
}
