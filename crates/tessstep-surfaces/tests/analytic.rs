use tessstep_curves::PlaneFrame;
use tessstep_math::{Angle, Length, ModelSpace, Point3, Vector3};
use tessstep_surfaces::*;
fn frame() -> PlaneFrame<ModelSpace, 3> {
    PlaneFrame::new(
        Point3::new([0.; 3]).unwrap(),
        Vector3::new([1., 0., 0.]).unwrap(),
        Vector3::new([0., 1., 0.]).unwrap(),
        Default::default(),
    )
    .unwrap()
}
fn l(x: f64) -> Length {
    Length::metres(x).unwrap()
}
fn surfaces() -> [Surface<ModelSpace>; 5] {
    [
        Surface::plane(frame()),
        Surface::cylinder(frame(), l(2.)).unwrap(),
        Surface::cone(frame(), Angle::radians(0.5).unwrap()).unwrap(),
        Surface::sphere(frame(), l(3.)).unwrap(),
        Surface::torus(frame(), l(4.), l(1.)).unwrap(),
    ]
}
fn near(a: [f64; 3], b: [f64; 3], tol: f64) {
    for i in 0..3 {
        assert!(
            (a[i] - b[i]).abs() < tol * b[i].abs().max(1.),
            "{a:?} != {b:?}"
        );
    }
}
#[test]
fn analytic_surfaces_known_values_and_singularities() {
    let [p, c, k, s, t] = surfaces();
    assert_eq!(
        p.evaluate(2., 3.).unwrap().position.coordinates(),
        [2., 3., 0.]
    );
    assert_eq!(
        c.evaluate(0., 3.).unwrap().position.coordinates(),
        [2., 0., 3.]
    );
    assert_eq!(
        s.evaluate(0., 0.).unwrap().position.coordinates(),
        [3., 0., 0.]
    );
    assert_eq!(
        t.evaluate(0., 0.).unwrap().position.coordinates(),
        [5., 0., 0.]
    );
    near(
        k.evaluate(0., 2.).unwrap().position.coordinates(),
        [2. * 0.5_f64.sin(), 0., 2. * 0.5_f64.cos()],
        1e-14,
    );
    for pole in [-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2] {
        let v = s.evaluate(0.7, pole).unwrap();
        assert_eq!(v.du.components(), [0.; 3]);
        assert_eq!(v.normal(Default::default()), Err(Error::SingularNormal));
    }
    assert_eq!(
        k.evaluate(1., 0.).unwrap().normal(Default::default()),
        Err(Error::SingularNormal)
    );
    near(
        c.evaluate(0., 2.)
            .unwrap()
            .normal(Default::default())
            .unwrap()
            .vector()
            .components(),
        [1., 0., 0.],
        1e-14,
    );
    assert_eq!(s.evaluate(0., 2.), Err(Error::ParameterOutsideDomain));
    assert_eq!(k.evaluate(0., -1.), Err(Error::ParameterOutsideDomain));
    // Rounding within 4 ulps of a closed end evaluates at the end; farther does not.
    let pole = std::f64::consts::FRAC_PI_2;
    let above = f64::from_bits(pole.to_bits() + 2);
    assert_eq!(
        s.evaluate(0.3, above).unwrap(),
        s.evaluate(0.3, pole).unwrap()
    );
    assert_eq!(
        s.evaluate(0.3, pole + 1e-12),
        Err(Error::ParameterOutsideDomain)
    );
    assert_eq!(
        k.evaluate(0.3, -1e-300).unwrap(),
        k.evaluate(0.3, 0.).unwrap()
    );
    assert_eq!(k.evaluate(0.3, -1e-12), Err(Error::ParameterOutsideDomain));
}
#[test]
fn analytic_surface_partials_match_finite_differences() {
    let h = 1e-5;
    for surface in surfaces() {
        for u in [-1., 0.4, 3.] {
            for v in [0.2, 0.7] {
                let a = surface.evaluate(u, v).unwrap();
                let up = surface.evaluate(u + h, v).unwrap();
                let um = surface.evaluate(u - h, v).unwrap();
                let vp = surface.evaluate(u, v + h).unwrap();
                let vm = surface.evaluate(u, v - h).unwrap();
                let diff =
                    |a: [f64; 3], b: [f64; 3]| std::array::from_fn(|i| (a[i] - b[i]) / (2. * h));
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
                    diff(vp.du.components(), vm.du.components()),
                    1e-8,
                );
                near(
                    a.duv.components(),
                    diff(up.dv.components(), um.dv.components()),
                    1e-8,
                );
                near(
                    a.dvv.components(),
                    diff(vp.dv.components(), vm.dv.components()),
                    1e-8,
                );
                let n = a.normal(Default::default()).unwrap().vector();
                assert!(n.dot(a.du).unwrap().abs() < 1e-12);
                assert!(n.dot(a.dv).unwrap().abs() < 1e-12);
            }
        }
    }
}
#[test]
fn analytic_surface_periods_loci_and_invalid_shapes() {
    for surface in surfaces() {
        for bad in [f64::NAN, f64::INFINITY] {
            assert!(surface.evaluate(bad, 0.).is_err());
            assert!(surface.evaluate(0., bad).is_err());
        }
        if surface.kind() != SurfaceKind::Plane {
            near(
                surface.evaluate(0.3, 0.4).unwrap().position.coordinates(),
                surface
                    .evaluate(0.3 + std::f64::consts::TAU, 0.4)
                    .unwrap()
                    .position
                    .coordinates(),
                1e-13,
            );
        }
    }
    assert!(Surface::sphere(frame(), l(0.)).is_err());
    assert!(Surface::torus(frame(), l(1.), l(1.)).is_err());
    assert!(Surface::cone(frame(), Angle::radians(0.).unwrap()).is_err());
    let s = surfaces()[3]
        .evaluate(0.2, 0.5)
        .unwrap()
        .position
        .coordinates();
    assert!((s.iter().map(|x| x * x).sum::<f64>() - 9.).abs() < 1e-12);
    let t = surfaces()[4];
    near(
        t.evaluate(0.3, 0.4).unwrap().position.coordinates(),
        t.evaluate(0.3, 0.4 + std::f64::consts::TAU)
            .unwrap()
            .position
            .coordinates(),
        1e-13,
    );
}
#[test]
fn analytic_spindle_torus_parts_end_at_the_axis_points() {
    use std::f64::consts::{PI, TAU};
    // a = 3, b = 5: the tube crosses the axis at height ±4 where cos v = -3/5.
    let apple = Surface::degenerate_torus(frame(), l(3.), l(5.), true).unwrap();
    let lemon = Surface::degenerate_torus(frame(), l(3.), l(5.), false).unwrap();
    assert_eq!(apple.kind(), SurfaceKind::SpindleTorus);
    let star = (-3.0f64 / 5.).acos();
    assert_eq!(
        apple.domain()[1],
        AxisDomain::Closed {
            min: -star,
            max: star
        }
    );
    assert_eq!(
        lemon.domain()[1],
        AxisDomain::Closed {
            min: star,
            max: TAU - star
        }
    );
    // The outer equator and the lemon's waist.
    near(
        apple.evaluate(0., 0.).unwrap().position.coordinates(),
        [8., 0., 0.],
        1e-14,
    );
    near(
        lemon.evaluate(0., PI).unwrap().position.coordinates(),
        [-2., 0., 0.],
        1e-14,
    );
    // Both parts share the axis points, where du vanishes exactly. du x dv points
    // away from the axis on the apple and towards it on the lemon.
    for (surface, ends, heights, sign) in [
        (apple, [-star, star], [-4., 4.], 1.),
        (lemon, [star, TAU - star], [4., -4.], -1.),
    ] {
        for (v, z) in ends.into_iter().zip(heights) {
            let e = surface.evaluate(1., v).unwrap();
            near(e.position.coordinates(), [0., 0., z], 1e-14);
            assert_eq!(e.du.components(), [0., 0., 0.]);
            assert!(e.normal(Default::default()).is_err());
        }
        let mid = 0.5 * (ends[0] + ends[1]);
        let e = surface.evaluate(0.3, mid).unwrap();
        let n = e.normal(Default::default()).unwrap().vector().components();
        let p = e.position.coordinates();
        assert!(sign * (n[0] * p[0] + n[1] * p[1]) > 0., "{n:?} at {p:?}");
        assert!(surface.evaluate(0., ends[0] - 1e-3).is_err());
        assert!(surface.evaluate(0., ends[1] + 1e-3).is_err());
    }
    assert!(Surface::degenerate_torus(frame(), l(5.), l(3.), true).is_err());
    assert!(Surface::degenerate_torus(frame(), l(5.), l(5.), true).is_err());
}
