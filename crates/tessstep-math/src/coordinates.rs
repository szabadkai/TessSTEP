use crate::{Error, finite};
use std::marker::PhantomData;

/// A coordinate-frame identity; callers may define additional distinct frames.
pub trait Space: Copy + std::fmt::Debug + PartialEq {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelSpace {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSpace {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterSpace {}
impl Space for ModelSpace {}
impl Space for LocalSpace {}
impl Space for ParameterSpace {}

/// Finite point coordinates. Points cannot be added to points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point<S: Space, const N: usize> {
    coordinates: [f64; N],
    space: PhantomData<S>,
}
/// Finite vector components; scalar results use this space's coordinate units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector<S: Space, const N: usize> {
    components: [f64; N],
    space: PhantomData<S>,
}
pub type Point2<S> = Point<S, 2>;
pub type Point3<S> = Point<S, 3>;
pub type Vector2<S> = Vector<S, 2>;
pub type Vector3<S> = Vector<S, 3>;

fn checked<const N: usize>(values: [f64; N]) -> Result<[f64; N], Error> {
    if N == 0 {
        return Err(Error::InvalidDimension);
    }
    for v in values {
        finite(v)?;
    }
    Ok(values)
}
impl<S: Space, const N: usize> Point<S, N> {
    pub fn new(coordinates: [f64; N]) -> Result<Self, Error> {
        Ok(Self {
            coordinates: checked(coordinates)?,
            space: PhantomData,
        })
    }
    pub fn coordinates(self) -> [f64; N] {
        self.coordinates
    }
    pub fn translated(self, vector: Vector<S, N>) -> Result<Self, Error> {
        Self::new(std::array::from_fn(|i| {
            self.coordinates[i] + vector.components[i]
        }))
    }
    /// Vector from `other` to `self`.
    pub fn difference(self, other: Self) -> Result<Vector<S, N>, Error> {
        Vector::new(std::array::from_fn(|i| {
            self.coordinates[i] - other.coordinates[i]
        }))
    }
    pub fn distance(self, other: Self) -> Result<f64, Error> {
        self.difference(other)?.norm()
    }
}
impl<S: Space, const N: usize> Vector<S, N> {
    pub fn new(components: [f64; N]) -> Result<Self, Error> {
        Ok(Self {
            components: checked(components)?,
            space: PhantomData,
        })
    }
    pub fn components(self) -> [f64; N] {
        self.components
    }
    pub fn added(self, other: Self) -> Result<Self, Error> {
        Self::new(std::array::from_fn(|i| {
            self.components[i] + other.components[i]
        }))
    }
    pub fn subtracted(self, other: Self) -> Result<Self, Error> {
        Self::new(std::array::from_fn(|i| {
            self.components[i] - other.components[i]
        }))
    }
    pub fn scaled(self, factor: f64) -> Result<Self, Error> {
        finite(factor)?;
        Self::new(self.components.map(|v| v * factor))
    }
    pub fn dot(self, other: Self) -> Result<f64, Error> {
        finite(
            self.components
                .iter()
                .zip(other.components)
                .map(|(a, b)| a * b)
                .sum(),
        )
    }
    /// Hypotenuse accumulation avoids squaring overflow/underflow.
    pub fn norm(self) -> Result<f64, Error> {
        finite(self.components.iter().fold(0_f64, |n, &v| n.hypot(v)))
    }
}
impl<S: Space> Vector3<S> {
    pub fn cross(self, other: Self) -> Result<Self, Error> {
        let [a, b, c] = self.components;
        let [x, y, z] = other.components;
        Self::new([b * z - c * y, c * x - a * z, a * y - b * x])
    }
    pub fn normalized(self) -> Result<Direction3<S>, Error> {
        // Scale first, so even a vector whose norm exceeds f64::MAX can normalize.
        let scale = self.components.iter().fold(0_f64, |a, b| a.max(b.abs()));
        if scale == 0. {
            return Err(Error::ZeroDirection);
        }
        let scaled = self.components.map(|v| v / scale);
        let norm = scaled[0].hypot(scaled[1]).hypot(scaled[2]);
        Ok(Direction3(Vector::new(scaled.map(|v| v / norm))?))
    }
}
/// Unit length to floating-point accuracy, constructed only by normalization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direction3<S: Space>(Vector3<S>);
impl<S: Space> Direction3<S> {
    pub fn new(components: [f64; 3]) -> Result<Self, Error> {
        Vector3::new(components)?.normalized()
    }
    pub fn vector(self) -> Vector3<S> {
        self.0
    }
    /// Unsigned angle in [0, pi], using atan2 rather than an unstable acos near 1.
    pub fn angle_to(self, other: Self) -> Result<crate::Angle, Error> {
        let sine = self.0.cross(other.0)?.norm()?;
        let cosine = self.0.dot(other.0)?;
        crate::Angle::radians(sine.atan2(cosine))
    }
}
