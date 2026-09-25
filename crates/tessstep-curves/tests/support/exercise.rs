use tessstep_curves::{Curve3, CurveSpan, Error, Evaluation, PlaneFrame};
use tessstep_math::{Length, ModelSpace, Point3, Vector3};
fn check(result: Result<Evaluation<ModelSpace, 3>, Error>) {
    if let Ok(v) = result {
        assert!(
            v.position
                .coordinates()
                .into_iter()
                .chain(v.first.components())
                .chain(v.second.components())
                .all(f64::is_finite)
        );
    }
}
/// Fixed-work arbitrary IEEE-754 exercise shared by stable smoke and libFuzzer.
pub fn exercise(data: &[u8]) {
    let mut values = [0.; 16];
    for (value, bytes) in values.iter_mut().zip(data.chunks_exact(8)) {
        *value = f64::from_bits(u64::from_le_bytes(bytes.try_into().unwrap()));
    }
    let origin = Point3::<ModelSpace>::new([values[0], values[1], values[2]]);
    let x = Vector3::new([values[3], values[4], values[5]]);
    let y = Vector3::new([values[6], values[7], values[8]]);
    if let (Ok(origin), Ok(x), Ok(y)) = (origin, x, y) {
        if let Ok(line) = Curve3::line(origin, x) {
            check(line.evaluate(values[11]));
        }
        if let Ok(frame) = PlaneFrame::new(origin, x, y, Default::default()) {
            let dot = frame.x().dot(frame.y()).unwrap();
            assert!(dot.abs() < 1e-12);
            assert!((frame.x().norm().unwrap() - 1.).abs() < 1e-14);
            assert!((frame.y().norm().unwrap() - 1.).abs() < 1e-14);
        }
    }
    let frame = PlaneFrame::new(
        Point3::new([0.; 3]).unwrap(),
        Vector3::new([1., 0., 0.]).unwrap(),
        Vector3::new([0., 1., 0.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    if let (Ok(a), Ok(b)) = (
        Length::metres(values[9].abs()),
        Length::metres(values[10].abs()),
    ) {
        for curve in [
            Curve3::circle(frame, a),
            Curve3::ellipse(frame, a, b),
            Curve3::parabola(frame, a),
            Curve3::hyperbola(frame, a, b),
        ]
        .into_iter()
        .flatten()
        {
            let result = curve.evaluate(values[11]);
            assert_eq!(result, curve.evaluate(values[11]));
            check(result);
            if let Ok(span) = CurveSpan::new(curve, values[12], values[13]) {
                assert_eq!(span.basis_parameter(0.).unwrap(), values[12]);
                assert_eq!(span.basis_parameter(1.).unwrap(), values[13]);
                check(span.evaluate(values[14]));
                check(span.reversed().evaluate(values[15]));
            }
        }
    }
}
