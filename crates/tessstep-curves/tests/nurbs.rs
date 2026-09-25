use tessstep_curves::spline::*;
use tessstep_math::{ModelSpace, Point2};
type C = NurbsCurve<ModelSpace, 2>;
fn points(v: &[[f64; 2]]) -> Vec<Point2<ModelSpace>> {
    v.iter().map(|&p| Point2::new(p).unwrap()).collect()
}
fn near(a: [f64; 2], b: [f64; 2], t: f64) {
    for i in 0..2 {
        assert!(
            (a[i] - b[i]).abs() <= t * b[i].abs().max(1.),
            "{a:?} != {b:?}"
        );
    }
}
fn quarter() -> C {
    C::new(
        2,
        &[0., 0., 0., 1., 1., 1.],
        &points(&[[1., 0.], [1., 1.], [0., 1.]]),
        &[1., std::f64::consts::FRAC_1_SQRT_2, 1.],
        Default::default(),
    )
    .unwrap()
}
#[test]
fn nurbs_curve_rational_circle_and_derivatives() {
    let c = quarter();
    let mid = c.evaluate(0.5).unwrap();
    near(
        mid.position.coordinates(),
        [std::f64::consts::FRAC_1_SQRT_2; 2],
        1e-14,
    );
    near(
        c.evaluate(0.).unwrap().position.coordinates(),
        [1., 0.],
        1e-14,
    );
    near(
        c.evaluate(1.).unwrap().position.coordinates(),
        [0., 1.],
        1e-14,
    );
    for i in 1..100 {
        let u = f64::from(i) / 100.;
        let v = c.evaluate(u).unwrap();
        let [x, y] = v.position.coordinates();
        assert!((x * x + y * y - 1.).abs() < 1e-13);
        let h = 1e-5;
        let a = c.evaluate(u - h).unwrap();
        let b = c.evaluate(u + h).unwrap();
        near(
            v.first.components(),
            std::array::from_fn(|j| {
                (b.position.coordinates()[j] - a.position.coordinates()[j]) / (2. * h)
            }),
            1e-8,
        );
        near(
            v.second.components(),
            std::array::from_fn(|j| (b.first.components()[j] - a.first.components()[j]) / (2. * h)),
            1e-8,
        );
    }
}
#[test]
fn spline_basis_partition_repeated_knots_and_sides() {
    let k = KnotVector::new(
        2,
        5,
        &[0., 0., 0., 0.5, 0.5, 1., 1., 1.],
        Default::default(),
    )
    .unwrap();
    for u in [0., 0.1, 0.5, 0.9, 1.] {
        let b = k.basis(u).unwrap();
        assert!((b.values().iter().sum::<f64>() - 1.).abs() < 1e-14);
        assert!(b.first().iter().sum::<f64>().abs() < 1e-13);
        assert!(b.second().iter().sum::<f64>().abs() < 1e-12);
        assert!(b.values().iter().all(|x| *x >= 0.));
    }
    let c = C::new(
        1,
        &[0., 0., 0.5, 1., 1.],
        &points(&[[0., 0.], [1., 0.], [1., 2.]]),
        &[1.; 3],
        Default::default(),
    )
    .unwrap();
    near(
        c.evaluate_on_side(0.5, KnotSide::Left)
            .unwrap()
            .first
            .components(),
        [2., 0.],
        1e-14,
    );
    near(c.evaluate(0.5).unwrap().first.components(), [0., 4.], 1e-14);
    let discontinuous = C::new(
        1,
        &[0., 0., 0.5, 0.5, 1., 1.],
        &points(&[[0., 0.], [1., 0.], [3., 0.], [4., 0.]]),
        &[1.; 4],
        Default::default(),
    )
    .unwrap();
    near(
        discontinuous
            .evaluate_on_side(0.5, KnotSide::Left)
            .unwrap()
            .position
            .coordinates(),
        [1., 0.],
        1e-14,
    );
    near(
        discontinuous.evaluate(0.5).unwrap().position.coordinates(),
        [3., 0.],
        1e-14,
    );
    // Unclamped uniform periodic representation; duplicate the first p controls.
    let periodic = C::new(
        2,
        &[0., 1., 2., 3., 4., 5., 6., 7., 8.],
        &points(&[[1., 0.], [0., 1.], [-1., 0.], [0., -1.], [1., 0.], [0., 1.]]),
        &[1.; 6],
        Default::default(),
    )
    .unwrap();
    let a = periodic.evaluate(2.).unwrap();
    let b = periodic.evaluate(6.).unwrap();
    near(a.position.coordinates(), b.position.coordinates(), 1e-14);
    near(a.first.components(), b.first.components(), 1e-14);
    assert_eq!(periodic.knot_vector().domain(), [2., 6.]);
    assert!(periodic.evaluate(6.1).is_err());
}
#[test]
fn nurbs_knot_insertion_preserves_rational_geometry_and_derivatives() {
    let c = quarter();
    let d = c
        .insert_knot(0.3, Default::default())
        .unwrap()
        .insert_knot(0.3, Default::default())
        .unwrap()
        .insert_knot(0.7, Default::default())
        .unwrap();
    for i in 0..=100 {
        let u = f64::from(i) / 100.;
        let a = c.evaluate(u).unwrap();
        let b = d.evaluate(u).unwrap();
        near(a.position.coordinates(), b.position.coordinates(), 1e-13);
        near(a.first.components(), b.first.components(), 1e-12);
        near(a.second.components(), b.second.components(), 1e-11);
    }
    assert_eq!(
        d.insert_knot(0.3, Default::default()),
        Err(SplineError::InsertionLimit)
    );
    assert_eq!(c.controls().len(), 3);
    let scaled = C::new(
        2,
        c.knot_vector().knots(),
        c.controls(),
        &[1e200, std::f64::consts::FRAC_1_SQRT_2 * 1e200, 1e200],
        Default::default(),
    )
    .unwrap();
    near(
        c.evaluate(0.4).unwrap().position.coordinates(),
        scaled.evaluate(0.4).unwrap().position.coordinates(),
        1e-14,
    );
}
#[test]
fn spline_limits_and_malformed_inputs_are_rejected() {
    let p = points(&[[0., 0.], [1., 1.]]);
    let k = [0., 0., 1., 1.];
    let limits = Default::default();
    assert_eq!(
        C::new(0, &k, &p, &[1.; 2], limits),
        Err(SplineError::InvalidDegree)
    );
    for knots in [
        vec![0., 0., 0., 0.],
        vec![0., 1., 0., 1.],
        vec![0., 0., f64::NAN, 1.],
        vec![0., 0., 1.],
    ] {
        assert!(C::new(1, &knots, &p, &[1.; 2], limits).is_err());
    }
    for w in [
        [0., 1.],
        [-1., 1.],
        [f64::NAN, 1.],
        [f64::from_bits(1), f64::MAX],
    ] {
        assert_eq!(
            C::new(1, &k, &p, &w, limits),
            Err(SplineError::InvalidWeights)
        );
    }
    assert_eq!(
        C::new(
            1,
            &k,
            &p,
            &[1.; 2],
            SplineLimits {
                max_controls: 1,
                ..limits
            }
        ),
        Err(SplineError::ResourceLimit)
    );
    assert_eq!(
        C::new(
            1,
            &k,
            &p,
            &[1.; 2],
            SplineLimits {
                max_knots: 3,
                ..limits
            }
        ),
        Err(SplineError::ResourceLimit)
    );
    let c = C::new(1, &k, &p, &[1.; 2], limits).unwrap();
    for u in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert_eq!(c.evaluate(u), Err(SplineError::OutsideDomain));
    }
    assert_eq!(
        c.insert_knot(
            0.5,
            SplineLimits {
                max_controls: 2,
                ..limits
            }
        ),
        Err(SplineError::ResourceLimit)
    );
    let tiny = C::new(
        1,
        &[0., 0., f64::from_bits(1), f64::from_bits(1)],
        &p,
        &[1.; 2],
        limits,
    )
    .unwrap();
    assert_eq!(tiny.evaluate(0.), Err(SplineError::NumericRange));
}

#[test]
fn spline_max_degree_and_affine_covariance() {
    let mut knots = vec![0.; MAX_DEGREE + 1];
    knots.extend(vec![1.; MAX_DEGREE + 1]);
    let control: Vec<_> = (0..=MAX_DEGREE)
        .map(|i| Point2::<ModelSpace>::new([i as f64, 2. * i as f64]).unwrap())
        .collect();
    let c = C::new(
        MAX_DEGREE,
        &knots,
        &control,
        &[1.; MAX_DEGREE + 1],
        Default::default(),
    )
    .unwrap();
    for u in [0., 0.1, 0.5, 0.9, 1.] {
        let a = c.evaluate(u).unwrap();
        near(a.position.coordinates(), [16. * u, 32. * u], 1e-13);
        near(a.first.components(), [16., 32.], 1e-12);
        near(a.second.components(), [0., 0.], 1e-10);
    }
    let c = quarter();
    let transformed: Vec<_> = c
        .controls()
        .iter()
        .map(|p| {
            let [x, y] = p.coordinates();
            Point2::new([2. * x + y + 3., 3. * y - 1.]).unwrap()
        })
        .collect();
    let t = C::new(
        2,
        c.knot_vector().knots(),
        &transformed,
        c.weights(),
        Default::default(),
    )
    .unwrap();
    for u in [0., 0.2, 0.7, 1.] {
        let a = c.evaluate(u).unwrap();
        let b = t.evaluate(u).unwrap();
        let [x, y] = a.position.coordinates();
        near(
            b.position.coordinates(),
            [2. * x + y + 3., 3. * y - 1.],
            1e-13,
        );
        let [x, y] = a.first.components();
        near(b.first.components(), [2. * x + y, 3. * y], 1e-12);
    }
}
