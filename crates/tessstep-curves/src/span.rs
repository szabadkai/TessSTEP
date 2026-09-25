use crate::{Curve, Error, Evaluation, finite};
use tessstep_math::Space;

/// Oriented finite interval on an analytic basis, evaluated on s in `[0,1]`.
/// Unwrapped periodic endpoints retain seams, direction and multiple revolutions.
/// This is a parameter span, not STEP trimming, a topological edge or UV trimming.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveSpan<S: Space, const N: usize> {
    basis: Curve<S, N>,
    start: f64,
    end: f64,
    delta: f64,
}
impl<S: Space, const N: usize> CurveSpan<S, N> {
    pub fn new(basis: Curve<S, N>, start: f64, end: f64) -> Result<Self, Error> {
        finite(start)?;
        finite(end)?;
        let delta = end - start;
        if !delta.is_finite() || delta == 0. {
            return Err(Error::InvalidInterval);
        }
        Ok(Self {
            basis,
            start,
            end,
            delta,
        })
    }
    pub fn basis(self) -> Curve<S, N> {
        self.basis
    }
    pub fn endpoints(self) -> [f64; 2] {
        [self.start, self.end]
    }
    pub fn reversed(self) -> Self {
        Self {
            start: self.end,
            end: self.start,
            delta: -self.delta,
            ..self
        }
    }
    /// Exact endpoint selection avoids losing the stored endpoint to cancellation.
    pub fn basis_parameter(self, s: f64) -> Result<f64, Error> {
        finite(s)?;
        if !(0. ..=1.).contains(&s) {
            return Err(Error::ParameterOutsideSpan);
        }
        if s == 0. {
            return Ok(self.start);
        }
        if s == 1. {
            return Ok(self.end);
        }
        finite(self.start + s * self.delta)
    }
    /// Derivatives are with respect to s: C'(u)*delta and C''(u)*delta^2.
    pub fn evaluate(self, s: f64) -> Result<Evaluation<S, N>, Error> {
        let value = self.basis.evaluate(self.basis_parameter(s)?)?;
        Ok(Evaluation {
            position: value.position,
            first: value.first.scaled(self.delta)?,
            second: value.second.scaled(self.delta)?.scaled(self.delta)?,
        })
    }
}
