use tessstep_curves::spline::{KnotSide, NurbsCurve, SplineLimits};
use tessstep_math::{ModelSpace, Point2};
pub fn exercise(data: &[u8]) {
    let mut x = [0_f64; 16];
    for (v, bytes) in x.iter_mut().zip(data.chunks_exact(8)) {
        *v = f64::from_le_bytes(bytes.try_into().unwrap());
    }
    let limits = SplineLimits {
        max_controls: 16,
        max_knots: 32,
    };
    let points: Result<Vec<_>, _> = (0..3)
        .map(|i| Point2::<ModelSpace>::new([x[2 * i], x[2 * i + 1]]))
        .collect();
    let degree = if data.first().is_some_and(|b| b & 1 != 0) {
        data[0] as usize % 20
    } else {
        2
    };
    let knots = if data.get(1).is_some_and(|b| b & 1 != 0) {
        [x[6], x[7], x[8], x[9], x[10], x[11]]
    } else {
        [0., 0., 0., 1., 1., 1.]
    };
    if let Ok(points) = points {
        if let Ok(c) = NurbsCurve::new(
            degree,
            &knots,
            &points,
            &[x[12].abs(), x[13].abs(), x[14].abs()],
            limits,
        ) {
            for u in [0., 0.5, 1., x[15]] {
                let a = c.evaluate(u);
                assert_eq!(a, c.evaluate(u));
                if let Ok(a) = a {
                    assert!(
                        a.position
                            .coordinates()
                            .into_iter()
                            .chain(a.first.components())
                            .chain(a.second.components())
                            .all(f64::is_finite)
                    );
                }
                let _ = c.evaluate_on_side(u, KnotSide::Left);
            }
            let _ = c.insert_knot(x[15], limits);
        }
    }
}
