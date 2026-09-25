use crate::{Error, dimension};
use tessstep_math::{NumericalTolerance, Point, Space, Vector};

/// Orthonormal plane basis to floating-point accuracy. In 2D either orientation
/// is allowed; in 3D x cross y defines the positive plane normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame<S: Space, const N: usize> {
    origin: Point<S, N>,
    x: Vector<S, N>,
    y: Vector<S, N>,
}
impl<S: Space, const N: usize> PlaneFrame<S, N> {
    /// `x` defines the first axis. `reference_y` chooses the side of the second
    /// axis after projection perpendicular to x; it need not be perpendicular.
    /// Rejects sine separation <= the explicit dimensionless tolerance.
    pub fn new(
        origin: Point<S, N>,
        x: Vector<S, N>,
        reference_y: Vector<S, N>,
        tolerance: NumericalTolerance,
    ) -> Result<Self, Error> {
        dimension::<N>()?;
        let x = normalize(x)?;
        let reference = normalize(reference_y)?;
        let a = x.components();
        let b = reference.components();
        let y = if N == 2 {
            let sine = a[0] * b[1] - a[1] * b[0];
            if sine.abs() <= tolerance.relative() {
                return Err(tessstep_math::Error::ParallelAxes.into());
            }
            Vector::new(std::array::from_fn(|i| {
                if i == 0 {
                    -a[1] * sine.signum()
                } else {
                    a[0] * sine.signum()
                }
            }))?
        } else {
            let cross: Vector<S, N> = Vector::new(std::array::from_fn(|i| {
                a[(i + 1) % 3] * b[(i + 2) % 3] - a[(i + 2) % 3] * b[(i + 1) % 3]
            }))?;
            if cross.norm()? <= tolerance.relative() {
                return Err(tessstep_math::Error::ParallelAxes.into());
            }
            let n = normalize(cross)?.components();
            normalize(Vector::new(std::array::from_fn(|i| {
                n[(i + 1) % 3] * a[(i + 2) % 3] - n[(i + 2) % 3] * a[(i + 1) % 3]
            }))?)?
        };
        Ok(Self { origin, x, y })
    }
    pub fn origin(self) -> Point<S, N> {
        self.origin
    }
    pub fn x(self) -> Vector<S, N> {
        self.x
    }
    pub fn y(self) -> Vector<S, N> {
        self.y
    }
    pub(crate) fn vector(self, x: f64, y: f64) -> Result<Vector<S, N>, Error> {
        Ok(self.x.scaled(x)?.added(self.y.scaled(y)?)?)
    }
    pub(crate) fn point(self, x: f64, y: f64) -> Result<Point<S, N>, Error> {
        Ok(self.origin.translated(self.vector(x, y)?)?)
    }
}
fn normalize<S: Space, const N: usize>(vector: Vector<S, N>) -> Result<Vector<S, N>, Error> {
    let components = vector.components();
    let scale = components.iter().fold(0_f64, |a, b| a.max(b.abs()));
    if scale == 0. {
        return Err(tessstep_math::Error::ZeroDirection.into());
    }
    let scaled = Vector::new(components.map(|v| v / scale))?;
    Ok(scaled.scaled(1. / scaled.norm()?)?)
}
