//! B-spline basis recurrence and Boehm insertion in homogeneous coordinates.
//! Algorithm sources and limits: docs/NURBS.md in the repository.
//!
//! ```
//! use tessstep_curves::spline::NurbsCurve;
//! use tessstep_math::{ModelSpace, Point2};
//! let points = [Point2::<ModelSpace>::new([0.,0.])?, Point2::new([2.,4.])?];
//! let curve = NurbsCurve::new(1, &[0.,0.,1.,1.], &points, &[1.,1.], Default::default())?;
//! assert_eq!(curve.evaluate(0.5)?.position.coordinates(), [1.,2.]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
mod curve;
mod knots;
pub use curve::*;
pub use knots::*;

pub const MAX_DEGREE: usize = 16;
pub const HARD_MAX_CONTROLS: usize = 1_000_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplineLimits {
    pub max_controls: usize,
    pub max_knots: usize,
}
impl Default for SplineLimits {
    fn default() -> Self {
        Self {
            max_controls: 100_000,
            max_knots: 100_034,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SplineError {
    InvalidDegree,
    InvalidKnots,
    InvalidControls,
    InvalidWeights,
    OutsideDomain,
    ResourceLimit,
    NumericRange,
    InsertionLimit,
}
impl std::fmt::Display for SplineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidDegree => "spline degree must be in 1..=16",
            Self::InvalidKnots => "invalid knot count, order, multiplicity or active domain",
            Self::InvalidControls => "invalid control count, dimension or net layout",
            Self::InvalidWeights => {
                "weights must be positive, finite and retain nonzero relative scale"
            }
            Self::OutsideDomain => "parameter outside active knot domain",
            Self::ResourceLimit => "spline resource limit",
            Self::NumericRange => "unrepresentable spline arithmetic",
            Self::InsertionLimit => {
                "knot insertion requires an interior knot with multiplicity below degree"
            }
        })
    }
}
impl std::error::Error for SplineError {}
impl From<tessstep_math::Error> for SplineError {
    fn from(_: tessstep_math::Error) -> Self {
        Self::NumericRange
    }
}
pub(crate) fn checked(x: f64) -> Result<f64, SplineError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(SplineError::NumericRange)
    }
}
/// Validate positive homogeneous scales before allocating output storage.
pub fn validate_weights(weights: &[f64]) -> Result<f64, SplineError> {
    let mut max = 0_f64;
    for &w in weights {
        if !w.is_finite() || w <= 0. {
            return Err(SplineError::InvalidWeights);
        }
        max = max.max(w);
    }
    if max == 0. || weights.iter().any(|w| w / max == 0.) {
        return Err(SplineError::InvalidWeights);
    }
    Ok(max)
}
