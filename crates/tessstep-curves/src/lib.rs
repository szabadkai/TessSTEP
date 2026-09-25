//! STEP-independent analytic curves with checked positions and two derivatives.
//!
//! Coordinates and radii use metres; space tags describe coordinate frames.
//! Parameters are dimensionless; circle/ellipse parameters are radians. Evaluation
//! is floating-point analytic evaluation, not exact arithmetic or tessellation.
//!
//! ```
//! use tessstep_curves::{Curve3, PlaneFrame};
//! use tessstep_math::{Length, ModelSpace, Point3, Vector3};
//! let frame = PlaneFrame::new(Point3::<ModelSpace>::new([0.;3])?,
//!     Vector3::new([1.,0.,0.])?, Vector3::new([0.,1.,0.])?, Default::default())?;
//! let circle = Curve3::circle(frame, Length::metres(2.)?)?;
//! let sample = circle.evaluate(0.)?;
//! assert_eq!(sample.position.coordinates(), [2.,0.,0.]);
//! assert_eq!(sample.first.components(), [0.,2.,0.]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! A frame cannot mix coordinate spaces:
//! ```compile_fail
//! use tessstep_curves::PlaneFrame;
//! use tessstep_math::{LocalSpace, ModelSpace, Point3, Vector3};
//! PlaneFrame::new(Point3::<ModelSpace>::new([0.;3]).unwrap(),
//!     Vector3::<LocalSpace>::new([1.,0.,0.]).unwrap(),
//!     Vector3::new([0.,1.,0.]).unwrap(), Default::default()).unwrap();
//! ```
#![forbid(unsafe_code)]

mod analytic;
mod frame;
mod span;
pub use analytic::*;
pub use frame::*;
pub use span::*;

/// Construction or evaluation failure; partial evaluations are never returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    Math(tessstep_math::Error),
    UnsupportedDimension,
    NonPositiveRadius,
    DegenerateLine,
    InvalidInterval,
    ParameterOutsideSpan,
}
impl From<tessstep_math::Error> for Error {
    fn from(value: tessstep_math::Error) -> Self {
        Self::Math(value)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Math(e) => write!(f, "curve arithmetic: {e}"),
            Self::UnsupportedDimension => f.write_str("analytic curves require dimension 2 or 3"),
            Self::NonPositiveRadius => {
                f.write_str("radius, semi-axis or focal length must be positive")
            }
            Self::DegenerateLine => f.write_str("line tangent must be nonzero"),
            Self::InvalidInterval => {
                f.write_str("span endpoints must be distinct with a finite nonzero difference")
            }
            Self::ParameterOutsideSpan => f.write_str("span parameter must be in [0, 1]"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Math(e) => Some(e),
            _ => None,
        }
    }
}
pub(crate) fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(tessstep_math::Error::NonFinite.into())
    }
}
pub(crate) fn dimension<const N: usize>() -> Result<(), Error> {
    if N == 2 || N == 3 {
        Ok(())
    } else {
        Err(Error::UnsupportedDimension)
    }
}
