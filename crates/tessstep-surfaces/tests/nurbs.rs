use tessstep_curves::spline::{KnotSide, SplineError, SplineLimits};
use tessstep_math::{ModelSpace, Point3};
use tessstep_surfaces::*;
type S = NurbsSurface<ModelSpace>;
fn pts(p: &[[f64; 3]]) -> Vec<Point3<ModelSpace>> {
    p.iter().map(|&p| Point3::new(p).unwrap()).collect()
}
fn patch() -> S {
    S::new(
        [1, 1],
        [&[0., 0., 1., 1.]; 2],
        [2, 2],
        &pts(&[[0., 0., 0.], [0., 1., 0.], [1., 0., 0.], [1., 1., 1.]]),
        &[1., 2., 3., 4.],
        Default::default(),
    )
    .unwrap()
}
fn near(a: [f64; 3], b: [f64; 3], tol: f64) {
    for i in 0..3 {
        assert!(
            (a[i] - b[i]).abs() <= tol * b[i].abs().max(1.),
            "{a:?} != {b:?}"
        );
    }
}
fn compare(a: Evaluation<ModelSpace>, b: Evaluation<ModelSpace>, tol: f64) {
    near(a.position.coordinates(), b.position.coordinates(), tol);
    for (x, y) in [
        (a.du, b.du),
        (a.dv, b.dv),
        (a.duu, b.duu),
        (a.duv, b.duv),
        (a.dvv, b.dvv),
    ] {
        near(x.components(), y.components(), tol);
    }
}
#[test]
fn nurbs_surface_known_rational_partials_and_tensor_order() {
    let a = patch().evaluate(0.5, 0.5).unwrap();
    near(a.position.coordinates(), [0.7, 0.6, 0.4], 1e-14);
    near(a.du.components(), [0.84, -0.08, 0.48], 1e-14);
    near(a.dv.components(), [-0.08, 0.96, 0.64], 1e-14);
    near(a.duu.components(), [-1.344, 0.128, -0.768], 1e-14);
    near(a.duv.components(), [0.128, 0.064, 0.896], 1e-14);
    near(a.dvv.components(), [0.064, -0.768, -0.512], 1e-14);
    let s = patch();
    for (uv, p) in [
        ([0., 0.], [0., 0., 0.]),
        ([0., 1.], [0., 1., 0.]),
        ([1., 0.], [1., 0., 0.]),
        ([1., 1.], [1., 1., 1.]),
    ] {
        near(
            s.evaluate(uv[0], uv[1]).unwrap().position.coordinates(),
            p,
            1e-14,
        );
    }
    let h = 1e-5;
    for u in [0.1, 0.4, 0.8] {
        for v in [0.2, 0.6, 0.9] {
            let a = s.evaluate(u, v).unwrap();
            let up = s.evaluate(u + h, v).unwrap();
            let um = s.evaluate(u - h, v).unwrap();
            let vp = s.evaluate(u, v + h).unwrap();
            let vm = s.evaluate(u, v - h).unwrap();
            let diff = |a: [f64; 3], b: [f64; 3]| std::array::from_fn(|i| (a[i] - b[i]) / (2. * h));
            near(
                a.du.components(),
                diff(up.position.coordinates(), um.position.coordinates()),
                1e-8,
            );
            near(
                a.dv.components(),
                diff(vp.position.coordinates(), vm.position.coordinates()),
                1e-8,
            );
            near(
                a.duu.components(),
                diff(up.du.components(), um.du.components()),
                1e-8,
            );
            near(
                a.duv.components(),
                diff(up.dv.components(), um.dv.components()),
                1e-8,
            );
            near(
                a.duv.components(),
                diff(vp.du.components(), vm.du.components()),
                1e-8,
            );
            near(
                a.dvv.components(),
                diff(vp.dv.components(), vm.dv.components()),
                1e-8,
            );
        }
    }
}
#[test]
fn nurbs_surface_refinement_preserves_global_weights() {
    let a = patch();
    let b = a
        .insert_knot(ParameterAxis::U, 0.3, Default::default())
        .unwrap()
        .insert_knot(ParameterAxis::V, 0.7, Default::default())
        .unwrap();
    assert_eq!(b.shape(), [3, 3]);
    assert_eq!(a.shape(), [2, 2]);
    for i in 0..=10 {
        for j in 0..=10 {
            let (u, v) = (f64::from(i) / 10., f64::from(j) / 10.);
            compare(a.evaluate(u, v).unwrap(), b.evaluate(u, v).unwrap(), 1e-12);
        }
    }
    assert_eq!(
        b.insert_knot(ParameterAxis::U, 0.3, Default::default()),
        Err(Error::Spline(SplineError::InsertionLimit))
    );
    assert_eq!(
        a.insert_knot(
            ParameterAxis::V,
            0.5,
            SplineLimits {
                max_controls: 5,
                ..Default::default()
            }
        ),
        Err(Error::Spline(SplineError::ResourceLimit))
    );
}
#[test]
fn nurbs_surface_rational_cylinder_periodic_net_and_singular_normal() {
    let q = std::f64::consts::FRAC_1_SQRT_2;
    let s = S::new(
        [2, 1],
        [&[0., 0., 0., 1., 1., 1.], &[0., 0., 1., 1.]],
        [3, 2],
        &pts(&[
            [1., 0., 0.],
            [1., 0., 2.],
            [1., 1., 0.],
            [1., 1., 2.],
            [0., 1., 0.],
            [0., 1., 2.],
        ]),
        &[1., 1., q, q, 1., 1.],
        Default::default(),
    )
    .unwrap();
    for i in 0..=10 {
        let v = s.evaluate(f64::from(i) / 10., 0.4).unwrap();
        let p = v.position.coordinates();
        assert!((p[0] * p[0] + p[1] * p[1] - 1.).abs() < 1e-13);
        near(
            v.normal(Default::default()).unwrap().vector().components(),
            [p[0], p[1], 0.],
            1e-13,
        );
        near(v.dv.components(), [0., 0., 2.], 1e-13);
    }
    let mut controls = Vec::new();
    for xy in [[1., 0.], [0., 1.], [-1., 0.], [0., -1.], [1., 0.], [0., 1.]] {
        for z in [0., 1.] {
            controls.push(Point3::new([xy[0], xy[1], z]).unwrap());
        }
    }
    let p = S::new(
        [2, 1],
        [&[0., 1., 2., 3., 4., 5., 6., 7., 8.], &[0., 0., 1., 1.]],
        [6, 2],
        &controls,
        &[1.; 12],
        Default::default(),
    )
    .unwrap();
    let a = p.evaluate(2., 0.3).unwrap();
    let b = p.evaluate(6., 0.3).unwrap();
    near(a.position.coordinates(), b.position.coordinates(), 1e-14);
    near(a.du.components(), b.du.components(), 1e-14);
    let c = S::new(
        [1, 1],
        [&[0., 0., 1., 1.]; 2],
        [2, 2],
        &pts(&[[0.; 3]; 4]),
        &[1.; 4],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        c.evaluate(0.5, 0.5).unwrap().normal(Default::default()),
        Err(Error::SingularNormal)
    );
    assert!(s.evaluate_on_sides(0., 0., [KnotSide::Left; 2]).is_ok());
}
#[test]
fn nurbs_surface_rejects_layout_domain_and_limits() {
    let a = patch();
    let k = [&[0., 0., 1., 1.][..]; 2];
    let limits = Default::default();
    assert_eq!(
        S::new([1, 1], k, [2, 3], a.controls(), a.weights(), limits),
        Err(Error::Spline(SplineError::InvalidControls))
    );
    assert_eq!(
        S::new(
            [1, 1],
            k,
            [usize::MAX, 2],
            a.controls(),
            a.weights(),
            limits
        ),
        Err(Error::Spline(SplineError::ResourceLimit))
    );
    assert_eq!(
        S::new([1, 1], k, [2, 2], a.controls(), &[0.; 4], limits),
        Err(Error::Spline(SplineError::InvalidWeights))
    );
    assert_eq!(
        S::new(
            [1, 1],
            k,
            [2, 2],
            a.controls(),
            a.weights(),
            SplineLimits {
                max_knots: 7,
                ..limits
            }
        ),
        Err(Error::Spline(SplineError::ResourceLimit))
    );
    for bad in [-1., 2., f64::NAN, f64::INFINITY] {
        assert!(a.evaluate(bad, 0.5).is_err());
        assert!(a.evaluate(0.5, bad).is_err());
    }
}

#[test]
fn nurbs_surface_affine_covariance_and_one_sided_partials() {
    let a = patch();
    let transform = tessstep_math::Affine3::<ModelSpace, ModelSpace>::new(
        [[2., 1., 0.], [0., 3., 0.], [0., 0., -1.]],
        [4., 5., 6.],
    )
    .unwrap();
    let controls: Vec<_> = a
        .controls()
        .iter()
        .map(|&p| transform.transform_point(p).unwrap())
        .collect();
    let k = a.knot_vectors();
    let b = S::new(
        [1, 1],
        [k[0].knots(), k[1].knots()],
        [2, 2],
        &controls,
        a.weights(),
        Default::default(),
    )
    .unwrap();
    for u in [0., 0.3, 1.] {
        for v in [0., 0.7, 1.] {
            compare(
                a.evaluate(u, v).unwrap().transformed(transform).unwrap(),
                b.evaluate(u, v).unwrap(),
                1e-12,
            );
        }
    }
    let s = S::new(
        [1, 1],
        [&[0., 0., 0.5, 1., 1.], &[0., 0., 1., 1.]],
        [3, 2],
        &pts(&[
            [0., 0., 0.],
            [0., 1., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [1., 0., 2.],
            [1., 1., 2.],
        ]),
        &[1.; 6],
        Default::default(),
    )
    .unwrap();
    near(
        s.evaluate_on_sides(0.5, 0.3, [KnotSide::Left, KnotSide::Right])
            .unwrap()
            .du
            .components(),
        [2., 0., 0.],
        1e-14,
    );
    near(
        s.evaluate(0.5, 0.3).unwrap().du.components(),
        [0., 0., 4.],
        1e-14,
    );
}
