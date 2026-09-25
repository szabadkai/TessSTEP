use crate::{Direction3, Error, NumericalTolerance, Point3, Space, Vector3, finite};
use std::marker::PhantomData;

const IDENTITY: [[f64; 3]; 3] = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];

/// Finite affine map with column vectors: `p_to = linear * p_from + translation`.
/// Rows are stored in row-major order. Singular maps may be constructed/applied;
/// inversion and normal transformation reject them explicitly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine3<From: Space, To: Space> {
    linear: [[f64; 3]; 3],
    translation: [f64; 3],
    spaces: PhantomData<(From, To)>,
}
impl<From: Space, To: Space> Affine3<From, To> {
    pub fn new(linear: [[f64; 3]; 3], translation: [f64; 3]) -> Result<Self, Error> {
        for v in linear.into_iter().flatten().chain(translation) {
            finite(v)?;
        }
        Ok(Self {
            linear,
            translation,
            spaces: PhantomData,
        })
    }
    pub fn linear(self) -> [[f64; 3]; 3] {
        self.linear
    }
    pub fn translation(self) -> [f64; 3] {
        self.translation
    }
    pub fn transform_point(self, point: Point3<From>) -> Result<Point3<To>, Error> {
        let p = multiply(self.linear, point.coordinates());
        Point3::new(std::array::from_fn(|i| p[i] + self.translation[i]))
    }
    /// Translation never acts on a vector.
    pub fn transform_vector(self, vector: Vector3<From>) -> Result<Vector3<To>, Error> {
        Vector3::new(multiply(self.linear, vector.components()))
    }
    pub fn transform_direction(self, direction: Direction3<From>) -> Result<Direction3<To>, Error> {
        self.transform_vector(direction.vector())?.normalized()
    }
    /// Inverse-transpose normal, renormalized. Reflection does not flip it to
    /// match transformed winding; orientation handling remains the caller's job.
    pub fn transform_normal(
        self,
        normal: Direction3<From>,
        tolerance: NumericalTolerance,
    ) -> Result<Direction3<To>, Error> {
        let inv = inverse_linear(self.linear, tolerance)?;
        let transpose = std::array::from_fn(|i| std::array::from_fn(|j| inv[j][i]));
        Direction3::new(multiply(transpose, normal.vector().components()))
    }
    /// Apply `self`, then `next`: the returned matrix is `next * self`.
    pub fn then<Next: Space>(self, next: Affine3<To, Next>) -> Result<Affine3<From, Next>, Error> {
        let linear = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..3).map(|k| next.linear[i][k] * self.linear[k][j]).sum())
        });
        let t = multiply(next.linear, self.translation);
        let translation = std::array::from_fn(|i| t[i] + next.translation[i]);
        Affine3::new(linear, translation)
    }
    /// Partial-pivot Gauss-Jordan inversion after scaling the linear matrix by
    /// its largest absolute entry. Rejects pivots <= the relative threshold.
    /// This is a numerical safeguard, not a condition-number/error certificate.
    pub fn inverse(self, tolerance: NumericalTolerance) -> Result<Affine3<To, From>, Error> {
        let linear = inverse_linear(self.linear, tolerance)?;
        let translation = multiply(linear, self.translation).map(|v| -v);
        Affine3::new(linear, translation)
    }
    /// Construct a right-handed orthonormal frame with explicit axis inputs.
    /// `axis` defines z; `reference` selects x after projection normal to z.
    /// No STEP defaulting rules are inferred here.
    pub fn from_frame(
        origin: Point3<To>,
        axis: Direction3<To>,
        reference: Direction3<To>,
        tolerance: NumericalTolerance,
    ) -> Result<Self, Error> {
        let y = axis.vector().cross(reference.vector())?;
        if y.norm()? <= tolerance.relative() {
            return Err(Error::ParallelAxes);
        }
        let y = y.normalized()?;
        let x = y.vector().cross(axis.vector())?.normalized()?;
        let columns = [
            x.vector().components(),
            y.vector().components(),
            axis.vector().components(),
        ];
        Self::new(
            std::array::from_fn(|i| std::array::from_fn(|j| columns[j][i])),
            origin.coordinates(),
        )
    }
}
impl<S: Space> Affine3<S, S> {
    pub fn identity() -> Self {
        Self {
            linear: IDENTITY,
            translation: [0.; 3],
            spaces: PhantomData,
        }
    }
    pub fn from_translation(offset: Vector3<S>) -> Self {
        Self {
            translation: offset.components(),
            ..Self::identity()
        }
    }
    /// Rodrigues rotation about an explicit unit axis; no implicit origin shift.
    pub fn rotation(axis: Direction3<S>, angle: crate::Angle) -> Result<Self, Error> {
        let [x, y, z] = axis.vector().components();
        let (s, c) = angle.as_radians().sin_cos();
        let d = 1. - c;
        Self::new(
            [
                [c + x * x * d, x * y * d - z * s, x * z * d + y * s],
                [y * x * d + z * s, c + y * y * d, y * z * d - x * s],
                [z * x * d - y * s, z * y * d + x * s, c + z * z * d],
            ],
            [0.; 3],
        )
    }
}
fn multiply(a: [[f64; 3]; 3], x: [f64; 3]) -> [f64; 3] {
    a.map(|row| row.iter().zip(x).map(|(a, b)| a * b).sum())
}
fn inverse_linear(
    matrix: [[f64; 3]; 3],
    tolerance: NumericalTolerance,
) -> Result<[[f64; 3]; 3], Error> {
    let scale = matrix.iter().flatten().fold(0_f64, |a, b| a.max(b.abs()));
    if scale == 0. {
        return Err(Error::SingularTransform);
    }
    let mut rows = [[0.; 6]; 3];
    for (i, row) in rows.iter_mut().enumerate() {
        for (j, v) in matrix[i].iter().enumerate() {
            row[j] = v / scale;
        }
        row[3 + i] = 1.;
    }
    for column in 0..3 {
        let mut pivot = column;
        for row in column + 1..3 {
            if rows[row][column].abs() > rows[pivot][column].abs() {
                pivot = row;
            }
        }
        if rows[pivot][column].abs() <= tolerance.relative() {
            return Err(Error::SingularTransform);
        }
        rows.swap(column, pivot);
        let divisor = rows[column][column];
        for value in &mut rows[column] {
            *value = finite(*value / divisor)?;
        }
        let selected = rows[column];
        for (i, row) in rows.iter_mut().enumerate() {
            if i == column {
                continue;
            }
            let factor = row[column];
            for (j, value) in row.iter_mut().enumerate() {
                *value = finite(*value - factor * selected[j])?;
            }
        }
    }
    let mut inverse = [[0.; 3]; 3];
    for (i, row) in inverse.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = finite(rows[i][j + 3] / scale)?;
        }
    }
    Ok(inverse)
}
