use tessstep_curves::{
    Curve, Evaluation,
    spline::{KnotSide, NurbsCurve},
};
use tessstep_math::{ModelSpace, Space};
use tessstep_surfaces::{AxisDomain, NurbsSurface, Surface};

/// Constructed geometry only. No schema or physical entity references.
#[derive(Clone, Debug, PartialEq)]
pub enum CurveGeometry<S: Space, const N: usize> {
    Analytic(Curve<S, N>),
    Nurbs(NurbsCurve<S, N>),
}
#[derive(Clone, Debug, PartialEq)]
pub enum GeometryError {
    Curve(tessstep_curves::Error),
    Spline(tessstep_curves::spline::SplineError),
    Surface(tessstep_surfaces::Error),
}
impl std::fmt::Display for GeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "geometry evaluation failed: {self:?}")
    }
}
impl std::error::Error for GeometryError {}
impl<S: Space, const N: usize> CurveGeometry<S, N> {
    pub fn evaluate(&self, u: f64) -> Result<Evaluation<S, N>, GeometryError> {
        self.evaluate_on_side(u, KnotSide::Right)
    }
    pub fn evaluate_on_side(
        &self,
        u: f64,
        side: KnotSide,
    ) -> Result<Evaluation<S, N>, GeometryError> {
        match self {
            Self::Analytic(c) => c.evaluate(u).map_err(GeometryError::Curve),
            Self::Nurbs(c) => c.evaluate_on_side(u, side).map_err(GeometryError::Spline),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceGeometry {
    Analytic(Surface<ModelSpace>),
    Nurbs(NurbsSurface<ModelSpace>),
}
impl SurfaceGeometry {
    pub fn evaluate(
        &self,
        uv: [f64; 2],
    ) -> Result<tessstep_surfaces::Evaluation<ModelSpace>, GeometryError> {
        match self {
            Self::Analytic(s) => s.evaluate(uv[0], uv[1]),
            Self::Nurbs(s) => s.evaluate(uv[0], uv[1]),
        }
        .map_err(GeometryError::Surface)
    }
    pub fn domain(&self) -> [AxisDomain; 2] {
        match self {
            Self::Analytic(s) => s.domain(),
            Self::Nurbs(s) => s.knot_vectors().map(|k| {
                let [min, max] = k.domain();
                AxisDomain::Closed { min, max }
            }),
        }
    }
}
impl<S: Space, const N: usize> CurveGeometry<S, N> {
    /// Increasing interval endpoints plus interior knot breaks or quarter-period seeds.
    /// This prevents periodic aliasing and avoids stepping across spline breaks.
    pub fn break_parameters(
        &self,
        range: [f64; 2],
        limit: usize,
    ) -> Result<Vec<f64>, GeometryError> {
        use tessstep_curves::{Domain, Error};
        let limit = limit.min(1_000_000);
        let [a, b] = range;
        if !a.is_finite() || !b.is_finite() || a >= b || !(b - a).is_finite() {
            return Err(GeometryError::Curve(Error::InvalidInterval));
        }
        let budget_error =
            || GeometryError::Spline(tessstep_curves::spline::SplineError::ResourceLimit);
        if limit < 2 {
            return Err(budget_error());
        }
        let mut out = vec![a];
        match self {
            Self::Analytic(c) => {
                if let Domain::Periodic { period } = c.domain() {
                    let n = ((b - a) / (period / 4.)).ceil().max(1.);
                    if n >= limit as f64 {
                        return Err(budget_error());
                    }
                    for i in 1..n as usize {
                        out.push(a + (b - a) * (i as f64 / n));
                    }
                }
            }
            Self::Nurbs(c) => {
                for &u in c.knot_vector().knots() {
                    if u > a && u < b && out.last() != Some(&u) {
                        if out.len() >= limit - 1 {
                            return Err(budget_error());
                        }
                        out.push(u);
                    }
                }
            }
        }
        out.push(b);
        Ok(out)
    }
}
