//! Bounded reconstruction of supplied pcurves in an explicit UV chart.
//! No arbitrary 3D projection, automatic healing or certified curve error bounds.
#![forbid(unsafe_code)]
mod polygon;
use tessstep_curves::spline::KnotSide;
use tessstep_math::NumericalTolerance;
use tessstep_surfaces::AxisDomain;
use tessstep_topology::{CoedgeId, FaceId, NormalizedBrep, Pcurve, WireId};
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Positive parameter-space chord and joining tolerance, in the supplied UV units.
    pub uv_tolerance: f64,
    pub max_depth: usize,
    pub max_samples: usize,
    pub max_work: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            uv_tolerance: 1e-5,
            max_depth: 24,
            max_samples: 100_000,
            max_work: 5_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidOptions,
    InvalidFace,
    MissingPcurve,
    DiscontinuousPcurve,
    Geometry,
    SingularSurface,
    CurveSurfaceMismatch,
    Limit,
    UnresolvedTolerance,
    OpenLoop,
    NonContractibleLoop,
    DegenerateLoop,
    SelfIntersection,
    WrongOrientation,
    HoleOutside,
    IntersectingLoops,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub coedge: Option<CoedgeId>,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UV trimming {:?} at {:?}", self.kind, self.coedge)
    }
}
impl std::error::Error for Error {}
fn err(kind: ErrorKind) -> Error {
    Error { kind, coedge: None }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvSample {
    pub edge_fraction: f64,
    pub uv: [f64; 2],
}
#[derive(Clone, Debug)]
pub struct BoundaryUse {
    pub coedge: CoedgeId,
    pub samples: Vec<UvSample>,
    pub period_shift: [f64; 2],
}
#[derive(Clone, Debug)]
pub struct TrimLoop {
    uses: Vec<BoundaryUse>,
    polygon: Vec<[f64; 2]>,
    signed_area: f64,
}
impl TrimLoop {
    pub fn uses(&self) -> &[BoundaryUse] {
        &self.uses
    }
    pub fn polygon(&self) -> &[[f64; 2]] {
        &self.polygon
    }
    pub fn signed_area(&self) -> f64 {
        self.signed_area
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Containment {
    Outside,
    Boundary,
    Inside,
}
#[derive(Clone, Debug)]
pub struct TrimmedFace {
    face: FaceId,
    outer: TrimLoop,
    holes: Vec<TrimLoop>,
    tolerance: f64,
}
impl TrimmedFace {
    pub fn face(&self) -> FaceId {
        self.face
    }
    pub fn outer(&self) -> &TrimLoop {
        &self.outer
    }
    pub fn holes(&self) -> &[TrimLoop] {
        &self.holes
    }
    /// Classification of the reconstructed polygons in their explicit unwrapped chart.
    pub fn classify(&self, uv: [f64; 2]) -> Result<Containment, Error> {
        let outer = polygon::classify(&self.outer.polygon, uv, self.tolerance)?;
        if outer != Containment::Inside {
            return Ok(outer);
        }
        for h in &self.holes {
            match polygon::classify(&h.polygon, uv, self.tolerance)? {
                Containment::Inside => return Ok(Containment::Outside),
                Containment::Boundary => return Ok(Containment::Boundary),
                _ => {}
            }
        }
        Ok(Containment::Inside)
    }
}
struct Budget {
    work: usize,
    samples: usize,
}
impl Budget {
    fn work(&mut self, n: usize) -> Result<(), Error> {
        self.work = self.work.checked_sub(n).ok_or(err(ErrorKind::Limit))?;
        Ok(())
    }
    fn sample(&mut self) -> Result<(), Error> {
        self.samples = self.samples.checked_sub(1).ok_or(err(ErrorKind::Limit))?;
        self.work(1)
    }
}
pub fn reconstruct(
    brep: &NormalizedBrep,
    face: FaceId,
    options: Options,
) -> Result<TrimmedFace, Error> {
    if !options.uv_tolerance.is_finite()
        || options.uv_tolerance <= 0.
        || options.max_depth > 48
        || options.max_samples > 1_000_000
    {
        return Err(err(ErrorKind::InvalidOptions));
    }
    let f = brep
        .data()
        .faces
        .get(face.0)
        .ok_or(err(ErrorKind::InvalidFace))?;
    let mut budget = Budget {
        work: options.max_work,
        samples: options.max_samples,
    };
    let outer = build_loop(brep, face, f.outer, options, &mut budget)?;
    if outer.signed_area <= 0. {
        return Err(err(ErrorKind::WrongOrientation));
    }
    let mut holes = vec![];
    for &wire in &f.holes {
        let h = build_loop(brep, face, wire, options, &mut budget)?;
        if h.signed_area >= 0. {
            return Err(err(ErrorKind::WrongOrientation));
        }
        polygon::disjoint(
            &outer.polygon,
            &h.polygon,
            options.uv_tolerance,
            &mut budget,
        )?;
        if polygon::classify(&outer.polygon, h.polygon[0], options.uv_tolerance)?
            != Containment::Inside
        {
            return Err(err(ErrorKind::HoleOutside));
        }
        for other in &holes {
            let other: &TrimLoop = other;
            polygon::disjoint(
                &other.polygon,
                &h.polygon,
                options.uv_tolerance,
                &mut budget,
            )?;
            if polygon::classify(&other.polygon, h.polygon[0], options.uv_tolerance)?
                != Containment::Outside
                || polygon::classify(&h.polygon, other.polygon[0], options.uv_tolerance)?
                    != Containment::Outside
            {
                return Err(err(ErrorKind::IntersectingLoops));
            }
        }
        holes.push(h);
    }
    Ok(TrimmedFace {
        face,
        outer,
        holes,
        tolerance: options.uv_tolerance,
    })
}
fn build_loop(
    brep: &NormalizedBrep,
    face: FaceId,
    wire: WireId,
    o: Options,
    budget: &mut Budget,
) -> Result<TrimLoop, Error> {
    let raw = brep.data();
    let f = &raw.faces[face.0];
    let domains = f.surface.domain();
    let mut uses = Vec::new();
    let mut polygon = Vec::new();
    let mut previous: Option<[f64; 2]> = None;
    for &cid in &raw.wires[wire.0].coedges {
        let c = &raw.coedges[cid.0];
        let build = |budget: &mut Budget| -> Result<Vec<UvSample>, Error> {
            let pc = c.pcurve.as_ref().ok_or(err(ErrorKind::MissingPcurve))?;
            let [p0, p1] = pc.range;
            let range = [p0.min(p1), p0.max(p1)];
            let seeds = pc
                .curve
                .break_parameters(range, budget.samples)
                .map_err(|_| err(ErrorKind::Limit))?;
            budget.work(seeds.len())?;
            let mut fractions: Vec<_> = seeds.into_iter().map(|u| (u - p0) / (p1 - p0)).collect();
            if p1 < p0 {
                fractions.reverse();
            }
            let eval = |s: f64, side: KnotSide, budget: &mut Budget| -> Result<UvSample, Error> {
                budget.sample()?;
                let uv = evaluate_pcurve(pc, s, side)?;
                let surface = f
                    .surface
                    .evaluate(uv)
                    .map_err(|_| err(ErrorKind::Geometry))?;
                surface
                    .normal(NumericalTolerance::default())
                    .map_err(|_| err(ErrorKind::SingularSurface))?;
                let e = &raw.edges[c.edge.0];
                let u = parameter(e.range, s);
                let point = e
                    .curve
                    .evaluate_on_side(u, side)
                    .map_err(|_| err(ErrorKind::Geometry))?
                    .position;
                if point
                    .distance(surface.position)
                    .map_err(|_| err(ErrorKind::Geometry))?
                    > brep.model_tolerance()
                {
                    return Err(err(ErrorKind::CurveSurfaceMismatch));
                }
                Ok(UvSample {
                    edge_fraction: s,
                    uv,
                })
            };
            let mut out = vec![eval(0., KnotSide::Right, budget)?];
            for w in fractions.windows(2) {
                let a = eval(w[0], KnotSide::Right, budget)?;
                let b = eval(w[1], KnotSide::Left, budget)?;
                if polygon::distance(a.uv, out.last().expect("first sample").uv)? > o.uv_tolerance {
                    return Err(err(ErrorKind::DiscontinuousPcurve));
                }
                let mut stack = vec![(a, b, 0)];
                while let Some((a, b, depth)) = stack.pop() {
                    let mut deviation = 0.0_f64;
                    for t in [0.25, 0.5, 0.75] {
                        let p = eval(
                            a.edge_fraction + (b.edge_fraction - a.edge_fraction) * t,
                            KnotSide::Right,
                            budget,
                        )?;
                        deviation = deviation.max(polygon::segment_distance(p.uv, a.uv, b.uv)?);
                    }
                    if deviation <= o.uv_tolerance {
                        out.push(b);
                    } else {
                        if depth >= o.max_depth {
                            return Err(err(ErrorKind::UnresolvedTolerance));
                        }
                        let mid = eval(
                            a.edge_fraction + (b.edge_fraction - a.edge_fraction) * 0.5,
                            KnotSide::Right,
                            budget,
                        )?;
                        if mid.edge_fraction == a.edge_fraction
                            || mid.edge_fraction == b.edge_fraction
                        {
                            return Err(err(ErrorKind::UnresolvedTolerance));
                        }
                        stack.push((mid, b, depth + 1));
                        stack.push((a, mid, depth + 1));
                    }
                }
            }
            if c.orientation.is_reversed() {
                out.reverse();
            }
            Ok(out)
        };
        let mut samples = build(budget).map_err(|mut e| {
            e.coedge = Some(cid);
            e
        })?;
        let mut shift = [0.; 2];
        if let Some(last) = previous {
            for k in 0..2 {
                if let AxisDomain::Periodic { period } = domains[k] {
                    shift[k] = ((last[k] - samples[0].uv[k]) / period).round() * period;
                }
            }
            for s in &mut samples {
                for (k, &offset) in shift.iter().enumerate() {
                    s.uv[k] += offset;
                }
                if s.uv.iter().any(|x| !x.is_finite()) {
                    return Err(err(ErrorKind::Geometry));
                }
            }
            if polygon::distance(last, samples[0].uv)? > o.uv_tolerance {
                return Err(Error {
                    kind: ErrorKind::OpenLoop,
                    coedge: Some(cid),
                });
            }
        }
        previous = Some(samples.last().expect("seed endpoints").uv);
        // Polygon joins use the next coedge's start, within the explicitly checked UV tolerance.
        polygon.extend(samples[..samples.len() - 1].iter().map(|p| p.uv));
        uses.push(BoundaryUse {
            coedge: cid,
            samples,
            period_shift: shift,
        });
    }
    if polygon::distance(previous.expect("nonempty validated wire"), polygon[0])? > o.uv_tolerance {
        let periodic = domains
            .iter()
            .any(|d| matches!(d, AxisDomain::Periodic { .. }));
        return Err(err(if periodic {
            ErrorKind::NonContractibleLoop
        } else {
            ErrorKind::OpenLoop
        }));
    }
    let signed_area = polygon::validate(&polygon, o.uv_tolerance, budget)?;
    Ok(TrimLoop {
        uses,
        polygon,
        signed_area,
    })
}
fn parameter(range: [f64; 2], s: f64) -> f64 {
    if s == 0. {
        range[0]
    } else if s == 1. {
        range[1]
    } else {
        range[0] + s * (range[1] - range[0])
    }
}
fn evaluate_pcurve(pc: &Pcurve, s: f64, mut side: KnotSide) -> Result<[f64; 2], Error> {
    if pc.range[1] < pc.range[0] {
        side = match side {
            KnotSide::Left => KnotSide::Right,
            KnotSide::Right => KnotSide::Left,
        };
    }
    Ok(pc
        .curve
        .evaluate_on_side(parameter(pc.range, s), side)
        .map_err(|_| err(ErrorKind::Geometry))?
        .position
        .coordinates())
}
