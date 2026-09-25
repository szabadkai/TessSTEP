//! STEP-independent, checked floating-point coordinates and affine transforms.
//!
//! Model/local coordinates use metres; parameter coordinates use the evaluator's
//! parameter units. Space tags prevent accidental mixing, not dimensional analysis
//! of every scalar operation. All storage is private and finite. Arithmetic is
//! fallible, uses ordinary `f64` rounding, and is not an exact predicate kernel.
//!
//! ```
//! use tessstep_math::{Affine3, LocalSpace, ModelSpace, Point3, Vector3};
//! let t = Affine3::<LocalSpace, ModelSpace>::new(
//!     [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]], [10., 0., 0.])?;
//! let p = Point3::<LocalSpace>::new([1., 2., 3.])?;
//! assert_eq!(t.transform_point(p)?.coordinates(), [11., 2., 3.]);
//! assert_eq!(t.transform_vector(Vector3::new([1., 2., 3.])?)?.components(), [1., 2., 3.]);
//! # Ok::<(), tessstep_math::Error>(())
//! ```
//!
//! Coordinates in different spaces cannot be combined:
//! ```compile_fail
//! use tessstep_math::{LocalSpace, ModelSpace, Point3, Vector3};
//! let p = Point3::<ModelSpace>::new([0.; 3]).unwrap();
//! let v = Vector3::<LocalSpace>::new([1.; 3]).unwrap();
//! p.translated(v).unwrap();
//! ```
//! Transform composition requires matching intermediate spaces:
//! ```compile_fail
//! use tessstep_math::{Affine3, LocalSpace, ModelSpace};
//! let a = Affine3::<LocalSpace, ModelSpace>::new(
//!     [[1.,0.,0.], [0.,1.,0.], [0.,0.,1.]], [0.;3]).unwrap();
//! a.then(a).unwrap();
//! ```
#![forbid(unsafe_code)]

mod coordinates;
mod transform;
mod units;

pub use coordinates::*;
pub use transform::*;
pub use units::*;

/// Checked construction/arithmetic failures. No non-finite value is published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    NonFinite,
    OutOfRange,
    ZeroDirection,
    InvalidTolerance,
    InvalidDimension,
    SingularTransform,
    ParallelAxes,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "non-finite input or arithmetic result",
            Self::OutOfRange => "value outside the representable or permitted range",
            Self::ZeroDirection => "zero vector has no direction",
            Self::InvalidTolerance => "invalid tolerance",
            Self::InvalidDimension => "coordinate dimension must be positive",
            Self::SingularTransform => "singular transform or pivot below numerical tolerance",
            Self::ParallelAxes => "frame axes parallel within numerical tolerance",
        })
    }
}
impl std::error::Error for Error {}

pub(crate) fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::NonFinite)
    }
}
pub(crate) fn positive(value: f64) -> Result<f64, Error> {
    finite(value)?;
    if value > 0. {
        Ok(value)
    } else {
        Err(Error::OutOfRange)
    }
}
