use crate::{Error, Evaluation};
use tessstep_curves::spline::{
    HARD_MAX_CONTROLS, KnotSide, KnotVector, MAX_DEGREE, SplineError, SplineLimits,
    validate_weights,
};
use tessstep_math::{Point3, Space, Vector3};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterAxis {
    U,
    V,
}
/// Tensor-product positive-weight NURBS. Control (i,j) is stored at i*nv+j.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface<S: Space> {
    u: KnotVector,
    v: KnotVector,
    controls: Vec<Point3<S>>,
    weights: Vec<f64>,
}
impl<S: Space> NurbsSurface<S> {
    pub fn new(
        degrees: [usize; 2],
        knots: [&[f64]; 2],
        shape: [usize; 2],
        controls: &[Point3<S>],
        weights: &[f64],
        limits: SplineLimits,
    ) -> Result<Self, Error> {
        let total = shape[0]
            .checked_mul(shape[1])
            .ok_or(SplineError::ResourceLimit)?;
        let knot_count = knots[0]
            .len()
            .checked_add(knots[1].len())
            .ok_or(SplineError::ResourceLimit)?;
        if total > limits.max_controls.min(HARD_MAX_CONTROLS)
            || knot_count
                > limits
                    .max_knots
                    .min(2 * (HARD_MAX_CONTROLS + MAX_DEGREE + 1))
        {
            return Err(SplineError::ResourceLimit.into());
        }
        if total != controls.len() || total != weights.len() {
            return Err(SplineError::InvalidControls.into());
        }
        let scale = validate_weights(weights)?;
        let u = KnotVector::new(degrees[0], shape[0], knots[0], limits)?;
        let v = KnotVector::new(degrees[1], shape[1], knots[1], limits)?;
        Ok(Self {
            u,
            v,
            controls: controls.to_vec(),
            weights: weights.iter().map(|w| w / scale).collect(),
        })
    }
    pub fn shape(&self) -> [usize; 2] {
        [self.u.control_count(), self.v.control_count()]
    }
    pub fn knot_vectors(&self) -> [&KnotVector; 2] {
        [&self.u, &self.v]
    }
    pub fn controls(&self) -> &[Point3<S>] {
        &self.controls
    }
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }
    pub fn evaluate(&self, u: f64, v: f64) -> Result<Evaluation<S>, Error> {
        self.evaluate_on_sides(u, v, [KnotSide::Right; 2])
    }
    pub fn evaluate_on_sides(
        &self,
        u: f64,
        v: f64,
        sides: [KnotSide; 2],
    ) -> Result<Evaluation<S>, Error> {
        let bu = self.u.basis_on_side(u, sides[0])?;
        let bv = self.v.basis_on_side(v, sides[1])?;
        let a = [bu.values(), bu.first(), bu.second()];
        let b = [bv.values(), bv.first(), bv.second()];
        let orders = [(0, 0), (1, 0), (0, 1), (2, 0), (1, 1), (0, 2)];
        let mut h = [[0.; 3]; 6];
        let mut w = [0.; 6];
        let nv = self.v.control_count();
        let mut scale = 0_f64;
        for (i, _) in a[0].iter().enumerate() {
            for (j, _) in b[0].iter().enumerate() {
                scale = scale.max(self.weights[(bu.start() + i) * nv + bv.start() + j]);
            }
        }
        for (i, _) in a[0].iter().enumerate() {
            for (j, _) in b[0].iter().enumerate() {
                let index = (bu.start() + i) * nv + bv.start() + j;
                let weight = self.weights[index] / scale;
                let point = self.controls[index].coordinates();
                for (d, &(du, dv)) in orders.iter().enumerate() {
                    let coefficient = checked(a[du][i] * b[dv][j] * weight)?;
                    w[d] = checked(w[d] + coefficient)?;
                    for (k, &coordinate) in point.iter().enumerate() {
                        h[d][k] = checked(h[d][k] + coefficient * coordinate)?;
                    }
                }
            }
        }
        if w[0] <= 0. {
            return Err(SplineError::NumericRange.into());
        }
        let p = std::array::from_fn(|i| h[0][i] / w[0]);
        let position = Point3::new(p)?;
        let du = Vector3::new(std::array::from_fn(|i| (h[1][i] - w[1] * p[i]) / w[0]))?;
        let dv = Vector3::new(std::array::from_fn(|i| (h[2][i] - w[2] * p[i]) / w[0]))?;
        let a = du.components();
        let b = dv.components();
        let duu = Vector3::new(std::array::from_fn(|i| {
            (h[3][i] - 2. * w[1] * a[i] - w[3] * p[i]) / w[0]
        }))?;
        let duv = Vector3::new(std::array::from_fn(|i| {
            (h[4][i] - w[1] * b[i] - w[2] * a[i] - w[4] * p[i]) / w[0]
        }))?;
        let dvv = Vector3::new(std::array::from_fn(|i| {
            (h[5][i] - 2. * w[2] * b[i] - w[5] * p[i]) / w[0]
        }))?;
        Ok(Evaluation {
            position,
            du,
            dv,
            duu,
            duv,
            dvv,
        })
    }
    /// Insert once along every control row/column, preserving the common weight scale.
    pub fn insert_knot(
        &self,
        axis: ParameterAxis,
        t: f64,
        limits: SplineLimits,
    ) -> Result<Self, Error> {
        let d = if axis == ParameterAxis::U { 0 } else { 1 };
        let vectors = self.knot_vectors();
        let vector = vectors[d];
        let knots = vector.knots();
        let mut shape = self.shape();
        shape[d] = shape[d].checked_add(1).ok_or(SplineError::ResourceLimit)?;
        let total = shape[0]
            .checked_mul(shape[1])
            .ok_or(SplineError::ResourceLimit)?;
        if total > limits.max_controls.min(HARD_MAX_CONTROLS)
            || self.u.knots().len() + self.v.knots().len() + 1 > limits.max_knots
        {
            return Err(SplineError::ResourceLimit.into());
        }
        let [min, max] = vector.domain();
        if !t.is_finite() || t <= min || t >= max {
            return Err(SplineError::InsertionLimit.into());
        }
        let p = vector.degree();
        let k = vector.span(t, KnotSide::Right)?;
        let s = knots.partition_point(|&v| v <= t) - knots.partition_point(|&v| v < t);
        if s >= p {
            return Err(SplineError::InsertionLimit.into());
        }
        let old_nv = self.v.control_count();
        let mut controls = Vec::with_capacity(total);
        let mut weights = Vec::with_capacity(total);
        for i in 0..shape[0] {
            for j in 0..shape[1] {
                let along = if d == 0 { i } else { j };
                let old_index = |x: usize| {
                    if d == 0 {
                        x * old_nv + j
                    } else {
                        i * old_nv + x
                    }
                };
                if along <= k - p {
                    let a = old_index(along);
                    controls.push(self.controls[a]);
                    weights.push(self.weights[a]);
                } else if along > k - s {
                    let a = old_index(along - 1);
                    controls.push(self.controls[a]);
                    weights.push(self.weights[a]);
                } else {
                    let alpha = checked((t - knots[along]) / (knots[along + p] - knots[along]))?;
                    let left = old_index(along - 1);
                    let right = old_index(along);
                    let a = (1. - alpha) * self.weights[left];
                    let b = alpha * self.weights[right];
                    let w = checked(a + b)?;
                    if w <= 0. {
                        return Err(SplineError::NumericRange.into());
                    }
                    let l = self.controls[left].coordinates();
                    let r = self.controls[right].coordinates();
                    controls.push(Point3::new(std::array::from_fn(|z| {
                        (a * l[z] + b * r[z]) / w
                    }))?);
                    weights.push(w);
                }
            }
        }
        let mut expanded = Vec::with_capacity(knots.len() + 1);
        expanded.extend_from_slice(&knots[..=k]);
        expanded.push(t);
        expanded.extend_from_slice(&knots[k + 1..]);
        let new_knots = if d == 0 {
            [expanded.as_slice(), self.v.knots()]
        } else {
            [self.u.knots(), expanded.as_slice()]
        };
        Self::new(
            [self.u.degree(), self.v.degree()],
            new_knots,
            shape,
            &controls,
            &weights,
            limits,
        )
    }
}
fn checked(x: f64) -> Result<f64, Error> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(SplineError::NumericRange.into())
    }
}
