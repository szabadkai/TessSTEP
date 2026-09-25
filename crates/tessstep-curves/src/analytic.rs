use crate::{Error, PlaneFrame, dimension, finite};
use tessstep_math::{Affine3, Length, Point, Space, Vector};

/// Domain of the analytic basis. Unbounded means all finite parameters may be
/// requested, not that every evaluation is representable. Periodic bases also
/// accept every finite parameter; the reported fundamental interval is [0, period).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Domain {
    Unbounded,
    Periodic { period: f64 },
}

/// Position, first derivative and second derivative with respect to the supplied
/// parameter. Derivatives are vectors, not normalized tangents or normals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evaluation<S: Space, const N: usize> {
    pub position: Point<S, N>,
    pub first: Vector<S, N>,
    pub second: Vector<S, N>,
}
impl<S: Space> Evaluation<S, 3> {
    /// Affine covariance: position includes translation; both derivatives do not.
    /// Singular maps are permitted and may collapse derivatives to zero.
    pub fn transformed<T: Space>(
        self,
        transform: Affine3<S, T>,
    ) -> Result<Evaluation<T, 3>, Error> {
        Ok(Evaluation {
            position: transform.transform_point(self.position)?,
            first: transform.transform_vector(self.first)?,
            second: transform.transform_vector(self.second)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveKind {
    Line,
    Circle,
    Ellipse,
    Parabola,
    Hyperbola,
}

/// Validated immutable analytic basis. No raw STEP entities or schema semantics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve<S: Space, const N: usize>(Basis<S, N>);
pub type Curve2<S> = Curve<S, 2>;
pub type Curve3<S> = Curve<S, 3>;
#[derive(Clone, Copy, Debug, PartialEq)]
enum ConicKind {
    Circle,
    Ellipse,
    Parabola,
    Hyperbola,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Basis<S: Space, const N: usize> {
    Line {
        origin: Point<S, N>,
        tangent: Vector<S, N>,
    },
    Conic {
        frame: PlaneFrame<S, N>,
        kind: ConicKind,
        a: f64,
        b: f64,
    },
}
impl<S: Space, const N: usize> Curve<S, N> {
    /// P(u) = origin + u*tangent. The supplied tangent magnitude is preserved.
    pub fn line(origin: Point<S, N>, tangent: Vector<S, N>) -> Result<Self, Error> {
        dimension::<N>()?;
        if tangent.components().iter().all(|&v| v == 0.) {
            return Err(Error::DegenerateLine);
        }
        Ok(Self(Basis::Line { origin, tangent }))
    }
    /// P(u) = origin + r*cos(u)*x + r*sin(u)*y; u in radians.
    pub fn circle(frame: PlaneFrame<S, N>, radius: Length) -> Result<Self, Error> {
        Self::conic(frame, ConicKind::Circle, radius, radius)
    }
    /// Semi-axis lengths along x/y. No axis reordering or parameter remapping.
    pub fn ellipse(
        frame: PlaneFrame<S, N>,
        x_radius: Length,
        y_radius: Length,
    ) -> Result<Self, Error> {
        Self::conic(frame, ConicKind::Ellipse, x_radius, y_radius)
    }
    /// P(u) = origin + f*u^2*x + 2*f*u*y; focus at origin + f*x.
    pub fn parabola(frame: PlaneFrame<S, N>, focal_length: Length) -> Result<Self, Error> {
        Self::conic(frame, ConicKind::Parabola, focal_length, focal_length)
    }
    /// Positive-x branch: P(u) = origin + a*cosh(u)*x + b*sinh(u)*y.
    /// Reverse the frame x axis (and choose y) to construct the other branch.
    pub fn hyperbola(
        frame: PlaneFrame<S, N>,
        x_radius: Length,
        y_radius: Length,
    ) -> Result<Self, Error> {
        Self::conic(frame, ConicKind::Hyperbola, x_radius, y_radius)
    }
    fn conic(
        frame: PlaneFrame<S, N>,
        kind: ConicKind,
        a: Length,
        b: Length,
    ) -> Result<Self, Error> {
        let (a, b) = (a.as_metres(), b.as_metres());
        if a <= 0. || b <= 0. {
            return Err(Error::NonPositiveRadius);
        }
        Ok(Self(Basis::Conic { frame, kind, a, b }))
    }
    pub fn kind(self) -> CurveKind {
        match self.0 {
            Basis::Line { .. } => CurveKind::Line,
            Basis::Conic { kind, .. } => match kind {
                ConicKind::Circle => CurveKind::Circle,
                ConicKind::Ellipse => CurveKind::Ellipse,
                ConicKind::Parabola => CurveKind::Parabola,
                ConicKind::Hyperbola => CurveKind::Hyperbola,
            },
        }
    }
    pub fn domain(self) -> Domain {
        match self.kind() {
            CurveKind::Circle | CurveKind::Ellipse => Domain::Periodic {
                period: std::f64::consts::TAU,
            },
            _ => Domain::Unbounded,
        }
    }
    /// Evaluate all three outputs atomically. Non-finite parameters or any
    /// unrepresentable intermediate/output fail; no clamping or silent wrapping.
    pub fn evaluate(self, parameter: f64) -> Result<Evaluation<S, N>, Error> {
        let u = finite(parameter)?;
        match self.0 {
            Basis::Line { origin, tangent } => Ok(Evaluation {
                position: origin.translated(tangent.scaled(u)?)?,
                first: tangent,
                second: Vector::new([0.; N])?,
            }),
            Basis::Conic { frame, kind, a, b } => {
                let (p, d, dd) = match kind {
                    ConicKind::Circle | ConicKind::Ellipse => {
                        let (s, c) = u.sin_cos();
                        ([a * c, b * s], [-a * s, b * c], [-a * c, -b * s])
                    }
                    ConicKind::Parabola => {
                        // Multiply by the focal scale before u to avoid u*u overflow
                        // when the scaled parabola remains representable.
                        let au = finite(a * u)?;
                        (
                            [finite(au * u)?, finite(2. * au)?],
                            [finite(2. * au)?, finite(2. * a)?],
                            [finite(2. * a)?, 0.],
                        )
                    }
                    ConicKind::Hyperbola => {
                        let (s, c) = (finite(u.sinh())?, finite(u.cosh())?);
                        (
                            [finite(a * c)?, finite(b * s)?],
                            [finite(a * s)?, finite(b * c)?],
                            [finite(a * c)?, finite(b * s)?],
                        )
                    }
                };
                Ok(Evaluation {
                    position: frame.point(p[0], p[1])?,
                    first: frame.vector(d[0], d[1])?,
                    second: frame.vector(dd[0], dd[1])?,
                })
            }
        }
    }
}
