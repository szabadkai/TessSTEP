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
/// Validate polygons at a caller's actual boundary resolution. Outer winding must
/// be CCW, holes CW, strictly contained, disjoint and unnested. No vertices are
/// removed or moved. Predicates use finite f64 arithmetic and a UV distance tolerance.
pub fn validate_polygons(
    outer: &[[f64; 2]],
    holes: &[Vec<[f64; 2]>],
    uv_tolerance: f64,
    max_work: usize,
) -> Result<(), Error> {
    if !uv_tolerance.is_finite() || uv_tolerance <= 0. {
        return Err(err(ErrorKind::InvalidOptions));
    }
    let mut budget = Budget {
        work: max_work,
        samples: 0,
    };
    if polygon::validate(outer, uv_tolerance, &mut budget)? <= 0. {
        return Err(err(ErrorKind::WrongOrientation));
    }
    for (i, hole) in holes.iter().enumerate() {
        if polygon::validate(hole, uv_tolerance, &mut budget)? >= 0. {
            return Err(err(ErrorKind::WrongOrientation));
        }
        validate_hole(
            outer,
            hole,
            holes[..i].iter().map(Vec::as_slice),
            uv_tolerance,
            &mut budget,
        )?;
    }
    Ok(())
}
fn validate_hole<'a>(
    outer: &[[f64; 2]],
    hole: &[[f64; 2]],
    previous: impl Iterator<Item = &'a [[f64; 2]]>,
    eps: f64,
    budget: &mut Budget,
) -> Result<(), Error> {
    polygon::disjoint(outer, hole, eps, budget)?;
    budget.work(outer.len())?;
    if polygon::classify(outer, hole[0], eps)? != Containment::Inside {
        return Err(err(ErrorKind::HoleOutside));
    }
    for other in previous {
        polygon::disjoint(other, hole, eps, budget)?;
        budget.work(other.len().saturating_add(hole.len()))?;
        if polygon::classify(other, hole[0], eps)? != Containment::Outside
            || polygon::classify(hole, other[0], eps)? != Containment::Outside
        {
            return Err(err(ErrorKind::IntersectingLoops));
        }
    }
    Ok(())
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
    let domains = f.surface.domain();
    for &wire in &f.holes {
        let mut h = build_loop(brep, face, wire, options, &mut budget)?;
        place_hole(&outer, &mut h, domains, options.uv_tolerance, &mut budget)?;
        if h.signed_area >= 0. {
            return Err(err(ErrorKind::WrongOrientation));
        }
        validate_hole(
            &outer.polygon,
            &h.polygon,
            holes
                .iter()
                .map(|other: &TrimLoop| other.polygon.as_slice()),
            options.uv_tolerance,
            &mut budget,
        )?;
        holes.push(h);
    }
    Ok(TrimmedFace {
        face,
        outer,
        holes,
        tolerance: options.uv_tolerance,
    })
}
/// Translate a hole by whole periods of the periodic surface axes into the chart of
/// the outer loop when its first point is not already inside. Candidate shifts are
/// the nearest-center shift and its immediate neighbours; the geometry is unchanged.
fn place_hole(
    outer: &TrimLoop,
    hole: &mut TrimLoop,
    domains: [AxisDomain; 2],
    eps: f64,
    budget: &mut Budget,
) -> Result<(), Error> {
    let periods = domains.map(|d| match d {
        AxisDomain::Periodic { period } => Some(period),
        _ => None,
    });
    if periods == [None, None] {
        return Ok(());
    }
    budget.work(outer.polygon.len())?;
    if polygon::classify(&outer.polygon, hole.polygon[0], eps)? == Containment::Inside {
        return Ok(());
    }
    let center = |p: &[[f64; 2]], k: usize| {
        let (lo, hi) = p
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), q| {
                (lo.min(q[k]), hi.max(q[k]))
            });
        0.5 * (lo + hi)
    };
    let base = [0, 1].map(|k| {
        periods[k].map_or(0., |p| {
            ((center(&outer.polygon, k) - center(&hole.polygon, k)) / p).round()
        })
    });
    for du in [0., -1., 1.] {
        for dv in [0., -1., 1.] {
            let steps = [base[0] + du, base[1] + dv];
            if (periods[0].is_none() && du != 0.) || (periods[1].is_none() && dv != 0.) {
                continue;
            }
            let shift = [0, 1].map(|k| periods[k].map_or(0., |p| steps[k] * p));
            if shift == [0., 0.] {
                continue;
            }
            let first = [hole.polygon[0][0] + shift[0], hole.polygon[0][1] + shift[1]];
            budget.work(outer.polygon.len())?;
            if polygon::classify(&outer.polygon, first, eps)? != Containment::Inside {
                continue;
            }
            budget.work(hole.polygon.len())?;
            for p in &mut hole.polygon {
                p[0] += shift[0];
                p[1] += shift[1];
            }
            for u in &mut hole.uses {
                for s in &mut u.samples {
                    s.uv[0] += shift[0];
                    s.uv[1] += shift[1];
                }
                u.period_shift[0] += shift[0];
                u.period_shift[1] += shift[1];
            }
            return Ok(());
        }
    }
    Ok(())
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
    // Chart axes have unrelated units (radians on a millimetre tube, metres along a
    // cylinder), so joins and polygon validity are decided in metres: chart offsets
    // are scaled by the largest sampled surface derivative along each axis.
    let scale = std::cell::Cell::new([0.0f64; 2]);
    let metric = |a: [f64; 2], b: [f64; 2]| -> f64 {
        let s = scale.get();
        ((a[0] - b[0]) * s[0]).hypot((a[1] - b[1]) * s[1])
    };
    let model = brep.model_tolerance();
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
            let e = &raw.edges[c.edge.0];
            let collapsed = brep.is_collapsed(c.edge);
            let eval = |s: f64, side: KnotSide, budget: &mut Budget| -> Result<UvSample, Error> {
                budget.sample()?;
                let uv = evaluate_pcurve(pc, s, side)?;
                let surface = f
                    .surface
                    .evaluate(uv)
                    .map_err(|_| err(ErrorKind::Geometry))?;
                let mut sc = scale.get();
                for (k, d) in [surface.du, surface.dv].into_iter().enumerate() {
                    if let Ok(n) = d.norm() {
                        sc[k] = sc[k].max(n);
                    }
                }
                scale.set(sc);
                // Collapsed edges and their vertices lie on singular chart lines.
                let singular = collapsed
                    || (s == 0. && brep.is_singular_vertex(e.vertices[0]))
                    || (s == 1. && brep.is_singular_vertex(e.vertices[1]));
                if !singular {
                    surface
                        .normal(NumericalTolerance::default())
                        .map_err(|_| err(ErrorKind::SingularSurface))?;
                }
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
                let last = out.last().expect("first sample").uv;
                if polygon::distance(a.uv, last)? > o.uv_tolerance && metric(a.uv, last) > model {
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
            if polygon::distance(last, samples[0].uv)? > o.uv_tolerance
                && metric(last, samples[0].uv) > model
                && !joins_in_space(brep, &f.surface, last, samples[0].uv, budget)?
            {
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
    let last = previous.expect("nonempty validated wire");
    if polygon::distance(last, polygon[0])? > o.uv_tolerance && metric(last, polygon[0]) > model {
        // A gap near a whole period is a loop that winds the chart; a small gap whose
        // ends coincide in space is a join within the model tolerance.
        let winds = (0..2).any(|k| match domains[k] {
            AxisDomain::Periodic { period } => (last[k] - polygon[0][k]).abs() > 0.25 * period,
            _ => false,
        });
        if winds || !joins_in_space(brep, &f.surface, last, polygon[0], budget)? {
            let periodic = domains
                .iter()
                .any(|d| matches!(d, AxisDomain::Periodic { .. }));
            return Err(err(if periodic {
                ErrorKind::NonContractibleLoop
            } else {
                ErrorKind::OpenLoop
            }));
        }
    }
    // Validate in metres when the surface gave a scale, so that a corner of a few
    // nanometres in one axis is not a backtrack; the area keeps its chart units.
    let s = scale.get();
    let signed_area = if s.iter().all(|x| *x > 0. && x.is_finite()) {
        budget.work(polygon.len())?;
        let scaled: Vec<[f64; 2]> = polygon.iter().map(|p| [p[0] * s[0], p[1] * s[1]]).collect();
        polygon::validate(&scaled, (model * 1e-3).max(1e-12), budget)? / (s[0] * s[1])
    } else {
        polygon::validate(&polygon, o.uv_tolerance, budget)?
    };
    Ok(TrimLoop {
        uses,
        polygon,
        signed_area,
    })
}
/// Whether two chart points that miss each other in UV still coincide on the surface
/// within the model tolerance. Vertices may sit up to that far from their curves, so
/// computed pcurves meet a seam or each other with a matching UV gap. Singular
/// points map every chart coordinate to one position and never qualify.
fn joins_in_space(
    brep: &NormalizedBrep,
    surface: &tessstep_topology::SurfaceGeometry,
    a: [f64; 2],
    b: [f64; 2],
    budget: &mut Budget,
) -> Result<bool, Error> {
    budget.work(2)?;
    let mut points = Vec::new();
    for uv in [a, b] {
        let Ok(jet) = surface.evaluate(uv) else {
            return Ok(false);
        };
        if jet.normal(NumericalTolerance::default()).is_err() {
            return Ok(false);
        }
        points.push(jet.position);
    }
    Ok(points[0]
        .distance(points[1])
        .map_err(|_| err(ErrorKind::Geometry))?
        <= brep.model_tolerance())
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
