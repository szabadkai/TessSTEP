use crate::{Error, Evaluation, finite};
use std::f64::consts::{FRAC_PI_2, TAU};
use tessstep_curves::PlaneFrame;
use tessstep_math::{Angle, Length, Space, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AxisDomain {
    Unbounded,
    Periodic { period: f64 },
    Closed { min: f64, max: f64 },
    LowerBounded { min: f64 },
}
impl AxisDomain {
    pub fn contains(self, value: f64) -> bool {
        value.is_finite()
            && match self {
                Self::Closed { min, max } => value >= min && value <= max,
                Self::LowerBounded { min } => value >= min,
                _ => true,
            }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceKind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Plane,
    Cylinder(f64),
    Cone(f64),
    Sphere(f64),
    Torus(f64, f64),
}
/// Oriented elementary surfaces. Periodic parameters are unwrapped radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface<S: Space> {
    frame: PlaneFrame<S, 3>,
    shape: Shape,
}
impl<S: Space> Surface<S> {
    pub fn plane(frame: PlaneFrame<S, 3>) -> Self {
        Self {
            frame,
            shape: Shape::Plane,
        }
    }
    pub fn cylinder(frame: PlaneFrame<S, 3>, radius: Length) -> Result<Self, Error> {
        Ok(Self {
            frame,
            shape: Shape::Cylinder(positive(radius)?),
        })
    }
    /// Cone vertex at frame origin; v is nonnegative slant distance in metres.
    pub fn cone(frame: PlaneFrame<S, 3>, semi_angle: Angle) -> Result<Self, Error> {
        let angle = semi_angle.as_radians();
        if angle <= 0. || angle >= FRAC_PI_2 {
            return Err(Error::InvalidShape);
        }
        Ok(Self {
            frame,
            shape: Shape::Cone(angle),
        })
    }
    /// u is azimuth, v is latitude in the closed interval from -pi/2 to pi/2.
    pub fn sphere(frame: PlaneFrame<S, 3>, radius: Length) -> Result<Self, Error> {
        Ok(Self {
            frame,
            shape: Shape::Sphere(positive(radius)?),
        })
    }
    /// Ring torus only: major radius must exceed the positive minor radius.
    pub fn torus(frame: PlaneFrame<S, 3>, major: Length, minor: Length) -> Result<Self, Error> {
        let (a, b) = (positive(major)?, positive(minor)?);
        if a <= b {
            return Err(Error::InvalidShape);
        }
        Ok(Self {
            frame,
            shape: Shape::Torus(a, b),
        })
    }
    pub fn kind(self) -> SurfaceKind {
        match self.shape {
            Shape::Plane => SurfaceKind::Plane,
            Shape::Cylinder(_) => SurfaceKind::Cylinder,
            Shape::Cone(_) => SurfaceKind::Cone,
            Shape::Sphere(_) => SurfaceKind::Sphere,
            Shape::Torus(..) => SurfaceKind::Torus,
        }
    }
    pub fn domain(self) -> [AxisDomain; 2] {
        let periodic = AxisDomain::Periodic { period: TAU };
        match self.shape {
            Shape::Plane => [AxisDomain::Unbounded; 2],
            Shape::Cylinder(_) => [periodic, AxisDomain::Unbounded],
            Shape::Cone(_) => [periodic, AxisDomain::LowerBounded { min: 0. }],
            Shape::Sphere(_) => [
                periodic,
                AxisDomain::Closed {
                    min: -FRAC_PI_2,
                    max: FRAC_PI_2,
                },
            ],
            Shape::Torus(..) => [periodic; 2],
        }
    }
    pub fn evaluate(self, u: f64, v: f64) -> Result<Evaluation<S>, Error> {
        finite(u)?;
        finite(v)?;
        let domains = self.domain();
        if !domains[0].contains(u) || !domains[1].contains(v) {
            return Err(Error::ParameterOutsideDomain);
        }
        let (s, c) = u.sin_cos();
        let zero = [0.; 3];
        let (p, du, dv, duu, duv, dvv) = match self.shape {
            Shape::Plane => ([u, v, 0.], [1., 0., 0.], [0., 1., 0.], zero, zero, zero),
            Shape::Cylinder(r) => (
                [r * c, r * s, v],
                [-r * s, r * c, 0.],
                [0., 0., 1.],
                [-r * c, -r * s, 0.],
                zero,
                zero,
            ),
            Shape::Cone(a) => {
                let (sa, ca) = a.sin_cos();
                let r = finite(v * sa)?;
                (
                    [r * c, r * s, v * ca],
                    [-r * s, r * c, 0.],
                    [sa * c, sa * s, ca],
                    [-r * c, -r * s, 0.],
                    [-sa * s, sa * c, 0.],
                    zero,
                )
            }
            Shape::Sphere(r) => {
                let (sv, mut cv) = v.sin_cos();
                if v.abs() == FRAC_PI_2 {
                    cv = 0.;
                }
                let rc = r * cv;
                let rs = r * sv;
                (
                    [rc * c, rc * s, rs],
                    [-rc * s, rc * c, 0.],
                    [-rs * c, -rs * s, rc],
                    [-rc * c, -rc * s, 0.],
                    [rs * s, -rs * c, 0.],
                    [-rc * c, -rc * s, -rs],
                )
            }
            Shape::Torus(a, b) => {
                let (sv, cv) = v.sin_cos();
                let bc = b * cv;
                let bs = b * sv;
                let r = finite(a + bc)?;
                (
                    [r * c, r * s, bs],
                    [-r * s, r * c, 0.],
                    [-bs * c, -bs * s, bc],
                    [-r * c, -r * s, 0.],
                    [bs * s, -bs * c, 0.],
                    [-bc * c, -bc * s, -bs],
                )
            }
        };
        let z = self.frame.x().cross(self.frame.y())?.normalized()?.vector();
        let vector = |p: [f64; 3]| -> Result<Vector3<S>, Error> {
            Ok(self
                .frame
                .x()
                .scaled(p[0])?
                .added(self.frame.y().scaled(p[1])?)?
                .added(z.scaled(p[2])?)?)
        };
        Ok(Evaluation {
            position: self.frame.origin().translated(vector(p)?)?,
            du: vector(du)?,
            dv: vector(dv)?,
            duu: vector(duu)?,
            duv: vector(duv)?,
            dvv: vector(dvv)?,
        })
    }
}
fn positive(length: Length) -> Result<f64, Error> {
    let v = length.as_metres();
    if v > 0. {
        Ok(v)
    } else {
        Err(Error::InvalidShape)
    }
}
