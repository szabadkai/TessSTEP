//! Computed pcurves for edge uses on face surfaces.
//!
//! Elementary surfaces are inverted in closed form and NURBS surfaces by bounded
//! Gauss-Newton iteration. Curve/surface pairs with a linear or conic UV image (every
//! planar curve, generators and coaxial circles of revolved surfaces) receive that exact
//! image; every other pair receives a piecewise cubic Hermite image, subdivided until
//! its lift agrees with the exact inverse to a quarter of the model tolerance. The
//! pcurve parameter is always the 3D edge parameter, so the affine correspondence
//! required by trimming and tessellation is the identity. Supplied STEP pcurves are
//! never read.
use std::f64::consts::TAU;
use tessstep_curves::{
    Curve, PlaneFrame,
    spline::{KnotSide, NurbsCurve, SplineLimits},
};
use tessstep_math::{Length, ModelSpace, NumericalTolerance, ParameterSpace, Point, Vector};
use tessstep_surfaces::NurbsSurface;
use tessstep_topology::{CurveGeometry, Pcurve, SurfaceGeometry};

pub(crate) type V3 = [f64; 3];
pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn distance(a: V3, b: V3) -> f64 {
    norm(sub(a, b))
}
/// The representative of `value` modulo `period` nearest to `hint`.
pub(crate) fn nearest(value: f64, hint: f64, period: f64) -> f64 {
    value + period * ((hint - value) / period).round()
}

/// Orthonormal right-handed frame in metres.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub o: V3,
    pub x: V3,
    pub y: V3,
    pub z: V3,
}
impl Frame {
    pub(crate) fn new(frame: PlaneFrame<ModelSpace, 3>) -> Self {
        let x = frame.x().components();
        let y = frame.y().components();
        Self {
            o: frame.origin().coordinates(),
            x,
            y,
            z: cross(x, y),
        }
    }
    fn local(&self, p: V3) -> V3 {
        let d = sub(p, self.o);
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }
    fn angle_of(&self, direction: V3) -> f64 {
        dot(direction, self.y).atan2(dot(direction, self.x))
    }
}

/// Construction parameters of a face surface, retained for inversion.
#[derive(Clone, Debug)]
pub(crate) enum Chart {
    Plane(Frame),
    Cylinder(Frame, f64),
    /// Frame origin at the apex; semi-angle in radians; v is slant distance.
    Cone(Frame, f64),
    Sphere(Frame, f64),
    Torus(Frame, f64, f64),
    /// Periods of the axes declared periodic on the kernel surface, and the domain
    /// sides that collapse to a point.
    Nurbs([Option<f64>; 2], Vec<Singular>),
}
impl Chart {
    pub(crate) fn periods(&self) -> [Option<f64>; 2] {
        match self {
            Self::Plane(_) => [None, None],
            Self::Nurbs(periods, _) => *periods,
            Self::Cylinder(..) | Self::Cone(..) | Self::Sphere(..) => [Some(TAU), None],
            Self::Torus(..) => [Some(TAU), Some(TAU)],
        }
    }
    /// Lines of the chart along which the surface collapses to one point.
    pub(crate) fn singular(&self) -> Vec<Singular> {
        use std::f64::consts::FRAC_PI_2;
        match *self {
            Self::Sphere(f, r) => vec![
                Singular {
                    k: 0,
                    value: -FRAC_PI_2,
                    point: sub(f.o, scale(f.z, r)),
                    side: 1.,
                },
                Singular {
                    k: 0,
                    value: FRAC_PI_2,
                    point: add(f.o, scale(f.z, r)),
                    side: -1.,
                },
            ],
            Self::Cone(f, _) => vec![Singular {
                k: 0,
                value: 0.,
                point: f.o,
                side: 1.,
            }],
            Self::Nurbs(_, ref singular) => singular.clone(),
            _ => Vec::new(),
        }
    }
    fn frame(&self) -> Option<&Frame> {
        match self {
            Self::Plane(f)
            | Self::Cylinder(f, _)
            | Self::Cone(f, _)
            | Self::Sphere(f, _)
            | Self::Torus(f, ..) => Some(f),
            Self::Nurbs(..) => None,
        }
    }
}

/// Domain sides of a B-spline surface that collapse to one point within `tolerance`,
/// sampled at nine parameters along each side (any knot vector).
pub(crate) fn nurbs_singular(surface: &NurbsSurface<ModelSpace>, tolerance: f64) -> Vec<Singular> {
    let domain = surface.knot_vectors().map(|k| k.domain());
    let mut out = Vec::new();
    for k in 0..2 {
        let j = 1 - k;
        for (end, side) in [(0, 1.), (1, -1.)] {
            let mut points = Vec::new();
            for i in 0..=8 {
                let mut uv = [0.; 2];
                uv[k] = domain[k][0] + (domain[k][1] - domain[k][0]) * i as f64 / 8.;
                uv[j] = domain[j][end];
                match surface.evaluate(uv[0], uv[1]) {
                    Ok(e) => points.push(e.position.coordinates()),
                    Err(_) => break,
                }
            }
            if points.len() == 9 && points.iter().all(|p| distance(*p, points[4]) <= tolerance) {
                out.push(Singular {
                    k,
                    value: domain[j][end],
                    point: points[4],
                    side,
                });
            }
        }
    }
    out
}

/// A pole or apex line of a chart: `uv[1 - k] == value` along axis `k`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Singular {
    pub k: usize,
    pub value: f64,
    /// The surface point the whole line maps to.
    pub point: V3,
    /// +1 when the surface lies at larger `uv[1 - k]` than the line, else -1.
    pub side: f64,
}

/// Importer-side description of an edge curve, retained for exact UV images.
#[derive(Clone, Debug)]
pub(crate) enum CurveShape {
    Line {
        origin: V3,
        tangent: V3,
    },
    /// Circle when `a == b`; P(t) = o + a cos t x + b sin t y.
    Conic {
        frame: Frame,
        a: f64,
        b: f64,
    },
    Nurbs,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PcurveError {
    /// The 3D edge point at `parameter` lies `distance` metres from the surface.
    OffSurface {
        parameter: f64,
        distance: f64,
    },
    /// The surface parameterization is singular where the edge crosses it.
    Singular {
        parameter: f64,
    },
    /// Subdivision reached its depth limit without meeting the lift tolerance.
    Unresolved,
    Limit,
    Geometry,
}

pub(crate) struct Work {
    pub remaining: usize,
}
impl Work {
    fn charge(&mut self, n: usize) -> Result<(), PcurveError> {
        self.remaining = self.remaining.checked_sub(n).ok_or(PcurveError::Limit)?;
        Ok(())
    }
}

pub(crate) struct Jet {
    pub p: V3,
    pub du: V3,
    pub dv: V3,
}
pub(crate) fn surface_jet(
    surface: &SurfaceGeometry,
    uv: [f64; 2],
    work: &mut Work,
) -> Result<Jet, PcurveError> {
    work.charge(1)?;
    let e = surface.evaluate(uv).map_err(|_| PcurveError::Geometry)?;
    Ok(Jet {
        p: e.position.coordinates(),
        du: e.du.components(),
        dv: e.dv.components(),
    })
}
fn curve_jet(
    curve: &CurveGeometry<ModelSpace, 3>,
    t: f64,
    side: KnotSide,
    work: &mut Work,
) -> Result<(V3, V3), PcurveError> {
    work.charge(1)?;
    let e = curve
        .evaluate_on_side(t, side)
        .map_err(|_| PcurveError::Geometry)?;
    Ok((e.position.coordinates(), e.first.components()))
}

/// Inverse of an elementary chart, periodic coordinates nearest `hint` when given.
fn invert_elementary(chart: &Chart, p: V3, hint: Option<[f64; 2]>) -> [f64; 2] {
    let f = chart.frame().expect("elementary chart");
    let l = f.local(p);
    let rho = l[0].hypot(l[1]);
    let u = l[1].atan2(l[0]);
    let uv = match *chart {
        Chart::Plane(_) => return [l[0], l[1]],
        Chart::Cylinder(..) => [u, l[2]],
        Chart::Cone(_, alpha) => [u, rho * alpha.sin() + l[2] * alpha.cos()],
        Chart::Sphere(..) => [u, l[2].atan2(rho)],
        Chart::Torus(_, major, _) => [u, l[2].atan2(rho - major)],
        Chart::Nurbs(..) => unreachable!("elementary chart"),
    };
    let mut uv = uv;
    if let Some(h) = hint {
        for (k, period) in chart.periods().into_iter().enumerate() {
            if let Some(period) = period {
                uv[k] = nearest(uv[k], h[k], period);
            }
        }
    }
    uv
}

/// Gauss-Newton closest point on a NURBS surface, clamped to its knot domain.
fn newton(
    surface: &SurfaceGeometry,
    domain: [[f64; 2]; 2],
    p: V3,
    mut uv: [f64; 2],
    work: &mut Work,
) -> Result<([f64; 2], f64), PcurveError> {
    let periodic = match surface {
        SurfaceGeometry::Nurbs(n) => n.periodic_axes(),
        SurfaceGeometry::Analytic(_) => [false; 2],
    };
    let clamp = |uv: [f64; 2]| {
        [0, 1].map(|k| {
            if periodic[k] {
                uv[k]
            } else {
                uv[k].clamp(domain[k][0], domain[k][1])
            }
        })
    };
    uv = clamp(uv);
    let mut jet = surface_jet(surface, uv, work)?;
    let mut nudged = false;
    for _ in 0..32 {
        let r = sub(jet.p, p);
        let (a, b, c) = (
            dot(jet.du, jet.du),
            dot(jet.du, jet.dv),
            dot(jet.dv, jet.dv),
        );
        let (g0, g1) = (dot(jet.du, r), dot(jet.dv, r));
        let det = a * c - b * b;
        if !det.is_finite() || det <= 1e-24 * a.max(c) * a.max(c) {
            // A seed on a collapsed side has a singular Jacobian: step once toward
            // the domain centre, where the surface is regular.
            if nudged {
                break;
            }
            nudged = true;
            let centre = [0, 1].map(|k| 0.5 * (domain[k][0] + domain[k][1]));
            uv = clamp([0, 1].map(|k| uv[k] + 1e-6 * (centre[k] - uv[k])));
            jet = surface_jet(surface, uv, work)?;
            continue;
        }
        let step = [-(c * g0 - b * g1) / det, -(a * g1 - b * g0) / det];
        let next = clamp([uv[0] + step[0], uv[1] + step[1]]);
        if next == uv {
            break;
        }
        let candidate = surface_jet(surface, next, work)?;
        if distance(candidate.p, p) > distance(jet.p, p) {
            break;
        }
        let moved = (next[0] - uv[0]).abs().max((next[1] - uv[1]).abs());
        uv = next;
        jet = candidate;
        let span = (domain[0][1] - domain[0][0]).max(domain[1][1] - domain[1][0]);
        if moved <= span * 1e-15 {
            break;
        }
    }
    Ok((uv, distance(jet.p, p)))
}

fn invert_nurbs(
    surface: &SurfaceGeometry,
    nurbs: &NurbsSurface<ModelSpace>,
    p: V3,
    hint: Option<[f64; 2]>,
    accept: f64,
    work: &mut Work,
) -> Result<[f64; 2], PcurveError> {
    let domain = nurbs.knot_vectors().map(|k| k.domain());
    if let Some(h) = hint {
        let (uv, d) = newton(surface, domain, p, h, work)?;
        if d <= accept {
            return Ok(uv);
        }
    }
    // Global seed: a grid over every knot span (subsampled on dense knot vectors).
    let axes = nurbs.knot_vectors().map(|k| {
        let mut breaks: Vec<f64> = k
            .knots()
            .iter()
            .copied()
            .filter(|&t| t >= k.domain()[0] && t <= k.domain()[1])
            .collect();
        breaks.dedup();
        let stride = breaks.len().div_ceil(24).max(1);
        let mut breaks: Vec<f64> = breaks
            .iter()
            .step_by(stride)
            .copied()
            .chain(std::iter::once(k.domain()[1]))
            .collect();
        breaks.dedup();
        let mut out = Vec::new();
        for w in breaks.windows(2) {
            for j in 0..8 {
                out.push(w[0] + (w[1] - w[0]) * j as f64 / 8.);
            }
        }
        out.push(k.domain()[1]);
        out
    });
    // Newton from the few nearest grid samples: on closed surfaces the nearest
    // sample may sit on the far side of a clamped seam.
    let mut seeds: Vec<(f64, [f64; 2])> = Vec::new();
    for &u in &axes[0] {
        for &v in &axes[1] {
            let d = distance(surface_jet(surface, [u, v], work)?.p, p);
            seeds.push((d, [u, v]));
        }
    }
    seeds.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Seeds with distinct distances: a collapsed side repeats one point many times.
    seeds.dedup_by(|a, b| a.0 == b.0);
    let mut best: Option<([f64; 2], f64)> = None;
    for &(_, seed) in seeds.iter().take(6) {
        let (uv, d) = newton(surface, domain, p, seed, work)?;
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((uv, d));
        }
    }
    if let Some(h) = hint {
        let (uv, d) = newton(surface, domain, p, h, work)?;
        if best.is_none_or(|(_, bd)| d <= bd) {
            best = Some((uv, d));
        }
    }
    Ok(best.expect("nonempty seed grid").0)
}

pub(crate) struct Inverter<'a> {
    pub chart: &'a Chart,
    pub surface: &'a SurfaceGeometry,
    /// Model distance tolerance in metres.
    pub tolerance: f64,
}
impl Inverter<'_> {
    pub(crate) fn invert(
        &self,
        p: V3,
        hint: Option<[f64; 2]>,
        work: &mut Work,
    ) -> Result<[f64; 2], PcurveError> {
        match (self.chart, self.surface) {
            (Chart::Nurbs(periods, _), SurfaceGeometry::Nurbs(n)) => {
                let mut uv = invert_nurbs(self.surface, n, p, hint, self.tolerance, work)?;
                if let Some(h) = hint {
                    for k in 0..2 {
                        if let Some(period) = periods[k] {
                            uv[k] = nearest(uv[k], h[k], period);
                        }
                    }
                }
                Ok(uv)
            }
            (Chart::Nurbs(..), _) => Err(PcurveError::Geometry),
            _ => {
                work.charge(1)?;
                Ok(invert_elementary(self.chart, p, hint))
            }
        }
    }
    /// UV derivative of the surface preimage of a curve with 3D tangent `d`.
    fn derivative(
        &self,
        uv: [f64; 2],
        d: V3,
        parameter: f64,
        work: &mut Work,
    ) -> Result<[f64; 2], PcurveError> {
        let j = surface_jet(self.surface, uv, work)?;
        let (a, b, c) = (dot(j.du, j.du), dot(j.du, j.dv), dot(j.dv, j.dv));
        let det = a * c - b * b;
        if !det.is_finite() || det <= 1e-24 * a * c {
            return Err(PcurveError::Singular { parameter });
        }
        let (r0, r1) = (dot(j.du, d), dot(j.dv, d));
        Ok([(c * r0 - b * r1) / det, (a * r1 - b * r0) / det])
    }
}

fn line2(origin: [f64; 2], tangent: [f64; 2]) -> Option<CurveGeometry<ParameterSpace, 2>> {
    Some(CurveGeometry::Analytic(
        Curve::line(Point::new(origin).ok()?, Vector::new(tangent).ok()?).ok()?,
    ))
}
fn parallel(a: V3, b: V3, angle: f64) -> bool {
    norm(cross(a, b)) <= angle * norm(a) * norm(b)
}

/// Exact UV image for curve/surface pairs whose preimage is a line or planar conic.
/// `range` selects the branch of images that are only piecewise linear (meridians
/// change longitude at the poles).
fn exact(
    chart: &Chart,
    shape: &CurveShape,
    curve: &CurveGeometry<ModelSpace, 3>,
    range: [f64; 2],
    tolerance: f64,
) -> Option<CurveGeometry<ParameterSpace, 2>> {
    const ANGLE: f64 = 1e-9;
    let f = chart.frame()?;
    match (chart, shape) {
        // Great circles through the poles map to constant-u lines between the poles,
        // including arcs that end at a pole, where the fitted inverse is singular.
        (Chart::Sphere(_, r), CurveShape::Conic { frame, a, b })
            if (a - r).abs() <= tolerance
                && (b - r).abs() <= tolerance
                && distance(frame.o, f.o) <= tolerance
                && dot(frame.z, f.z).abs() <= ANGLE =>
        {
            let t = 0.5 * (range[0] + range[1]);
            let (sin, cos) = t.sin_cos();
            let d = add(scale(frame.x, cos), scale(frame.y, sin));
            let l = [dot(d, f.x), dot(d, f.y), dot(d, f.z)];
            let tangent = add(scale(frame.x, -sin), scale(frame.y, cos));
            let sigma = dot(tangent, f.z).signum();
            if dot(tangent, f.z).abs() <= ANGLE {
                return None;
            }
            let v = l[2].atan2(l[0].hypot(l[1]));
            line2([l[1].atan2(l[0]), v - sigma * t], [0., sigma])
        }
        (Chart::Plane(_), CurveShape::Line { origin, tangent }) => {
            let o = f.local(*origin);
            line2([o[0], o[1]], [dot(*tangent, f.x), dot(*tangent, f.y)])
        }
        (Chart::Plane(_), CurveShape::Conic { frame, a, b }) => {
            if !parallel(frame.z, f.z, ANGLE) {
                return None;
            }
            let o = f.local(frame.o);
            let x = [dot(frame.x, f.x), dot(frame.x, f.y)];
            let y = [dot(frame.y, f.x), dot(frame.y, f.y)];
            let frame2 = PlaneFrame::new(
                Point::new([o[0], o[1]]).ok()?,
                Vector::new(x).ok()?,
                Vector::new(y).ok()?,
                NumericalTolerance::default(),
            )
            .ok()?;
            Some(CurveGeometry::Analytic(
                Curve::ellipse(frame2, Length::metres(*a).ok()?, Length::metres(*b).ok()?).ok()?,
            ))
        }
        (Chart::Plane(_), CurveShape::Nurbs) => {
            let CurveGeometry::Nurbs(n) = curve else {
                return None;
            };
            let controls: Option<Vec<_>> = n
                .controls()
                .iter()
                .map(|p| {
                    let l = f.local(p.coordinates());
                    Point::new([l[0], l[1]]).ok()
                })
                .collect();
            Some(CurveGeometry::Nurbs(
                NurbsCurve::new(
                    n.knot_vector().degree(),
                    n.knot_vector().knots(),
                    &controls?,
                    n.weights(),
                    SplineLimits::default(),
                )
                .ok()?,
            ))
        }
        // Generators of cylinders and cones map to constant-u lines.
        (Chart::Cylinder(..), CurveShape::Line { origin, tangent }) => {
            if !parallel(*tangent, f.z, ANGLE) {
                return None;
            }
            let o = f.local(*origin);
            line2([o[1].atan2(o[0]), o[2]], [0., dot(*tangent, f.z)])
        }
        (Chart::Cone(_, alpha), CurveShape::Line { origin, tangent }) => {
            let o = f.local(*origin);
            let t = [dot(*tangent, f.x), dot(*tangent, f.y), dot(*tangent, f.z)];
            // The line must pass through the apex along a generator.
            if norm(cross(o, t)) > tolerance * norm(t) {
                return None;
            }
            let sign = if t[2] >= 0. { 1. } else { -1. };
            let u = (sign * t[1]).atan2(sign * t[0]);
            let g = [alpha.sin() * u.cos(), alpha.sin() * u.sin(), alpha.cos()];
            if !parallel(t, g, ANGLE) {
                return None;
            }
            line2([u, dot(o, g)], [0., sign * norm(t)])
        }
        // Coaxial circles map to constant-v lines.
        (
            Chart::Cylinder(..) | Chart::Cone(..) | Chart::Sphere(..) | Chart::Torus(..),
            CurveShape::Conic { frame, a, b },
        ) => {
            if a != b || !parallel(frame.z, f.z, ANGLE) {
                return None;
            }
            let c = f.local(frame.o);
            if c[0].hypot(c[1]) > tolerance {
                return None;
            }
            let sigma = dot(frame.z, f.z).signum();
            let phi = f.angle_of(frame.x);
            let v = match *chart {
                Chart::Cylinder(..) => c[2],
                Chart::Cone(_, alpha) => c[2] / alpha.cos(),
                Chart::Sphere(..) => c[2].atan2(*a),
                Chart::Torus(_, major, _) => c[2].atan2(a - major),
                _ => return None,
            };
            line2([phi, v], [sigma, 0.])
        }
        _ => None,
    }
}

/// An interior curve parameter at which the curve passes within the model tolerance
/// of a singular point of the chart (a pole or apex), where no continuous pcurve
/// exists. Each knot or quarter-period span is sampled and every local minimum of
/// the distance is refined by golden-section search.
fn singular_crossing(
    chart: &Chart,
    curve: &CurveGeometry<ModelSpace, 3>,
    range: [f64; 2],
    tolerance: f64,
    work: &mut Work,
) -> Result<Option<f64>, PcurveError> {
    let singular = chart.singular();
    if singular.is_empty() {
        return Ok(None);
    }
    let seeds = curve
        .break_parameters(range, 30_000)
        .map_err(|_| PcurveError::Limit)?;
    let margin = 1e-9 * (range[1] - range[0]);
    let mut at = |t: f64, p: V3| -> Result<f64, PcurveError> {
        Ok(distance(curve_jet(curve, t, KnotSide::Right, work)?.0, p))
    };
    const SAMPLES: usize = 16;
    for w in seeds.windows(2) {
        for s in &singular {
            let ts: Vec<f64> = (0..=SAMPLES)
                .map(|i| w[0] + (w[1] - w[0]) * i as f64 / SAMPLES as f64)
                .collect();
            let d = ts
                .iter()
                .map(|&t| at(t, s.point))
                .collect::<Result<Vec<_>, _>>()?;
            for i in 0..=SAMPLES {
                let lower = i == 0 || d[i] <= d[i - 1];
                let upper = i == SAMPLES || d[i] <= d[i + 1];
                if !(lower && upper) {
                    continue;
                }
                let (mut a, mut b) = (ts[i.saturating_sub(1)], ts[(i + 1).min(SAMPLES)]);
                const PHI: f64 = 0.618_033_988_749_894_9;
                for _ in 0..60 {
                    let (x, y) = (b - PHI * (b - a), a + PHI * (b - a));
                    if at(x, s.point)? <= at(y, s.point)? {
                        b = y;
                    } else {
                        a = x;
                    }
                }
                let t = 0.5 * (a + b);
                if at(t, s.point)? <= tolerance && t - range[0] > margin && range[1] - t > margin {
                    return Ok(Some(t));
                }
            }
        }
    }
    Ok(None)
}

/// Compute the pcurve of an edge (curve on `range`) on a face surface.
pub(crate) fn pcurve(
    inverter: &Inverter<'_>,
    shape: &CurveShape,
    curve: &CurveGeometry<ModelSpace, 3>,
    range: [f64; 2],
    work: &mut Work,
) -> Result<Pcurve, PcurveError> {
    let tolerance = inverter.tolerance;
    if let Some(parameter) = singular_crossing(inverter.chart, curve, range, tolerance, work)? {
        return Err(PcurveError::Singular { parameter });
    }
    if let Some(candidate) = exact(inverter.chart, shape, curve, range, tolerance) {
        let mut agrees = true;
        for s in [0., 0.25, 0.5, 0.75, 1.] {
            let t = range[0] + (range[1] - range[0]) * s;
            let uv = candidate
                .evaluate(t)
                .map_err(|_| PcurveError::Geometry)?
                .position
                .coordinates();
            let (p, _) = curve_jet(curve, t, KnotSide::Right, work)?;
            match surface_jet(inverter.surface, uv, work) {
                Ok(j) if distance(j.p, p) <= 0.5 * tolerance => {}
                _ => agrees = false,
            }
        }
        if agrees {
            return Ok(Pcurve {
                curve: candidate,
                range,
            });
        }
    }
    Ok(Pcurve {
        curve: CurveGeometry::Nurbs(fitted(inverter, curve, range, work)?),
        range,
    })
}

struct Segment {
    t: [f64; 2],
    uv: [[f64; 2]; 2],
    d: [[f64; 2]; 2],
}
/// Bezier control points of a Hermite segment, clamped to the closed parts of the
/// surface domain so that (by the convex hull property) the image stays inside it.
fn controls(s: &Segment, domain: [tessstep_surfaces::AxisDomain; 2]) -> [[f64; 2]; 4] {
    let h = s.t[1] - s.t[0];
    let mut c = [
        s.uv[0],
        [0, 1].map(|k| s.uv[0][k] + s.d[0][k] * h / 3.),
        [0, 1].map(|k| s.uv[1][k] - s.d[1][k] * h / 3.),
        s.uv[1],
    ];
    for p in &mut c {
        for (k, d) in domain.iter().enumerate() {
            p[k] = match *d {
                tessstep_surfaces::AxisDomain::Closed { min, max } => p[k].clamp(min, max),
                tessstep_surfaces::AxisDomain::LowerBounded { min } => p[k].max(min),
                _ => p[k],
            };
        }
    }
    c
}
fn bezier(c: &[[f64; 2]; 4], f: f64) -> [f64; 2] {
    let g = 1. - f;
    let b = [g * g * g, 3. * g * g * f, 3. * g * f * f, f * f * f];
    [0, 1].map(|k| (0..4).map(|i| b[i] * c[i][k]).sum())
}

/// Verified piecewise cubic Hermite image, returned as a C0 piecewise Bezier NURBS
/// whose parameter is the edge parameter. Curve knots/quarter periods seed spans.
fn fitted(
    inverter: &Inverter<'_>,
    curve: &CurveGeometry<ModelSpace, 3>,
    range: [f64; 2],
    work: &mut Work,
) -> Result<NurbsCurve<ParameterSpace, 2>, PcurveError> {
    const MAX_DEPTH: usize = 40;
    const MAX_SEGMENTS: usize = 30_000;
    let tolerance = inverter.tolerance;
    let domain = inverter.surface.domain();
    let seeds = curve
        .break_parameters(range, MAX_SEGMENTS)
        .map_err(|_| PcurveError::Limit)?;
    work.charge(seeds.len())?;
    let singular = inverter.chart.singular();
    let point = |t: f64,
                 side: KnotSide,
                 hint: Option<[f64; 2]>,
                 work: &mut Work|
     -> Result<([f64; 2], [f64; 2]), PcurveError> {
        let (p, d) = curve_jet(curve, t, side, work)?;
        // A curve end at a pole or apex takes its one-sided limit: the coordinate
        // along the singular line from just inside the curve, the other on the line.
        if t == range[0] || t == range[1] {
            if let Some(s) = singular.iter().find(|s| distance(p, s.point) <= tolerance) {
                let h = 1e-7 * (range[1] - range[0]) * if t == range[0] { 1. } else { -1. };
                let (q, _) = curve_jet(curve, t + h, side, work)?;
                let inside = inverter.invert(q, hint, work)?;
                let mut uv = inside;
                uv[1 - s.k] = s.value;
                return Ok((uv, [0, 1].map(|k| (inside[k] - uv[k]) / h)));
            }
        }
        let uv = inverter.invert(p, hint, work)?;
        let lifted = surface_jet(inverter.surface, uv, work)?.p;
        let off = distance(lifted, p);
        if off > tolerance {
            return Err(PcurveError::OffSurface {
                parameter: t,
                distance: off,
            });
        }
        Ok((uv, inverter.derivative(uv, d, t, work)?))
    };
    let mut segments = Vec::new();
    // Resolve the chart of the first point from a nearby interior point, so a start
    // on a closed seam takes the representative the curve continues from.
    let inner = seeds[0] + (seeds[1] - seeds[0]) * 1e-3;
    let (near, _) = point(inner, KnotSide::Right, None, work)?;
    let (mut previous, _) = point(seeds[0], KnotSide::Right, Some(near), work)?;
    for w in seeds.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (_, da) = point(a, KnotSide::Right, Some(previous), work)?;
        let guess = [0, 1].map(|k| previous[k] + da[k] * (b - a));
        let (uvb, db) = point(b, KnotSide::Left, Some(guess), work)?;
        let mut stack = vec![(
            Segment {
                t: [a, b],
                uv: [previous, uvb],
                d: [da, db],
            },
            0,
        )];
        while let Some((segment, depth)) = stack.pop() {
            let mut good = true;
            let c = controls(&segment, domain);
            for f in [0.25, 0.5, 0.75] {
                let t = segment.t[0] + (segment.t[1] - segment.t[0]) * f;
                let h = bezier(&c, f);
                let (p, _) = curve_jet(curve, t, KnotSide::Right, work)?;
                let uv = inverter.invert(p, Some(h), work)?;
                let exact = surface_jet(inverter.surface, uv, work)?.p;
                let off = distance(exact, p);
                if off > tolerance {
                    return Err(PcurveError::OffSurface {
                        parameter: t,
                        distance: off,
                    });
                }
                let approximated = match surface_jet(inverter.surface, h, work) {
                    Ok(j) => j.p,
                    Err(PcurveError::Limit) => return Err(PcurveError::Limit),
                    Err(_) => {
                        good = false;
                        break;
                    }
                };
                if distance(approximated, exact) > 0.25 * tolerance {
                    good = false;
                    break;
                }
            }
            if good {
                if segments.len() >= MAX_SEGMENTS {
                    return Err(PcurveError::Limit);
                }
                segments.push(segment);
                continue;
            }
            if depth >= MAX_DEPTH {
                return Err(PcurveError::Unresolved);
            }
            let m = segment.t[0] + (segment.t[1] - segment.t[0]) * 0.5;
            if m <= segment.t[0] || m >= segment.t[1] {
                return Err(PcurveError::Unresolved);
            }
            let (uvm, dm) = point(m, KnotSide::Right, Some(bezier(&c, 0.5)), work)?;
            let [t0, t1] = segment.t;
            stack.push((
                Segment {
                    t: [m, t1],
                    uv: [uvm, segment.uv[1]],
                    d: [dm, segment.d[1]],
                },
                depth + 1,
            ));
            stack.push((
                Segment {
                    t: [t0, m],
                    uv: [segment.uv[0], uvm],
                    d: [segment.d[0], dm],
                },
                depth + 1,
            ));
        }
        previous = uvb;
    }
    let mut knots = vec![segments[0].t[0]; 4];
    let mut points = Vec::with_capacity(3 * segments.len() + 1);
    for (i, s) in segments.iter().enumerate() {
        let c = controls(s, domain);
        points.extend_from_slice(&c[..3]);
        let repeat = if i + 1 == segments.len() { 4 } else { 3 };
        knots.extend(std::iter::repeat_n(s.t[1], repeat));
    }
    points.push(controls(segments.last().expect("nonempty seeds"), domain)[3]);
    work.charge(points.len())?;
    let points: Result<Vec<Point<ParameterSpace, 2>>, _> =
        points.into_iter().map(Point::new).collect();
    let points = points.map_err(|_| PcurveError::Geometry)?;
    let weights = vec![1.; points.len()];
    NurbsCurve::new(
        3,
        &knots,
        &points,
        &weights,
        SplineLimits {
            max_controls: 3 * MAX_SEGMENTS + 1,
            max_knots: 3 * MAX_SEGMENTS + 5,
        },
    )
    .map_err(|_| PcurveError::Geometry)
}
