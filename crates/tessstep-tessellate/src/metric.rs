//! Curvature-anisotropic UV metrics for adaptive refinement (Milestone 22.2).
//!
//! A metric tensor `[m00, m01, m11]` measures a UV step `d` as `sqrt(d^T M d)`.
//! Refinement only compares lengths (the longest edge of a failing facet, the
//! shorter diagonal of a quadrilateral), so a metric's shape matters, not its scale.
//!
//! The shape comes from the surface's second derivatives in an orthonormal tangent
//! frame. A linear facet approximates a quadratic surface best, both in distance and
//! in normal direction, when it is equilateral in the absolute second fundamental
//! form |II| (principal curvatures replaced by their magnitudes): a facet that is
//! longer along a tightly curved direction than across it sags along its long edge by
//! more than its width, and its plane, hence its normal, tilts. Across a narrow torus
//! band the allowed length ratio is therefore `sqrt(k_max / k_min)`, not the ratio of
//! normal-turn rates `k_max / k_min`. The normal part is |II| scaled so its largest
//! eigenvalue is the inverse square of the step the angle or chord allows.
//!
//! Parameter lines that curve within the tangent plane (the u circles near the top of
//! a torus, polar charts near a cone apex) bend a long facet edge sideways too; such
//! a facet degenerates when that sagitta approaches its width. The tangential second
//! derivatives add a term that keeps the in-plane sagitta of a metric-unit edge below
//! a quarter of the smallest allowed step. An isotropic floor (the inverse square of
//! the face's extent) keeps every tensor positive definite, so a plane is measured by
//! arc length and a flat direction relative to the face size.
use tessstep_math::{Direction3, ModelSpace};
use tessstep_surfaces::Evaluation;

/// Symmetric positive definite 2x2 tensor `[m00, m01, m11]`.
pub(crate) type Tensor = [f64; 3];

/// The tolerances that size a metric, the isotropic floor (1 / extent^2) and the
/// ceiling (1 / model tolerance^2): no step below the model's resolution is asked
/// for, however sharply a degenerate patch curves next to a pole.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sizing {
    pub chord: f64,
    pub angle: f64,
    pub floor: f64,
    pub ceiling: f64,
}

/// Eigenvalues (descending) and the angle of the first eigenvector of a symmetric form.
fn eigen(t: Tensor) -> ([f64; 2], f64) {
    let mean = 0.5 * (t[0] + t[2]);
    let radius = (0.5 * (t[0] - t[2])).hypot(t[1]);
    (
        [mean + radius, mean - radius],
        0.5 * t[1].atan2(0.5 * (t[0] - t[2])),
    )
}
/// The symmetric form with the given eigenvalues along the angle `phi` and normal to it.
fn compose(lambda: [f64; 2], phi: f64) -> Tensor {
    let (c, s) = (phi.cos(), phi.sin());
    [
        lambda[0] * c * c + lambda[1] * s * s,
        (lambda[0] - lambda[1]) * c * s,
        lambda[0] * s * s + lambda[1] * c * c,
    ]
}
/// |A|: the same eigenvectors with absolute eigenvalues.
fn absolute(t: Tensor) -> Tensor {
    let (lambda, phi) = eigen(t);
    compose(lambda.map(f64::abs), phi)
}

/// The UV metric at a regular surface point and the square root of its largest
/// tangent-plane eigenvalue (the inverse of the smallest step it allows, in metres),
/// or `None` when the frame or curvature is not finite.
pub(crate) fn curvature(
    jet: &Evaluation<ModelSpace>,
    normal: Direction3<ModelSpace>,
    sizing: Sizing,
) -> Option<(Tensor, f64)> {
    let du = jet.du.components();
    let dv = jet.dv.components();
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    // Orthonormal tangent frame e1 = du / |du|, e2 = the rest of dv; J = [[a, along],
    // [0, b]] maps UV steps to it.
    let a = dot(du, du).sqrt();
    let along = dot(dv, du) / a;
    let rest = [0, 1, 2].map(|k| dv[k] - along * du[k] / a);
    let b = dot(rest, rest).sqrt();
    if !(a > 0. && b > 0. && a.is_finite() && b.is_finite()) {
        return None;
    }
    let e1 = du.map(|x| x / a);
    let e2 = rest.map(|x| x / b);
    let second = [jet.duu, jet.duv, jet.dvv].map(|v| v.components());
    // A UV second-derivative component in the tangent frame: J^-T X J^-1.
    let (p, q, s) = (1. / a, -along / (a * b), 1. / b);
    let frame = |axis: [f64; 3]| -> Tensor {
        let [xuu, xuv, xvv] = second.map(|x| dot(x, axis));
        [
            xuu * p * p,
            p * (xuu * q + xuv * s),
            xuu * q * q + 2. * xuv * q * s + xvv * s * s,
        ]
    };
    let (k, phi) = eigen(frame(normal.vector().components()));
    let k_max = k[0].abs().max(k[1].abs());
    // |II| scaled so the steepest direction meets the angle or the chord.
    let scale = (k_max / (sizing.angle * sizing.angle)).max(1. / (6. * sizing.chord));
    let normal_part = compose(k.map(|k| k.abs() * scale), phi);
    let finest = (k_max * scale).max(sizing.floor).sqrt();
    let [g1, g2] = [e1, e2].map(|axis| absolute(frame(axis)));
    // In-plane sagitta of a unit edge at most a quarter of the finest step.
    let bend = 0.5 * finest;
    let t = [0, 1, 2]
        .map(|i| normal_part[i] + bend * (g1[i] + g2[i]) + if i == 1 { 0. } else { sizing.floor });
    let (lambda, phi) = eigen(t);
    let t = compose(lambda.map(|l| l.min(sizing.ceiling)), phi);
    let density = lambda[0].min(sizing.ceiling).sqrt();
    // Back to UV: J^T T J.
    let tensor = [
        t[0] * a * a,
        a * (t[0] * along + t[1] * b),
        t[0] * along * along + 2. * t[1] * along * b + t[2] * b * b,
    ];
    let finite = tensor.iter().all(|v| v.is_finite()) && density.is_finite();
    (finite && tensor[0] > 0. && tensor[0] * tensor[2] > tensor[1] * tensor[1])
        .then_some((tensor, density))
}

/// Length of a UV step in a tensor.
pub(crate) fn length(t: Tensor, d: [f64; 2]) -> f64 {
    (t[0] * d[0] * d[0] + 2. * t[1] * d[0] * d[1] + t[2] * d[1] * d[1])
        .max(0.)
        .sqrt()
}

/// The mean of the given tensors, or `None` when there are none.
pub(crate) fn mean(tensors: impl IntoIterator<Item = Tensor>) -> Option<Tensor> {
    let mut sum = [0.; 3];
    let mut count = 0.;
    for t in tensors {
        for k in 0..3 {
            sum[k] += t[k];
        }
        count += 1.;
    }
    (count > 0.).then(|| sum.map(|x| x / count))
}

/// Maps UV into a chart where the tensor is Euclidean (its upper Cholesky factor),
/// preserving orientation; `None` when the tensor is not positive definite.
pub(crate) fn chart(t: Tensor) -> Option<impl Fn([f64; 2]) -> [f64; 2]> {
    let r00 = t[0].sqrt();
    let r01 = t[1] / r00;
    let r11 = (t[2] - r01 * r01).sqrt();
    (r00 > 0. && r11 > 0. && r00.is_finite() && r01.is_finite() && r11.is_finite())
        .then_some(move |p: [f64; 2]| [r00 * p[0] + r01 * p[1], r11 * p[1]])
}
