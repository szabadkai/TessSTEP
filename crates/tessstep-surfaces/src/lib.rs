//! Independent analytic and tensor-product surface evaluation in metres.
//! No STEP interpretation, topology, trimming or tessellation is performed.
//!
//! ```
//! use tessstep_surfaces::NurbsSurface;
//! use tessstep_math::{ModelSpace, Point3};
//! let points = [[0.,0.,0.],[0.,1.,0.],[1.,0.,0.],[1.,1.,0.]]
//!     .map(|p| Point3::<ModelSpace>::new(p).unwrap());
//! let surface = NurbsSurface::new([1,1], [&[0.,0.,1.,1.];2], [2,2],
//!     &points, &[1.;4], Default::default())?;
//! assert_eq!(surface.evaluate(0.5,0.5)?.position.coordinates(), [0.5,0.5,0.]);
//! # Ok::<(), tessstep_surfaces::Error>(())
//! ```
#![forbid(unsafe_code)]
mod analytic;
mod nurbs;
pub use analytic::*;
pub use nurbs::*;
use tessstep_math::{Affine3, Direction3, NumericalTolerance, Point3, Space, Vector3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    Spline(tessstep_curves::spline::SplineError),
    Math(tessstep_math::Error),
    InvalidShape,
    ParameterOutsideDomain,
    SingularNormal,
}
impl From<tessstep_math::Error> for Error {
    fn from(e: tessstep_math::Error) -> Self {
        Self::Math(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spline(e) => write!(f, "surface spline: {e}"),
            Self::Math(e) => write!(f, "surface arithmetic: {e}"),
            Self::InvalidShape => f.write_str("invalid surface radii or cone angle"),
            Self::ParameterOutsideDomain => f.write_str("parameter outside surface domain"),
            Self::SingularNormal => f.write_str("surface partials do not define a normal"),
        }
    }
}
impl std::error::Error for Error {}
pub(crate) fn finite(x: f64) -> Result<f64, Error> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(tessstep_math::Error::NonFinite.into())
    }
}
/// All partial derivatives are with respect to the supplied u,v coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evaluation<S: Space> {
    pub position: Point3<S>,
    pub du: Vector3<S>,
    pub dv: Vector3<S>,
    pub duu: Vector3<S>,
    pub duv: Vector3<S>,
    pub dvv: Vector3<S>,
}
impl<S: Space> Evaluation<S> {
    /// Oriented normal from du cross dv; singularities are explicit failures.
    pub fn normal(self, tolerance: NumericalTolerance) -> Result<Direction3<S>, Error> {
        let a = self.du.normalized().map_err(|_| Error::SingularNormal)?;
        let b = self.dv.normalized().map_err(|_| Error::SingularNormal)?;
        let cross = a.vector().cross(b.vector())?;
        if cross.norm()? <= tolerance.relative() {
            return Err(Error::SingularNormal);
        }
        Ok(cross.normalized()?)
    }
    pub fn transformed<T: Space>(self, t: Affine3<S, T>) -> Result<Evaluation<T>, Error> {
        Ok(Evaluation {
            position: t.transform_point(self.position)?,
            du: t.transform_vector(self.du)?,
            dv: t.transform_vector(self.dv)?,
            duu: t.transform_vector(self.duu)?,
            duv: t.transform_vector(self.duv)?,
            dvv: t.transform_vector(self.dvv)?,
        })
    }
}

impl From<tessstep_curves::spline::SplineError> for Error {
    fn from(e: tessstep_curves::spline::SplineError) -> Self {
        Self::Spline(e)
    }
}
