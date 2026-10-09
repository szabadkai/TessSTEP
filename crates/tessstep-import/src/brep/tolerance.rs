//! Tolerance policy for edges off their face surfaces and vertices off their edge
//! curves (see docs/CURVED_IMPORT.md#tolerance-policy).
//!
//! A distance within the model tolerance is consistent geometry. A larger one, up to
//! `ImportOptions::deviation_factor` times the model tolerance, is accepted and
//! counted; anything farther is rejected with the measured distance and the bound.
//! No coordinate is moved: an edge keeps its 3D curve (the mesh boundary follows it)
//! and receives the pcurve of its surface projection, and a vertex keeps its position
//! while its edge is bounded at the vertex's projection onto the curve. Every later
//! stage then uses the working tolerance, which covers a vertex and an edge end that
//! each deviate by the largest accepted distance.
use super::*;

/// The accepted deviation bound of one import.
#[derive(Clone, Copy, Debug)]
pub(super) struct Policy {
    /// Model tolerance in metres.
    pub tolerance: f64,
    /// Accepted multiple of the model tolerance; 1 accepts no deviation.
    pub factor: f64,
}
impl Policy {
    /// The policy of `options`: strict import accepts no deviation. A factor below 1,
    /// or not finite, is invalid.
    pub(super) fn new(tolerance: f64, options: ImportOptions) -> Result<Self, String> {
        let factor = options.deviation_factor;
        if !(factor.is_finite() && factor >= 1.) {
            return Err(format!(
                "deviation_factor must be a finite multiple of at least 1 (got {factor})"
            ));
        }
        Ok(Self {
            tolerance,
            factor: if options.strict { 1. } else { factor },
        })
    }
    /// The largest accepted distance in metres.
    pub(super) fn allowance(&self) -> f64 {
        self.tolerance * self.factor
    }
    /// Whether `distance` is accepted.
    pub(super) fn accepts(&self, distance: f64) -> bool {
        distance <= self.allowance()
    }
    /// Validation, trimming and tessellation tolerance after accepting deviations of
    /// at most `largest` metres: unchanged without deviations; otherwise the model
    /// tolerance plus twice the deviation, since a vertex and the edge end beside it
    /// may each be that far off, on either side of the surface.
    pub(super) fn working(&self, largest: f64) -> f64 {
        if largest > self.tolerance {
            self.tolerance + 2. * largest
        } else {
            self.tolerance
        }
    }
    /// The bound a rejected distance exceeds, for diagnostics.
    pub(super) fn bound(&self) -> String {
        if self.factor == 1. {
            format!("the model tolerance {:.3e} m", self.tolerance)
        } else {
            format!(
                "the accepted deviation {:.3e} m ({} × the model tolerance {:.3e} m)",
                self.allowance(),
                self.factor,
                self.tolerance
            )
        }
    }
}

/// Where an edge's interior meets a singular point (pole, apex or collapsed B-spline
/// side) of a face chart.
#[derive(Clone, Copy, Debug)]
pub(super) struct Crossing {
    /// The singular line.
    pub singular: Singular,
    /// Distance in metres of the curve point from the singular point.
    pub distance: f64,
    /// Largest distance in metres of the singular line's surface points from its
    /// point: zero for analytic poles and apices, the side's extent for a B-spline
    /// side that collapses only within the model tolerance.
    pub extent: f64,
}

/// The singular point of `chart` nearest to the edge point at `parameter`, with the
/// singular line's sampled extent on `surface`.
pub(super) fn crossing(
    chart: &Chart,
    surface: &SurfaceGeometry,
    curve: &CurveGeometry<ModelSpace, 3>,
    parameter: f64,
) -> Option<Crossing> {
    let p = curve.evaluate(parameter).ok()?.position.coordinates();
    let singular = chart
        .singular()
        .into_iter()
        .min_by(|a, b| norm(sub(a.point, p)).total_cmp(&norm(sub(b.point, p))))?;
    let domain = surface.domain();
    let along = match domain[singular.k] {
        tessstep_surfaces::AxisDomain::Closed { min, max } => Some([min, max]),
        tessstep_surfaces::AxisDomain::Periodic { period } => Some([0., period]),
        _ => None,
    };
    let mut extent = 0.0_f64;
    if let Some([lo, hi]) = along {
        for i in 0..=16 {
            let mut uv = [0.; 2];
            uv[singular.k] = lo + (hi - lo) * f64::from(i) / 16.;
            uv[1 - singular.k] = singular.value;
            if let Ok(jet) = surface.evaluate(uv) {
                extent = extent.max(norm(sub(jet.position.coordinates(), singular.point)));
            }
        }
    }
    Some(Crossing {
        singular,
        distance: norm(sub(p, singular.point)),
        extent,
    })
}

impl Crossing {
    /// A B-spline side that collapses only within the model tolerance: its points
    /// spread by more than the trimmer's touch threshold, a thousandth of it.
    pub(super) fn near_collapsed(&self, tolerance: f64) -> bool {
        self.extent > 1e-3 * tolerance
    }
}

impl Builder {
    /// `face` with every B-spline side that an edge interior runs along and that
    /// collapses only within the model tolerance charted as a regular side, or None
    /// when no side qualifies. Such a side is an edge shorter than the model
    /// tolerance, not a pole: the exporter bounds it with an edge, not a vertex.
    pub(super) fn uncollapse(
        &mut self,
        c: &Context<'_, '_>,
        face: &FaceRecord,
        tolerance: f64,
    ) -> Result<Option<FaceRecord>, Error> {
        let Chart::Nurbs(periods, singular) = &face.chart else {
            return Ok(None);
        };
        let mut kept = singular.clone();
        let mut edges: Vec<usize> = Vec::new();
        for u in face.loops.iter().flat_map(|l| &l.uses) {
            if !edges.contains(&u.edge) {
                edges.push(u.edge);
            }
        }
        for edge in edges {
            if kept.is_empty() {
                break;
            }
            let chart = Chart::Nurbs(*periods, kept.clone());
            let e = &self.edges[edge];
            let found =
                pcurve::singular_crossing(&chart, &e.curve, e.range, tolerance, &mut self.work)
                    .map_err(|err| self.pcurve_error(c, edge, err))?;
            let Some(parameter) = found else {
                continue;
            };
            let e = &self.edges[edge];
            let Some(x) = crossing(&chart, &face.surface, &e.curve, parameter) else {
                continue;
            };
            if x.distance <= tolerance && x.near_collapsed(tolerance) {
                kept.retain(|s| *s != x.singular);
                self.adaptations.uncollapsed_sides += 1;
            }
        }
        if kept.len() == singular.len() {
            return Ok(None);
        }
        let mut regular = face.clone();
        regular.chart = Chart::Nurbs(*periods, kept);
        Ok(Some(regular))
    }
}
