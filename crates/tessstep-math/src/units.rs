use crate::{Error, finite, positive};

/// Nonnegative distance in metres, separate from signed coordinate components.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Length(f64);
impl Length {
    pub fn metres(value: f64) -> Result<Self, Error> {
        finite(value)?;
        if value < 0. {
            return Err(Error::OutOfRange);
        }
        Ok(Self(value))
    }
    pub fn as_metres(self) -> f64 {
        self.0
    }
}
/// Signed finite angle in radians. No implicit periodic reduction.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Angle(f64);
impl Angle {
    pub fn radians(value: f64) -> Result<Self, Error> {
        Ok(Self(finite(value)?))
    }
    pub fn degrees(value: f64) -> Result<Self, Error> {
        Self::radians(convert(value, std::f64::consts::PI / 180.)?)
    }
    pub fn as_radians(self) -> f64 {
        self.0
    }
}
/// Explicit positive source-length scale. This type interprets no STEP entities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LengthUnit(f64);
impl LengthUnit {
    pub const METRE: Self = Self(1.);
    pub const MILLIMETRE: Self = Self(0.001);
    pub const INCH: Self = Self(0.0254);
    pub fn metres_per_unit(value: f64) -> Result<Self, Error> {
        Ok(Self(positive(value)?))
    }
    pub fn scale(self) -> f64 {
        self.0
    }
    /// Signed coordinates are permitted. Nonzero values rounding to zero fail.
    pub fn to_metres(self, value: f64) -> Result<f64, Error> {
        convert(value, self.0)
    }
    pub fn from_metres(self, value: f64) -> Result<f64, Error> {
        finite(value)?;
        let result = finite(value / self.0)?;
        if value != 0. && result == 0. {
            return Err(Error::OutOfRange);
        }
        Ok(result)
    }
}
fn convert(value: f64, scale: f64) -> Result<f64, Error> {
    finite(value)?;
    let result = finite(value * scale)?;
    if value != 0. && result == 0. {
        return Err(Error::OutOfRange);
    }
    Ok(result)
}
fn tolerance(distance: Length, angle: Angle) -> Result<(), Error> {
    if distance.0 <= 0. || angle.0 <= 0. || angle.0 > std::f64::consts::PI {
        return Err(Error::InvalidTolerance);
    }
    Ok(())
}
/// Caller-supplied modeling distance and angle tolerances; no global defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelTolerance {
    distance: Length,
    angle: Angle,
}
impl ModelTolerance {
    pub fn new(distance: Length, angle: Angle) -> Result<Self, Error> {
        tolerance(distance, angle)?;
        Ok(Self { distance, angle })
    }
    pub fn distance(self) -> Length {
        self.distance
    }
    pub fn angle(self) -> Angle {
        self.angle
    }
}
/// Requested tessellation errors, independent of model validity tolerances.
/// This is a value contract; no tessellator is implemented yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TessellationTolerance {
    chord: Length,
    normal_angle: Angle,
}
impl TessellationTolerance {
    pub fn new(chord: Length, normal_angle: Angle) -> Result<Self, Error> {
        tolerance(chord, normal_angle)?;
        Ok(Self {
            chord,
            normal_angle,
        })
    }
    pub fn chord(self) -> Length {
        self.chord
    }
    pub fn normal_angle(self) -> Angle {
        self.normal_angle
    }
}
/// Dimensionless numerical threshold in (0, 1). Used for scaled matrix pivots
/// and the sine between frame axes, never as a modeling distance tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericalTolerance(f64);
impl NumericalTolerance {
    pub fn new(relative: f64) -> Result<Self, Error> {
        if !relative.is_finite() || relative <= 0. || relative >= 1. {
            return Err(Error::InvalidTolerance);
        }
        Ok(Self(relative))
    }
    pub fn relative(self) -> f64 {
        self.0
    }
}
impl Default for NumericalTolerance {
    fn default() -> Self {
        Self(64. * f64::EPSILON)
    }
}
