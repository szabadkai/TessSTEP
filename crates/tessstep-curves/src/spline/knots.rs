use super::{HARD_MAX_CONTROLS, MAX_DEGREE, SplineError, SplineLimits, checked};
/// One-sided evaluation at repeated knots. Endpoints always use the inward side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnotSide {
    Left,
    Right,
}
#[derive(Clone, Debug, PartialEq)]
pub struct KnotVector {
    degree: usize,
    controls: usize,
    knots: Vec<f64>,
}
/// Local support values and first/second derivatives, in control-point order.
#[derive(Clone, Debug, PartialEq)]
pub struct BasisSample {
    start: usize,
    count: usize,
    rows: [[f64; MAX_DEGREE + 1]; 3],
}
impl BasisSample {
    pub fn start(&self) -> usize {
        self.start
    }
    pub fn values(&self) -> &[f64] {
        &self.rows[0][..self.count]
    }
    pub fn first(&self) -> &[f64] {
        &self.rows[1][..self.count]
    }
    pub fn second(&self) -> &[f64] {
        &self.rows[2][..self.count]
    }
}
impl KnotVector {
    pub fn new(
        degree: usize,
        controls: usize,
        knots: &[f64],
        limits: SplineLimits,
    ) -> Result<Self, SplineError> {
        if degree == 0 || degree > MAX_DEGREE {
            return Err(SplineError::InvalidDegree);
        }
        if controls > limits.max_controls.min(HARD_MAX_CONTROLS)
            || knots.len() > limits.max_knots.min(HARD_MAX_CONTROLS + MAX_DEGREE + 1)
        {
            return Err(SplineError::ResourceLimit);
        }
        if controls <= degree || knots.len() != controls + degree + 1 {
            return Err(SplineError::InvalidKnots);
        }
        let mut run = 0;
        let mut prev = None;
        for &k in knots {
            if !k.is_finite() || prev.is_some_and(|p| k < p) {
                return Err(SplineError::InvalidKnots);
            }
            run = if prev == Some(k) { run + 1 } else { 1 };
            if run > degree + 1 {
                return Err(SplineError::InvalidKnots);
            }
            prev = Some(k);
        }
        if knots[degree] >= knots[controls] || !(knots[knots.len() - 1] - knots[0]).is_finite() {
            return Err(SplineError::InvalidKnots);
        }
        Ok(Self {
            degree,
            controls,
            knots: knots.to_vec(),
        })
    }
    pub fn degree(&self) -> usize {
        self.degree
    }
    pub fn control_count(&self) -> usize {
        self.controls
    }
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }
    pub fn domain(&self) -> [f64; 2] {
        [self.knots[self.degree], self.knots[self.controls]]
    }
    pub fn span(&self, u: f64, side: KnotSide) -> Result<usize, SplineError> {
        let [min, max] = self.domain();
        let u = self.snap(u).ok_or(SplineError::OutsideDomain)?;
        let left = u == max || (side == KnotSide::Left && u != min);
        let end = if left {
            self.knots.partition_point(|&k| k < u)
        } else {
            self.knots.partition_point(|&k| k <= u)
        };
        Ok((end - 1).clamp(self.degree, self.controls - 1))
    }
    pub fn basis(&self, u: f64) -> Result<BasisSample, SplineError> {
        self.basis_on_side(u, KnotSide::Right)
    }
    /// Iterative Cox-de Boor recurrence and differentiated lower-degree recurrence.
    /// Repeated-knot terms with zero denominator contribute zero.
    /// The parameter, or the domain end it lies within 4 ulps of (rounding of
    /// convex combinations on a boundary); None when it is outside the domain.
    pub fn snap(&self, u: f64) -> Option<f64> {
        let [min, max] = self.domain();
        let near = |end: f64| (u - end).abs() <= 4. * f64::EPSILON * end.abs().max(max - min);
        if !u.is_finite() {
            None
        } else if u < min {
            near(min).then_some(min)
        } else if u > max {
            near(max).then_some(max)
        } else {
            Some(u)
        }
    }
    pub fn basis_on_side(&self, u: f64, side: KnotSide) -> Result<BasisSample, SplineError> {
        let u = self.snap(u).ok_or(SplineError::OutsideDomain)?;
        let span = self.span(u, side)?;
        let p = self.degree;
        let start = span - p;
        // Two buffers alternate between degrees. Degree d writes entries p - d..=p and
        // reads p - d..=p + 1 of degree d - 1; the entries it reads beyond those written
        // were never written, so they are the zeros of the full recurrence.
        let mut buffers = [[[0.; MAX_DEGREE + 2]; 3]; 2];
        buffers[0][0][p] = 1.;
        for d in 1..=p {
            let [even, odd] = &mut buffers;
            let (prev, next) = if d % 2 == 1 {
                (&*even, odd)
            } else {
                (&*odd, even)
            };
            for j in p - d..=p {
                let i = start + j;
                let l = self.knots[i + d] - self.knots[i];
                let r = self.knots[i + d + 1] - self.knots[i + 1];
                let mut value = [0.; 3];
                if l != 0. {
                    value[0] += (u - self.knots[i]) / l * prev[0][j];
                    value[1] += d as f64 * (prev[0][j] / l);
                    value[2] += d as f64 * (prev[1][j] / l);
                }
                if r != 0. {
                    value[0] += (self.knots[i + d + 1] - u) / r * prev[0][j + 1];
                    value[1] -= d as f64 * (prev[0][j + 1] / r);
                    value[2] -= d as f64 * (prev[1][j + 1] / r);
                }
                for (row, value) in next.iter_mut().zip(value) {
                    row[j] = checked(value)?;
                }
            }
        }
        let last = &buffers[p % 2];
        Ok(BasisSample {
            start,
            count: p + 1,
            rows: std::array::from_fn(|r| std::array::from_fn(|j| last[r][j])),
        })
    }
}
