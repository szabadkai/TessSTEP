//! Scan parameters of a pcurve for isoparametric-line crossings.
use tessstep_topology::Pcurve;

/// The parameters at which a pcurve is scanned for crossings of a line `k = value`:
/// the fixed scan (`base`) plus every turning point of the pcurve's `k` coordinate
/// between two scan samples, located by bisection on the sign of its derivative. An
/// edge that runs beside the line and dips across it between two samples (a
/// B-spline tangent to a seam line, 1 µm across over 0.2 % of its length) has such
/// a turning point beyond the line, so both of its crossings are found; a fixed
/// scan of any density can miss them.
pub(super) fn with_turning_points(pcurve: &Pcurve, k: usize, base: Vec<f64>) -> Vec<f64> {
    let slope = |t: f64| -> Option<f64> {
        let e = pcurve.curve.evaluate(t).ok()?;
        Some(e.first.components()[k])
    };
    let mut out = Vec::with_capacity(base.len());
    // The scan starts at the first range end, like `crossings`.
    let t0 = pcurve.range[0];
    let mut previous = slope(t0).map(|d| (t0, d));
    for t in base {
        let Some(d) = slope(t) else {
            out.push(t);
            continue;
        };
        if let Some((ta, da)) = previous {
            if da != 0. && d != 0. && da.signum() != d.signum() {
                let (mut a, mut b) = (ta, t);
                for _ in 0..60 {
                    let mid = 0.5 * (a + b);
                    match slope(mid) {
                        Some(m) if m.signum() == da.signum() => a = mid,
                        Some(_) => b = mid,
                        None => break,
                    }
                }
                let turn = 0.5 * (a + b);
                if (turn - ta) * (t - turn) > 0. {
                    out.push(turn);
                }
            }
        }
        out.push(t);
        previous = Some((t, d));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::{crossings, scan_parameters};
    use tessstep_curves::{Curve, PlaneFrame};
    use tessstep_math::{Length, NumericalTolerance, Point, Vector};
    use tessstep_topology::{CurveGeometry, Pcurve};

    #[test]
    fn crossings_between_scan_samples_are_found_at_turning_points() {
        // A unit circle whose lowest point dips 1e-6 below the line v = 0: its two
        // crossings lie 1.4e-3 rad either side of 3 pi / 2, between scan samples.
        let dip = 1e-6;
        let frame = PlaneFrame::new(
            Point::new([0., 1. - dip]).unwrap(),
            Vector::new([1., 0.]).unwrap(),
            Vector::new([0., 1.]).unwrap(),
            NumericalTolerance::default(),
        )
        .unwrap();
        let pcurve = Pcurve {
            curve: CurveGeometry::Analytic(
                Curve::circle(frame, Length::metres(1.).unwrap()).unwrap(),
            ),
            range: [std::f64::consts::PI, std::f64::consts::TAU + 0.3],
        };
        let v = |t: f64| pcurve.curve.evaluate(t).unwrap().position.coordinates()[1];
        let fixed = scan_parameters(pcurve.range);
        assert!(
            fixed.iter().all(|&t| v(t) > 0.),
            "the fixed scan alone never sees the curve below the line"
        );
        let found = crossings(&pcurve, 1, 0., 1e6);
        assert_eq!(found.len(), 2);
        let half = (2. * dip).sqrt();
        for ((t, uv), side) in found.iter().zip([-1., 1.]) {
            let expected = 1.5 * std::f64::consts::PI + side * half;
            assert!((t - expected).abs() < 1e-5, "{t} vs {expected}");
            assert!(uv[1].abs() < 1e-12);
        }
    }
}
