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
    /// Pcurve evaluations across all loops of the face, at most one million (the
    /// default: a plane with a thousand sampled holes needs a few hundred thousand).
    pub max_samples: usize,
    /// Logical sampling and polygon-check work.
    pub max_work: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            uv_tolerance: 1e-5,
            max_depth: 24,
            max_samples: 1_000_000,
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
/// Where a reconstruction or polygon check failed. Each field is set where it can be
/// determined: a failure local to one coedge use names it, a conflict between two
/// boundary segments names the coedge of each, and a loop-level failure (winding,
/// area, closure, containment) names its wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// The coedge whose use, or whose boundary segment, failed.
    pub coedge: Option<CoedgeId>,
    /// The coedge of the second segment when two boundary segments cross, touch or
    /// double back on each other (`SelfIntersection`, `IntersectingLoops`).
    pub other: Option<CoedgeId>,
    /// The loop of the failure; for a conflict between two loops, the later one.
    pub wire: Option<WireId>,
    /// For `Limit`, the exhausted budget and its configured value.
    pub budget: Option<Budget>,
}
/// A logical budget of [`Options`] (or of [`validate_polygons`]) with its configured
/// value, named by exhausted-budget errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    /// `Options::max_samples`: pcurve evaluations across the face's loops.
    MaxSamples(usize),
    /// `Options::max_work` (or the `max_work` argument): logical sampling and
    /// polygon-check work.
    MaxWork(usize),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UV trimming {:?}", self.kind)?;
        if let Some(c) = self.coedge {
            write!(f, " at coedge {}", c.0)?;
        }
        if let Some(c) = self.other {
            write!(f, " against coedge {}", c.0)?;
        }
        if let Some(w) = self.wire {
            write!(f, " in wire {}", w.0)?;
        }
        match self.budget {
            Some(Budget::MaxSamples(n)) => write!(f, " (max_samples budget {n} exhausted)"),
            Some(Budget::MaxWork(n)) => write!(f, " (max_work budget {n} exhausted)"),
            None => Ok(()),
        }
    }
}
impl std::error::Error for Error {}
fn err(kind: ErrorKind) -> Error {
    Error {
        kind,
        coedge: None,
        other: None,
        wire: None,
        budget: None,
    }
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
    /// The coedge whose use starts the polygon segment from each point.
    owners: Vec<CoedgeId>,
    wire: WireId,
    chart: Chart,
}
/// The chart in which loop validity is decided. When the surface gives a scale,
/// chart offsets are scaled by the largest sampled surface derivative along each
/// axis (so distances are metres) and segments closer than a thousandth of the model
/// tolerance touch; otherwise the unscaled chart and the UV tolerance are used.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Chart {
    scale: [f64; 2],
    eps: f64,
    /// Extent below which a conflict within one loop is a wobble, not a defect: the
    /// model tolerance in a metric chart, zero (every conflict counts) otherwise.
    noise: f64,
    /// Whether `scale` came from the surface (false: the unscaled fallback).
    metric: bool,
}
impl Chart {
    fn new(scale: [f64; 2], model: f64, uv_tolerance: f64) -> Self {
        if scale.iter().all(|x| *x > 0. && x.is_finite()) {
            Self {
                scale,
                eps: (model * 1e-3).max(1e-12),
                noise: model,
                metric: true,
            }
        } else {
            Self::unscaled(uv_tolerance)
        }
    }
    fn unscaled(uv_tolerance: f64) -> Self {
        Self {
            scale: [1.; 2],
            eps: uv_tolerance,
            noise: 0.,
            metric: false,
        }
    }
    /// The chart of several loops of one face: the larger scale per axis, or the
    /// unscaled chart when any loop has no scale.
    fn join(charts: &[Chart], uv_tolerance: f64) -> Self {
        if charts.is_empty() || charts.iter().any(|c| !c.metric) {
            return Self::unscaled(uv_tolerance);
        }
        let scale = [0, 1].map(|k| charts.iter().map(|c| c.scale[k]).fold(0., f64::max));
        Self { scale, ..charts[0] }
    }
    fn apply<'a>(
        &self,
        points: &'a [[f64; 2]],
        budget: &mut Remaining,
    ) -> Result<std::borrow::Cow<'a, [[f64; 2]]>, Error> {
        if !self.metric {
            return Ok(points.into());
        }
        budget.work(points.len())?;
        Ok(points
            .iter()
            .map(|p| [p[0] * self.scale[0], p[1] * self.scale[1]])
            .collect())
    }
}
/// A loop to check: chart points, the coedge starting the segment from each point
/// (when known), its wire and its chart.
struct Points<'a> {
    points: &'a [[f64; 2]],
    owners: Option<&'a [CoedgeId]>,
    wire: Option<WireId>,
    chart: Chart,
}
impl Points<'_> {
    fn owner(&self, segment: Option<usize>) -> Option<CoedgeId> {
        Some(*self.owners?.get(segment?)?)
    }
}
/// Self-validation of one loop in its chart; returns the signed area in chart units.
fn check_loop(l: &Points<'_>, budget: &mut Remaining) -> Result<f64, Error> {
    let scaled = l.chart.apply(l.points, budget)?;
    let area =
        polygon::validate(&scaled, l.chart.eps, l.chart.noise, budget).map_err(|d| Error {
            coedge: l.owner(d.at),
            other: l.owner(d.other),
            wire: d.error.wire.or(l.wire),
            ..d.error
        })?;
    Ok(area / (l.chart.scale[0] * l.chart.scale[1]))
}
/// Holes must lie strictly inside the outer loop, apart from it and from each other,
/// and unnested, decided in the face chart. Hole pairs are compared only when their
/// boxes overlap.
fn check_holes(
    outer: &Points<'_>,
    holes: &[Points<'_>],
    chart: Chart,
    budget: &mut Remaining,
) -> Result<(), Error> {
    if holes.is_empty() {
        return Ok(());
    }
    let eps = chart.eps;
    let o = chart.apply(outer.points, budget)?;
    let mut scaled = Vec::with_capacity(holes.len());
    for h in holes {
        let p = chart.apply(h.points, budget)?;
        polygon::disjoint(&o, &p, eps, budget).map_err(|d| Error {
            coedge: outer.owner(d.at),
            other: h.owner(d.other),
            wire: h.wire,
            ..d.error
        })?;
        budget.work(o.len())?;
        if polygon::classify(&o, p[0], eps)? != Containment::Inside {
            return Err(Error {
                wire: h.wire,
                ..err(ErrorKind::HoleOutside)
            });
        }
        scaled.push(p);
    }
    let boxes: Vec<polygon::Bounds> = scaled
        .iter()
        .map(|p| polygon::Bounds::of(p.iter().copied()))
        .collect();
    let mut failure = None;
    polygon::sweep(&boxes, eps, budget, |a, b, budget| {
        let located = |d: polygon::Defect| Error {
            coedge: holes[a].owner(d.at),
            other: holes[b].owner(d.other),
            wire: holes[b].wire,
            ..d.error
        };
        if let Err(d) = polygon::disjoint(&scaled[a], &scaled[b], eps, budget) {
            failure = Some(located(d));
            return Ok(true);
        }
        budget.work(scaled[a].len().saturating_add(scaled[b].len()))?;
        if polygon::classify(&scaled[a], scaled[b][0], eps)? != Containment::Outside
            || polygon::classify(&scaled[b], scaled[a][0], eps)? != Containment::Outside
        {
            failure = Some(Error {
                wire: holes[b].wire,
                ..err(ErrorKind::IntersectingLoops)
            });
            return Ok(true);
        }
        Ok(false)
    })
    .map_err(|d| d.error)?;
    failure.map_or(Ok(()), Err)
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
/// Remaining budgets, with their configured values for diagnostics.
struct Remaining {
    work: usize,
    samples: usize,
    max_work: usize,
    max_samples: usize,
}
impl Remaining {
    fn new(max_work: usize, max_samples: usize) -> Self {
        Self {
            work: max_work,
            samples: max_samples,
            max_work,
            max_samples,
        }
    }
    fn exhausted(budget: Budget) -> Error {
        Error {
            budget: Some(budget),
            ..err(ErrorKind::Limit)
        }
    }
    fn work(&mut self, n: usize) -> Result<(), Error> {
        match self.work.checked_sub(n) {
            Some(rest) => {
                self.work = rest;
                Ok(())
            }
            None => Err(Self::exhausted(Budget::MaxWork(self.max_work))),
        }
    }
    fn sample(&mut self) -> Result<(), Error> {
        match self.samples.checked_sub(1) {
            Some(rest) => self.samples = rest,
            None => return Err(Self::exhausted(Budget::MaxSamples(self.max_samples))),
        }
        self.work(1)
    }
}
/// Validate polygons at a caller's actual boundary resolution. Outer winding must
/// be CCW, holes CW, strictly contained, disjoint and unnested. No vertices are
/// removed or moved. Predicates use finite f64 arithmetic and a UV distance tolerance.
/// Segment pairs and hole pairs are compared only when their boxes overlap within
/// the tolerance (a sweep along the wider axis), and each comparison charges work.
pub fn validate_polygons(
    outer: &[[f64; 2]],
    holes: &[Vec<[f64; 2]>],
    uv_tolerance: f64,
    max_work: usize,
) -> Result<(), Error> {
    if !uv_tolerance.is_finite() || uv_tolerance <= 0. {
        return Err(err(ErrorKind::InvalidOptions));
    }
    let mut budget = Remaining::new(max_work, 0);
    let chart = Chart::unscaled(uv_tolerance);
    fn points(points: &[[f64; 2]], chart: Chart) -> Points<'_> {
        Points {
            points,
            owners: None,
            wire: None,
            chart,
        }
    }
    let holes: Vec<Points<'_>> = holes.iter().map(|h| points(h, chart)).collect();
    check_loops(&points(outer, chart), &holes, chart, &mut budget)
}
/// Winding and self-validation of every loop in its own chart, then the hole
/// relationships in the face chart.
fn check_loops(
    outer: &Points<'_>,
    holes: &[Points<'_>],
    chart: Chart,
    budget: &mut Remaining,
) -> Result<(), Error> {
    if check_loop(outer, budget)? <= 0. {
        return Err(Error {
            wire: outer.wire,
            ..err(ErrorKind::WrongOrientation)
        });
    }
    for h in holes {
        if check_loop(h, budget)? >= 0. {
            return Err(Error {
                wire: h.wire,
                ..err(ErrorKind::WrongOrientation)
            });
        }
    }
    check_holes(outer, holes, chart, budget)
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
    let mut budget = Remaining::new(options.max_work, options.max_samples);
    let mut outer = build_loop(brep, face, f.outer, options, &mut budget)?;
    outer.signed_area = check_loop(&outer.points(), &mut budget)?;
    if outer.signed_area <= 0. {
        return Err(Error {
            wire: Some(f.outer),
            ..err(ErrorKind::WrongOrientation)
        });
    }
    let mut holes = vec![];
    let domains = f.surface.domain();
    for &wire in &f.holes {
        let mut h = build_loop(brep, face, wire, options, &mut budget)?;
        place_hole(&outer, &mut h, domains, options.uv_tolerance, &mut budget).map_err(|e| {
            Error {
                wire: Some(wire),
                ..e
            }
        })?;
        h.signed_area = check_loop(&h.points(), &mut budget)?;
        if h.signed_area >= 0. {
            return Err(Error {
                wire: Some(wire),
                ..err(ErrorKind::WrongOrientation)
            });
        }
        holes.push(h);
    }
    let trimmed = TrimmedFace {
        face,
        outer,
        holes,
        tolerance: options.uv_tolerance,
    };
    let holes: Vec<Points<'_>> = trimmed.holes.iter().map(TrimLoop::points).collect();
    check_holes(
        &trimmed.outer.points(),
        &holes,
        trimmed.chart(),
        &mut budget,
    )?;
    Ok(trimmed)
}
impl TrimLoop {
    fn points(&self) -> Points<'_> {
        Points {
            points: &self.polygon,
            owners: Some(&self.owners),
            wire: Some(self.wire),
            chart: self.chart,
        }
    }
}
impl TrimmedFace {
    fn chart(&self) -> Chart {
        let charts: Vec<Chart> = std::iter::once(&self.outer)
            .chain(&self.holes)
            .map(|l| l.chart)
            .collect();
        Chart::join(&charts, self.tolerance)
    }
    /// Check this face's loops again at another boundary resolution, such as the
    /// shared edge samples a mesh uses, with the checks of [`validate_polygons`] in
    /// the unscaled chart at the caller's `uv_tolerance`. Validity at the model
    /// tolerance was established by the reconstruction at its own resolution; a
    /// denser resolution may legitimately place samples closer than that, so the
    /// caller chooses the resolution its consumer needs (a triangulator: simple,
    /// correctly nested loops at floating-point resolution). `loops` lists the outer
    /// loop, then the holes in order; each point carries the coedge whose use starts
    /// the segment from it. Failures name that coedge, the other segment's coedge
    /// and the loop's wire. Comparisons charge `max_work`.
    pub fn validate_resolution(
        &self,
        loops: &[Vec<([f64; 2], CoedgeId)>],
        uv_tolerance: f64,
        max_work: usize,
    ) -> Result<(), Error> {
        if !uv_tolerance.is_finite() || uv_tolerance <= 0. {
            return Err(err(ErrorKind::InvalidOptions));
        }
        if loops.len() != 1 + self.holes.len() {
            return Err(err(ErrorKind::InvalidFace));
        }
        let chart = Chart::unscaled(uv_tolerance);
        let mut budget = Remaining::new(max_work, 0);
        budget.work(loops.iter().map(Vec::len).sum())?;
        let split: Vec<(Vec<[f64; 2]>, Vec<CoedgeId>)> =
            loops.iter().map(|l| l.iter().copied().unzip()).collect();
        let points: Vec<Points<'_>> = std::iter::once(&self.outer)
            .chain(&self.holes)
            .zip(&split)
            .map(|(trim, (points, owners))| Points {
                points,
                owners: Some(owners),
                wire: Some(trim.wire),
                chart,
            })
            .collect();
        check_loops(&points[0], &points[1..], chart, &mut budget)
    }
}
/// Translate a hole by whole periods of the periodic surface axes into the chart of
/// the outer loop when its first point is not already inside. Candidate shifts are
/// the nearest-center shift and its immediate neighbours; the geometry is unchanged.
fn place_hole(
    outer: &TrimLoop,
    hole: &mut TrimLoop,
    domains: [AxisDomain; 2],
    eps: f64,
    budget: &mut Remaining,
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
/// Sample one loop; every failure names the loop's wire. Validity is checked by
/// the caller in the returned loop's chart.
fn build_loop(
    brep: &NormalizedBrep,
    face: FaceId,
    wire: WireId,
    o: Options,
    budget: &mut Remaining,
) -> Result<TrimLoop, Error> {
    trace_loop(brep, face, wire, o, budget).map_err(|e| Error {
        wire: Some(wire),
        ..e
    })
}
fn trace_loop(
    brep: &NormalizedBrep,
    face: FaceId,
    wire: WireId,
    o: Options,
    budget: &mut Remaining,
) -> Result<TrimLoop, Error> {
    let raw = brep.data();
    let f = &raw.faces[face.0];
    let domains = f.surface.domain();
    let mut uses = Vec::new();
    let mut polygon = Vec::new();
    let mut owners = Vec::new();
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
        let build = |budget: &mut Remaining| -> Result<Vec<UvSample>, Error> {
            let pc = c.pcurve.as_ref().ok_or(err(ErrorKind::MissingPcurve))?;
            let [p0, p1] = pc.range;
            let range = [p0.min(p1), p0.max(p1)];
            let seeds = pc
                .curve
                .break_parameters(range, budget.samples)
                .map_err(|e| match e {
                    tessstep_topology::GeometryError::Spline(
                        tessstep_curves::spline::SplineError::ResourceLimit,
                    ) => Remaining::exhausted(Budget::MaxSamples(budget.max_samples)),
                    _ => err(ErrorKind::Geometry),
                })?;
            budget.work(seeds.len())?;
            let mut fractions: Vec<_> = seeds.into_iter().map(|u| (u - p0) / (p1 - p0)).collect();
            if p1 < p0 {
                fractions.reverse();
            }
            // Seeds within numerical noise of each other or of the range ends (a knot
            // written a few ulps from an edge end) would add a segment of no length,
            // which the polygon checks reject; shared-edge sampling drops them too.
            let mut kept: Vec<f64> = Vec::with_capacity(fractions.len());
            for &f in &fractions {
                let interior = f != 0. && f != 1.;
                if interior && (f <= 1e-9 || f >= 1. - 1e-9) {
                    continue;
                }
                if kept.last().is_some_and(|&last| f - last <= 1e-9) {
                    if interior {
                        continue;
                    }
                    kept.pop();
                }
                kept.push(f);
            }
            let fractions = kept;
            let e = &raw.edges[c.edge.0];
            let collapsed = brep.is_collapsed(c.edge);
            let eval =
                |s: f64, side: KnotSide, budget: &mut Remaining| -> Result<UvSample, Error> {
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
                    // A loop much smaller than the UV tolerance (a sub-micrometre face in
                    // a metre chart) would otherwise keep a chord across its whole span:
                    // deviations must also stay within a quarter of the chord, down to a
                    // thousandth of the tolerance, so its polygon keeps its shape.
                    let chord = polygon::distance(a.uv, b.uv)?;
                    if deviation <= o.uv_tolerance
                        && (deviation <= 0.25 * chord || deviation <= 1e-3 * o.uv_tolerance)
                    {
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
                    coedge: Some(cid),
                    ..err(ErrorKind::OpenLoop)
                });
            }
        }
        previous = Some(samples.last().expect("seed endpoints").uv);
        // Polygon joins use the next coedge's start, within the explicitly checked UV tolerance.
        polygon.extend(samples[..samples.len() - 1].iter().map(|p| p.uv));
        owners.extend(std::iter::repeat_n(cid, samples.len() - 1));
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
    // Validity is decided in metres when the surface gave a scale, so that a corner of
    // a few nanometres in one axis is not a backtrack; the area keeps its chart units.
    Ok(TrimLoop {
        uses,
        polygon,
        signed_area: 0.,
        owners,
        wire,
        chart: Chart::new(scale.get(), model, o.uv_tolerance),
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
    budget: &mut Remaining,
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
