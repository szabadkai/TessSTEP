use std::f64::consts::{FRAC_PI_2, PI, TAU};
use tessstep_curves::*;
use tessstep_math::{
    Affine3, Length, ModelSpace, NumericalTolerance, Point2, Point3, Vector2, Vector3,
};
type C = Curve2<ModelSpace>;
fn length(v: f64) -> Length {
    Length::metres(v).unwrap()
}
fn frame() -> PlaneFrame<ModelSpace, 2> {
    PlaneFrame::new(
        Point2::new([0., 0.]).unwrap(),
        Vector2::new([1., 0.]).unwrap(),
        Vector2::new([0., 1.]).unwrap(),
        Default::default(),
    )
    .unwrap()
}
fn curves() -> [C; 5] {
    [
        C::line(
            Point2::new([1., 2.]).unwrap(),
            Vector2::new([3., 4.]).unwrap(),
        )
        .unwrap(),
        C::circle(frame(), length(2.)).unwrap(),
        C::ellipse(frame(), length(3.), length(2.)).unwrap(),
        C::parabola(frame(), length(2.)).unwrap(),
        C::hyperbola(frame(), length(3.), length(2.)).unwrap(),
    ]
}
fn near<const N: usize>(a: [f64; N], b: [f64; N], tol: f64) {
    for i in 0..N {
        assert!(
            (a[i] - b[i]).abs() <= tol * b[i].abs().max(1.),
            "{a:?} != {b:?}"
        );
    }
}

#[test]
fn analytic_curves_have_known_positions_derivatives_and_domains() {
    let [line, circle, ellipse, parabola, hyperbola] = curves();
    let line = line.evaluate(2.).unwrap();
    assert_eq!(line.position.coordinates(), [7., 10.]);
    assert_eq!(line.first.components(), [3., 4.]);
    assert_eq!(line.second.components(), [0., 0.]);
    let c = circle.evaluate(FRAC_PI_2).unwrap();
    near(c.position.coordinates(), [0., 2.], 1e-14);
    near(c.first.components(), [-2., 0.], 1e-14);
    near(c.second.components(), [0., -2.], 1e-14);
    let e = ellipse.evaluate(0.).unwrap();
    assert_eq!(e.position.coordinates(), [3., 0.]);
    assert_eq!(e.first.components(), [0., 2.]);
    assert_eq!(e.second.components(), [-3., 0.]);
    let p = parabola.evaluate(-2.).unwrap();
    assert_eq!(p.position.coordinates(), [8., -8.]);
    assert_eq!(p.first.components(), [-8., 4.]);
    assert_eq!(p.second.components(), [4., 0.]);
    // cosh(ln 2)=5/4 and sinh(ln 2)=3/4, independent reference values.
    let h = hyperbola.evaluate(2_f64.ln()).unwrap();
    near(h.position.coordinates(), [3.75, 1.5], 1e-14);
    near(h.first.components(), [2.25, 2.5], 1e-14);
    near(h.second.components(), [3.75, 1.5], 1e-14);
    for (c, kind) in curves().into_iter().zip([
        CurveKind::Line,
        CurveKind::Circle,
        CurveKind::Ellipse,
        CurveKind::Parabola,
        CurveKind::Hyperbola,
    ]) {
        assert_eq!(c.kind(), kind);
        assert_eq!(
            c.domain(),
            if matches!(kind, CurveKind::Circle | CurveKind::Ellipse) {
                Domain::Periodic { period: TAU }
            } else {
                Domain::Unbounded
            }
        );
    }
}

#[test]
fn analytic_derivatives_match_finite_differences_and_conic_loci() {
    let h = 1e-5;
    for c in curves() {
        for i in -30..=30 {
            let u = f64::from(i) / 10.;
            let v = c.evaluate(u).unwrap();
            let left = c.evaluate(u - h).unwrap();
            let right = c.evaluate(u + h).unwrap();
            let fd1 = std::array::from_fn(|j| {
                (right.position.coordinates()[j] - left.position.coordinates()[j]) / (2. * h)
            });
            let fd2 = std::array::from_fn(|j| {
                (right.first.components()[j] - left.first.components()[j]) / (2. * h)
            });
            near(v.first.components(), fd1, 2e-9);
            near(v.second.components(), fd2, 2e-9);
            let [x, y] = v.position.coordinates();
            let residual = match c.kind() {
                CurveKind::Line => 4. * (x - 1.) - 3. * (y - 2.),
                CurveKind::Circle => x * x + y * y - 4.,
                CurveKind::Ellipse => x * x / 9. + y * y / 4. - 1.,
                CurveKind::Parabola => y * y - 8. * x,
                CurveKind::Hyperbola => x * x / 9. - y * y / 4. - 1.,
            };
            assert!(residual.abs() < 1e-11, "{c:?}: {residual}");
        }
    }
}

#[test]
fn analytic_frames_preserve_2d_orientation_and_3d_plane() {
    let f = PlaneFrame::new(
        Point2::<ModelSpace>::new([10., 20.]).unwrap(),
        Vector2::new([0., 2.]).unwrap(),
        Vector2::new([3., 1.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    near(f.x().components(), [0., 1.], 1e-14);
    near(f.y().components(), [1., 0.], 1e-14);
    let c = C::ellipse(f, length(3.), length(2.)).unwrap();
    near(
        c.evaluate(0.).unwrap().position.coordinates(),
        [10., 23.],
        1e-14,
    );
    near(
        c.evaluate(FRAC_PI_2).unwrap().position.coordinates(),
        [12., 20.],
        1e-14,
    );
    let origin = Point3::<ModelSpace>::new([2., 3., 4.]).unwrap();
    let frame = PlaneFrame::new(
        origin,
        Vector3::new([1., 1., 0.]).unwrap(),
        Vector3::new([1., 0., 1.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    let normal = frame.x().cross(frame.y()).unwrap();
    let c = Curve3::circle(frame, length(7.)).unwrap();
    for i in 0..100 {
        let v = c.evaluate(f64::from(i) / 10.).unwrap();
        assert!(
            normal
                .dot(v.position.difference(origin).unwrap())
                .unwrap()
                .abs()
                < 1e-13
        );
        assert!(normal.dot(v.first).unwrap().abs() < 1e-13);
        near([v.position.distance(origin).unwrap()], [7.], 1e-14);
    }
    for scale in [f64::from_bits(1), 1e-300, 1e300, f64::MAX] {
        let f = PlaneFrame::new(
            origin,
            Vector3::new([scale, 0., 0.]).unwrap(),
            Vector3::new([scale, scale, 0.]).unwrap(),
            Default::default(),
        )
        .unwrap();
        near(f.x().components(), [1., 0., 0.], 1e-14);
        near(f.y().components(), [0., 1., 0.], 1e-14);
    }
}

#[test]
fn analytic_spans_keep_orientation_seams_and_chain_rule() {
    let c = curves()[1];
    let span = CurveSpan::new(c, 1.5 * PI, 2.5 * PI).unwrap();
    assert_eq!(span.endpoints(), [1.5 * PI, 2.5 * PI]);
    assert_eq!(span.basis(), c);
    near(
        span.evaluate(0.5).unwrap().position.coordinates(),
        [2., 0.],
        1e-14,
    );
    near(
        span.evaluate(0.5).unwrap().first.components(),
        [0., 2. * PI],
        1e-14,
    );
    near(
        span.evaluate(0.5).unwrap().second.components(),
        [-2. * PI * PI, 0.],
        1e-13,
    );
    for i in 0..=10 {
        let s = f64::from(i) / 10.;
        let forward = span.evaluate(s).unwrap();
        let reverse = span.reversed().evaluate(1. - s).unwrap();
        near(
            forward.position.coordinates(),
            reverse.position.coordinates(),
            1e-13,
        );
        near(
            forward.first.components(),
            reverse.first.components().map(|x| -x),
            1e-13,
        );
        near(
            forward.second.components(),
            reverse.second.components(),
            1e-13,
        );
    }
    let full = CurveSpan::new(c, 0., 2. * TAU).unwrap();
    assert_eq!(full.basis_parameter(1.), Ok(2. * TAU));
    near(
        full.evaluate(0.5).unwrap().position.coordinates(),
        full.evaluate(1.).unwrap().position.coordinates(),
        1e-14,
    );
    let endpoints = CurveSpan::new(curves()[0], 1e16, 1.).unwrap();
    assert_eq!(endpoints.basis_parameter(0.), Ok(1e16));
    assert_eq!(endpoints.basis_parameter(1.), Ok(1.));
    let line = CurveSpan::new(curves()[0], 3., -2.).unwrap();
    assert_eq!(line.evaluate(0.5).unwrap().first.components(), [-15., -20.]);
    assert_eq!(line.evaluate(0.5).unwrap().second.components(), [0., 0.]);
}

#[test]
fn analytic_periodicity_and_affine_derivatives() {
    for c in [curves()[1], curves()[2]] {
        for u in [-3., -0.2, 0., 2.5] {
            let a = c.evaluate(u).unwrap();
            let b = c.evaluate(u + TAU).unwrap();
            near(a.position.coordinates(), b.position.coordinates(), 1e-14);
            near(a.first.components(), b.first.components(), 1e-14);
            near(a.second.components(), b.second.components(), 1e-14);
        }
    }
    let f = PlaneFrame::new(
        Point3::<ModelSpace>::new([1., 2., 3.]).unwrap(),
        Vector3::new([1., 0., 0.]).unwrap(),
        Vector3::new([0., 1., 0.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    let v = Curve3::circle(f, length(2.)).unwrap().evaluate(0.).unwrap();
    let t = Affine3::new([[2., 1., 0.], [0., 3., 0.], [0., 0., -1.]], [10., 20., 30.]).unwrap();
    let v: Evaluation<ModelSpace, 3> = v.transformed(t).unwrap();
    assert_eq!(v.position.coordinates(), [18., 26., 27.]);
    assert_eq!(v.first.components(), [2., 6., 0.]);
    assert_eq!(v.second.components(), [-4., 0., 0.]);
}

#[test]
fn analytic_invalid_inputs_and_numeric_limits_fail_explicitly() {
    use tessstep_math::Error as M;
    let zero = Vector2::new([0., 0.]).unwrap();
    let origin = Point2::<ModelSpace>::new([0., 0.]).unwrap();
    assert_eq!(C::line(origin, zero), Err(Error::DegenerateLine));
    assert_eq!(
        C::circle(frame(), length(0.)),
        Err(Error::NonPositiveRadius)
    );
    assert_eq!(
        C::ellipse(frame(), length(1.), length(0.)),
        Err(Error::NonPositiveRadius)
    );
    assert_eq!(
        C::parabola(frame(), length(0.)),
        Err(Error::NonPositiveRadius)
    );
    assert_eq!(
        C::hyperbola(frame(), length(0.), length(1.)),
        Err(Error::NonPositiveRadius)
    );
    assert_eq!(
        PlaneFrame::new(origin, zero, zero, Default::default()),
        Err(Error::Math(M::ZeroDirection))
    );
    let x = Vector2::new([1., 0.]).unwrap();
    for y in [[1., 0.], [-1., 0.], [1., 1e-16]] {
        assert_eq!(
            PlaneFrame::new(origin, x, Vector2::new(y).unwrap(), Default::default()),
            Err(Error::Math(M::ParallelAxes))
        );
    }
    let p = tessstep_math::Point::<ModelSpace, 4>::new([0.; 4]).unwrap();
    let v = tessstep_math::Vector::new([1.; 4]).unwrap();
    assert_eq!(Curve::line(p, v), Err(Error::UnsupportedDimension));
    assert_eq!(
        PlaneFrame::new(p, v, v, Default::default()),
        Err(Error::UnsupportedDimension)
    );
    for c in curves() {
        for u in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(c.evaluate(u), Err(Error::Math(M::NonFinite)));
            assert_eq!(CurveSpan::new(c, u, 1.), Err(Error::Math(M::NonFinite)));
        }
        for (start, end) in [(0., 0.), (-f64::MAX, f64::MAX)] {
            assert_eq!(CurveSpan::new(c, start, end), Err(Error::InvalidInterval));
        }
        let s = CurveSpan::new(c, 0., 1.).unwrap();
        for u in [-0.01, 1.01] {
            assert_eq!(s.evaluate(u), Err(Error::ParameterOutsideSpan));
        }
        assert_eq!(s.evaluate(f64::NAN), Err(Error::Math(M::NonFinite)));
    }
    assert_eq!(
        curves()[0].evaluate(f64::MAX),
        Err(Error::Math(M::NonFinite))
    );
    assert_eq!(curves()[4].evaluate(1000.), Err(Error::Math(M::NonFinite)));
    // Scale before squaring avoids unnecessary overflow on this small parabola.
    assert!(
        C::parabola(frame(), length(1e-300))
            .unwrap()
            .evaluate(1e200)
            .is_ok()
    );
    // Every derivative must be representable, even if the vertex position is finite.
    assert_eq!(
        C::parabola(frame(), length(f64::MAX)).unwrap().evaluate(0.),
        Err(Error::Math(M::NonFinite))
    );
    let tiny = C::circle(frame(), length(f64::from_bits(1)))
        .unwrap()
        .evaluate(0.)
        .unwrap();
    assert_eq!(tiny.position.coordinates(), [f64::from_bits(1), 0.]);
    assert!(
        PlaneFrame::new(
            origin,
            x,
            Vector2::new([1., 1e-10]).unwrap(),
            NumericalTolerance::default()
        )
        .is_ok()
    );
}

#[test]
fn analytic_oriented_branches_and_span_derivatives() {
    let e = C::ellipse(frame(), length(1.), length(4.)).unwrap();
    near(
        e.evaluate(0.).unwrap().position.coordinates(),
        [1., 0.],
        1e-14,
    );
    near(
        e.evaluate(FRAC_PI_2).unwrap().position.coordinates(),
        [0., 4.],
        1e-14,
    );
    let opposite = PlaneFrame::new(
        Point2::<ModelSpace>::new([0., 0.]).unwrap(),
        Vector2::new([-1., 0.]).unwrap(),
        Vector2::new([0., 1.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    let h = C::hyperbola(opposite, length(3.), length(2.))
        .unwrap()
        .evaluate(2_f64.ln())
        .unwrap();
    near(h.position.coordinates(), [-3.75, 1.5], 1e-14);
    near(h.first.components(), [-2.25, 2.5], 1e-14);
    for curve in curves() {
        let span = CurveSpan::new(curve, 2., -1.).unwrap();
        for s in [0.1, 0.3, 0.7, 0.9] {
            let delta = 1e-5;
            let a = span.evaluate(s - delta).unwrap();
            let b = span.evaluate(s + delta).unwrap();
            let v = span.evaluate(s).unwrap();
            near(
                v.first.components(),
                std::array::from_fn(|i| {
                    (b.position.coordinates()[i] - a.position.coordinates()[i]) / (2. * delta)
                }),
                2e-8,
            );
            near(
                v.second.components(),
                std::array::from_fn(|i| {
                    (b.first.components()[i] - a.first.components()[i]) / (2. * delta)
                }),
                2e-8,
            );
        }
    }
    // Parallel arbitrary axes must fail even below ordinary machine epsilon.
    let tol = NumericalTolerance::new(f64::from_bits(1)).unwrap();
    let v = Vector3::<ModelSpace>::new([1., 2., 3.]).unwrap();
    assert_eq!(
        PlaneFrame::new(Point3::new([0.; 3]).unwrap(), v, v, tol),
        Err(Error::Math(tessstep_math::Error::ParallelAxes))
    );
}
