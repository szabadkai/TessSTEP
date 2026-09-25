use super::{
    HARD_MAX_CONTROLS, KnotSide, KnotVector, SplineError, SplineLimits, checked, validate_weights,
};
use crate::Evaluation;
use tessstep_math::{Point, Space, Vector};
/// Immutable positive-weight rational B-spline curve; weights share a normalized scale.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve<S: Space, const N: usize> {
    knots: KnotVector,
    controls: Vec<Point<S, N>>,
    weights: Vec<f64>,
}
impl<S: Space, const N: usize> NurbsCurve<S, N> {
    pub fn new(
        degree: usize,
        knots: &[f64],
        controls: &[Point<S, N>],
        weights: &[f64],
        limits: SplineLimits,
    ) -> Result<Self, SplineError> {
        if controls.len() > limits.max_controls.min(HARD_MAX_CONTROLS) {
            return Err(SplineError::ResourceLimit);
        }
        if (N != 2 && N != 3) || controls.len() != weights.len() {
            return Err(SplineError::InvalidControls);
        }
        let scale = validate_weights(weights)?;
        let knots = KnotVector::new(degree, controls.len(), knots, limits)?;
        Ok(Self {
            knots,
            controls: controls.to_vec(),
            weights: weights.iter().map(|w| w / scale).collect(),
        })
    }
    pub fn knot_vector(&self) -> &KnotVector {
        &self.knots
    }
    pub fn controls(&self) -> &[Point<S, N>] {
        &self.controls
    }
    /// Weights are normalized by a common positive scale; geometry is unchanged.
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }
    pub fn evaluate(&self, u: f64) -> Result<Evaluation<S, N>, SplineError> {
        self.evaluate_on_side(u, KnotSide::Right)
    }
    pub fn evaluate_on_side(
        &self,
        u: f64,
        side: KnotSide,
    ) -> Result<Evaluation<S, N>, SplineError> {
        let basis = self.knots.basis_on_side(u, side)?;
        let values = [basis.values(), basis.first(), basis.second()];
        let mut h = [[0.; N]; 3];
        let mut w = [0.; 3];
        let scale = self.weights[basis.start()..basis.start() + values[0].len()]
            .iter()
            .copied()
            .fold(0_f64, f64::max);
        for (j, _) in values[0].iter().enumerate() {
            let i = basis.start() + j;
            let weight = self.weights[i] / scale;
            let point = self.controls[i].coordinates();
            for d in 0..3 {
                let coefficient = checked(values[d][j] * weight)?;
                w[d] = checked(w[d] + coefficient)?;
                for k in 0..N {
                    h[d][k] = checked(h[d][k] + coefficient * point[k])?;
                }
            }
        }
        if w[0] <= 0. {
            return Err(SplineError::NumericRange);
        }
        let p = std::array::from_fn(|i| h[0][i] / w[0]);
        let position = Point::new(p)?;
        let first = Vector::new(std::array::from_fn(|i| (h[1][i] - w[1] * p[i]) / w[0]))?;
        let d = first.components();
        let second = Vector::new(std::array::from_fn(|i| {
            (h[2][i] - 2. * w[1] * d[i] - w[2] * p[i]) / w[0]
        }))?;
        Ok(Evaluation {
            position,
            first,
            second,
        })
    }
    /// One interior Boehm knot insertion, in homogeneous coordinates.
    /// Returned controls/knots are bounded by the supplied limits; self is unchanged.
    pub fn insert_knot(&self, u: f64, limits: SplineLimits) -> Result<Self, SplineError> {
        let n = self.controls.len();
        if n >= limits.max_controls.min(HARD_MAX_CONTROLS)
            || self.knots.knots().len()
                >= limits
                    .max_knots
                    .min(HARD_MAX_CONTROLS + super::MAX_DEGREE + 1)
        {
            return Err(SplineError::ResourceLimit);
        }
        let [min, max] = self.knots.domain();
        if !u.is_finite() || u <= min || u >= max {
            return Err(SplineError::InsertionLimit);
        }
        let p = self.knots.degree();
        let k = self.knots.span(u, KnotSide::Right)?;
        let knots = self.knots.knots();
        let multiplicity = knots.partition_point(|&v| v <= u) - knots.partition_point(|&v| v < u);
        if multiplicity >= p {
            return Err(SplineError::InsertionLimit);
        }
        let mut controls = Vec::with_capacity(n + 1);
        let mut weights = Vec::with_capacity(n + 1);
        for i in 0..=n {
            if i <= k - p {
                controls.push(self.controls[i]);
                weights.push(self.weights[i]);
            } else if i > k - multiplicity {
                controls.push(self.controls[i - 1]);
                weights.push(self.weights[i - 1]);
            } else {
                let alpha = checked((u - knots[i]) / (knots[i + p] - knots[i]))?;
                let a = (1. - alpha) * self.weights[i - 1];
                let b = alpha * self.weights[i];
                let w = checked(a + b)?;
                if w <= 0. {
                    return Err(SplineError::NumericRange);
                }
                let left = self.controls[i - 1].coordinates();
                let right = self.controls[i].coordinates();
                controls.push(Point::new(std::array::from_fn(|j| {
                    (a * left[j] + b * right[j]) / w
                }))?);
                weights.push(w);
            }
        }
        let mut expanded = Vec::with_capacity(knots.len() + 1);
        expanded.extend_from_slice(&knots[..=k]);
        expanded.push(u);
        expanded.extend_from_slice(&knots[k + 1..]);
        Self::new(p, &expanded, &controls, &weights, limits)
    }
}
