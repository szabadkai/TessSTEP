use tessstep_curves::{PlaneFrame, spline::SplineLimits};
use tessstep_math::{Angle, Length, ModelSpace, Point3, Vector3};
use tessstep_surfaces::{Evaluation, NurbsSurface, ParameterAxis, Surface};
fn check(v: Evaluation<ModelSpace>) {
    assert!(
        v.position
            .coordinates()
            .into_iter()
            .chain(v.du.components())
            .chain(v.dv.components())
            .chain(v.duu.components())
            .chain(v.duv.components())
            .chain(v.dvv.components())
            .all(f64::is_finite)
    );
    let _ = v.normal(Default::default());
}
pub fn exercise(data: &[u8]) {
    let mut x = [0.; 16];
    for (v, bytes) in x.iter_mut().zip(data.chunks_exact(8)) {
        *v = f64::from_le_bytes(bytes.try_into().unwrap());
    }
    let frame = PlaneFrame::new(
        Point3::<ModelSpace>::new([0.; 3]).unwrap(),
        Vector3::new([1., 0., 0.]).unwrap(),
        Vector3::new([0., 1., 0.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    let mut shapes = vec![Surface::plane(frame)];
    if let Ok(r) = Length::metres(x[0].abs()) {
        shapes.extend(Surface::sphere(frame, r));
        shapes.extend(Surface::cylinder(frame, r));
        if let Ok(b) = Length::metres(x[1].abs()) {
            shapes.extend(Surface::torus(frame, r, b));
        }
    }
    if let Ok(a) = Angle::radians(x[1]) {
        shapes.extend(Surface::cone(frame, a));
    }
    for s in shapes {
        if let Ok(v) = s.evaluate(x[2], x[3]) {
            check(v);
        }
    }
    let points: Result<Vec<_>, _> = (0..4)
        .map(|i| Point3::new([x[i * 3], x[i * 3 + 1], x[i * 3 + 2]]))
        .collect();
    let weights = x[12..].iter().map(|w| w.abs()).collect::<Vec<_>>();
    let limits = SplineLimits {
        max_controls: 32,
        max_knots: 32,
    };
    if let Ok(points) = points {
        if let Ok(s) = NurbsSurface::new(
            [1, 1],
            [&[0., 0., 1., 1.]; 2],
            [2, 2],
            &points,
            &weights,
            limits,
        ) {
            for (u, v) in [(0., 0.), (0.5, 0.5), (1., 1.), (x[0], x[1])] {
                let a = s.evaluate(u, v);
                assert_eq!(a, s.evaluate(u, v));
                if let Ok(a) = a {
                    check(a);
                }
            }
            let _ = s.insert_knot(ParameterAxis::U, 0.3, limits);
            let _ = s.insert_knot(ParameterAxis::V, x[15], limits);
        }
    }
}
