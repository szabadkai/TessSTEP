use std::f64::consts::{FRAC_PI_2, PI};
use tessstep_math::*;
type P = Point3<ModelSpace>;
type V = Vector3<ModelSpace>;
type D = Direction3<ModelSpace>;
type T = Affine3<ModelSpace, ModelSpace>;
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 2e-12 * b.abs().max(1.), "{a} != {b}");
}
fn near3(a: [f64; 3], b: [f64; 3]) {
    for i in 0..3 {
        near(a[i], b[i]);
    }
}
fn diagonal(a: f64, b: f64, c: f64) -> T {
    T::new([[a, 0., 0.], [0., b, 0.], [0., 0., c]], [0.; 3]).unwrap()
}

#[test]
fn math_coordinates_units_and_tolerances_are_explicit() {
    let p = Point2::<ParameterSpace>::new([2., 3.]).unwrap();
    let q = p.translated(Vector2::new([3., 4.]).unwrap()).unwrap();
    assert_eq!(q.coordinates(), [5., 7.]);
    assert_eq!(q.distance(p), Ok(5.));
    let x = V::new([1., 0., 0.]).unwrap();
    let y = V::new([0., 1., 0.]).unwrap();
    assert_eq!(x.dot(y), Ok(0.));
    assert_eq!(x.cross(y).unwrap().components(), [0., 0., 1.]);
    assert_eq!(y.cross(x).unwrap().components(), [0., 0., -1.]);
    assert_eq!(x.added(y).unwrap().subtracted(y), Ok(x));
    near(LengthUnit::MILLIMETRE.to_metres(-25.).unwrap(), -0.025);
    near(LengthUnit::INCH.from_metres(0.0508).unwrap(), 2.);
    near(Angle::degrees(180.).unwrap().as_radians(), PI);
    let model =
        ModelTolerance::new(Length::metres(1e-6).unwrap(), Angle::radians(1e-5).unwrap()).unwrap();
    let mesh =
        TessellationTolerance::new(Length::metres(1e-3).unwrap(), Angle::degrees(5.).unwrap())
            .unwrap();
    assert_eq!(model.distance().as_metres(), 1e-6);
    assert_eq!(model.angle().as_radians(), 1e-5);
    assert_eq!(mesh.chord().as_metres(), 1e-3);
    near(mesh.normal_angle().as_radians(), PI / 36.);
}

#[test]
fn math_rejects_nonfinite_invalid_and_overflow_values() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(P::new([value, 0., 0.]), Err(Error::NonFinite));
        assert_eq!(V::new([0., value, 0.]), Err(Error::NonFinite));
        assert_eq!(D::new([0., 0., value]), Err(Error::NonFinite));
        assert_eq!(Length::metres(value), Err(Error::NonFinite));
        assert_eq!(Angle::radians(value), Err(Error::NonFinite));
        assert_eq!(LengthUnit::metres_per_unit(value), Err(Error::NonFinite));
        assert_eq!(NumericalTolerance::new(value), Err(Error::InvalidTolerance));
        assert_eq!(T::new([[value; 3]; 3], [0.; 3]), Err(Error::NonFinite));
        assert_eq!(T::new([[0.; 3]; 3], [value; 3]), Err(Error::NonFinite));
    }
    assert_eq!(
        Point::<ModelSpace, 0>::new([]),
        Err(Error::InvalidDimension)
    );
    assert_eq!(
        Vector::<ModelSpace, 0>::new([]),
        Err(Error::InvalidDimension)
    );
    assert_eq!(Length::metres(-1.), Err(Error::OutOfRange));
    for bad in [0., -1.] {
        assert_eq!(LengthUnit::metres_per_unit(bad), Err(Error::OutOfRange));
    }
    for bad in [0., -1., 1., 2.] {
        assert_eq!(NumericalTolerance::new(bad), Err(Error::InvalidTolerance));
    }
    let zero = Length::metres(0.).unwrap();
    let one = Length::metres(1.).unwrap();
    let angle = Angle::radians(1.).unwrap();
    assert_eq!(
        ModelTolerance::new(zero, angle),
        Err(Error::InvalidTolerance)
    );
    assert_eq!(
        TessellationTolerance::new(zero, angle),
        Err(Error::InvalidTolerance)
    );
    for bad in [0., -1., PI + 0.1] {
        assert_eq!(
            ModelTolerance::new(one, Angle::radians(bad).unwrap()),
            Err(Error::InvalidTolerance)
        );
        assert_eq!(
            TessellationTolerance::new(one, Angle::radians(bad).unwrap()),
            Err(Error::InvalidTolerance)
        );
    }
    let huge = V::new([f64::MAX, 0., 0.]).unwrap();
    assert_eq!(huge.scaled(2.), Err(Error::NonFinite));
    assert_eq!(huge.dot(huge), Err(Error::NonFinite));
    assert_eq!(
        P::new([f64::MAX; 3]).unwrap().translated(huge),
        Err(Error::NonFinite)
    );
    assert_eq!(
        LengthUnit::MILLIMETRE.from_metres(f64::MAX),
        Err(Error::NonFinite)
    );
    assert_eq!(
        LengthUnit::MILLIMETRE.to_metres(f64::from_bits(1)),
        Err(Error::OutOfRange)
    );
    assert_eq!(
        LengthUnit::metres_per_unit(f64::MAX)
            .unwrap()
            .from_metres(f64::from_bits(1)),
        Err(Error::OutOfRange)
    );
    assert!(LengthUnit::METRE.to_metres(-0.).unwrap().is_sign_negative());
    assert_eq!(
        LengthUnit::METRE.to_metres(f64::from_bits(1)),
        Ok(f64::from_bits(1))
    );
}

#[test]
fn math_normalization_handles_extreme_scales_and_near_angles() {
    for scale in [f64::from_bits(1), 1e-300, 1., 1e300, f64::MAX] {
        let d = D::new([scale, scale, 0.]).unwrap();
        near3(
            d.vector().components(),
            [
                std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
                0.,
            ],
        );
        near(d.vector().norm().unwrap(), 1.);
    }
    assert_eq!(D::new([0.; 3]), Err(Error::ZeroDirection));
    assert_eq!(V::new([f64::MAX; 3]).unwrap().norm(), Err(Error::NonFinite));
    assert_eq!(
        V::new([f64::from_bits(1), 0., 0.]).unwrap().norm(),
        Ok(f64::from_bits(1))
    );
    let x = D::new([1., 0., 0.]).unwrap();
    near(
        x.angle_to(D::new([-1., 0., 0.]).unwrap())
            .unwrap()
            .as_radians(),
        PI,
    );
    let small = x
        .angle_to(D::new([1., 1e-14, 0.]).unwrap())
        .unwrap()
        .as_radians();
    assert!((small / 1e-14 - 1.).abs() < 1e-14);
}

#[test]
fn math_affine_composition_inversion_and_normal_contract() {
    let rotate = T::rotation(
        D::new([0., 0., 1.]).unwrap(),
        Angle::radians(FRAC_PI_2).unwrap(),
    )
    .unwrap();
    let translate = T::from_translation(V::new([10., 20., 30.]).unwrap());
    let p = P::new([2., 3., 4.]).unwrap();
    let t = rotate.then(translate).unwrap();
    near3(t.transform_point(p).unwrap().coordinates(), [7., 22., 34.]);
    near3(
        t.transform_vector(V::new([2., 3., 4.]).unwrap())
            .unwrap()
            .components(),
        [-3., 2., 4.],
    );
    near3(
        translate
            .then(rotate)
            .unwrap()
            .transform_point(p)
            .unwrap()
            .coordinates(),
        [-23., 12., 34.],
    );
    near3(
        t.inverse(Default::default())
            .unwrap()
            .transform_point(t.transform_point(p).unwrap())
            .unwrap()
            .coordinates(),
        p.coordinates(),
    );
    // Known inverse of a shear, nonuniform scale and reflection (not just roundtrip).
    let shear = T::new([[2., 1., 0.], [0., 3., 0.], [0., 0., -4.]], [4., 6., 8.]).unwrap();
    let inv = shear.inverse(Default::default()).unwrap();
    for (a, b) in
        inv.linear()
            .into_iter()
            .zip([[0.5, -1. / 6., 0.], [0., 1. / 3., 0.], [0., 0., -0.25]])
    {
        near3(a, b);
    }
    near3(inv.translation(), [-1., -2., 2.]);
    let tangent = V::new([1., 0., 0.]).unwrap();
    let normal = D::new([0., 1., 0.]).unwrap();
    let nt = shear.transform_normal(normal, Default::default()).unwrap();
    near(
        nt.vector()
            .dot(shear.transform_vector(tangent).unwrap())
            .unwrap(),
        0.,
    );
    // Normals under nonuniform scale differ from transformed directions.
    let scale = diagonal(2., 1., 1.);
    let n = D::new([1., 1., 0.]).unwrap();
    near3(
        scale
            .transform_normal(n, Default::default())
            .unwrap()
            .vector()
            .components(),
        [1. / 5_f64.sqrt(), 2. / 5_f64.sqrt(), 0.],
    );
    near3(
        scale.transform_direction(n).unwrap().vector().components(),
        [2. / 5_f64.sqrt(), 1. / 5_f64.sqrt(), 0.],
    );
}

#[test]
fn math_inverse_pivoting_scale_limits_and_singular_failures() {
    let tol = NumericalTolerance::default();
    let swap = T::new([[0., 1., 0.], [1., 0., 0.], [0., 0., 1.]], [0.; 3]).unwrap();
    assert_eq!(swap.inverse(tol), Ok(swap));
    for s in [1e-300, 1., 1e300] {
        let inv = diagonal(s, s, s).inverse(tol).unwrap();
        near(inv.linear()[0][0] * s, 1.);
    }
    for t in [
        diagonal(0., 1., 1.),
        diagonal(1., 1e-16, 1.),
        diagonal(0., 0., 0.),
    ] {
        assert_eq!(t.inverse(tol), Err(Error::SingularTransform));
        assert_eq!(
            t.transform_normal(D::new([0., 0., 1.]).unwrap(), tol),
            Err(Error::SingularTransform)
        );
        assert!(t.transform_point(P::new([1.; 3]).unwrap()).is_ok());
    }
    let dependent = T::new([[1., 2., 3.], [2., 4., 6.], [0., 0., 1.]], [0.; 3]).unwrap();
    assert_eq!(dependent.inverse(tol), Err(Error::SingularTransform));
    assert_eq!(
        diagonal(1., tol.relative(), 1.).inverse(tol),
        Err(Error::SingularTransform)
    );
    assert!(diagonal(1., tol.relative() * 2., 1.).inverse(tol).is_ok());
    assert_eq!(
        diagonal(f64::from_bits(1), f64::from_bits(1), f64::from_bits(1)).inverse(tol),
        Err(Error::NonFinite)
    );
    let huge = diagonal(f64::MAX, 1., 1.);
    assert_eq!(huge.then(huge), Err(Error::NonFinite));
    assert_eq!(
        huge.transform_point(P::new([2., 0., 0.]).unwrap()),
        Err(Error::NonFinite)
    );
    let z = D::new([0., 0., 1.]).unwrap();
    assert_eq!(
        diagonal(1., 1., 0.).transform_direction(z),
        Err(Error::ZeroDirection)
    );
    near3(
        diagonal(1., 1., -1.)
            .transform_normal(z, tol)
            .unwrap()
            .vector()
            .components(),
        [0., 0., -1.],
    );
    // Normal transformation must not depend on inverse translation range.
    let far = T::new(diagonal(0.5, 0.5, 0.5).linear(), [f64::MAX; 3]).unwrap();
    assert_eq!(far.inverse(tol), Err(Error::NonFinite));
    assert_eq!(far.transform_normal(z, tol), Ok(z));
}

#[test]
fn math_explicit_frames_are_right_handed_and_reject_parallel_axes() {
    let z = D::new([0., 0., 1.]).unwrap();
    let origin = P::new([5., 6., 7.]).unwrap();
    let tol = NumericalTolerance::default();
    let frame = Affine3::<LocalSpace, ModelSpace>::from_frame(
        origin,
        z,
        D::new([2., 0., 2.]).unwrap(),
        tol,
    )
    .unwrap();
    assert_eq!(frame.linear(), T::identity().linear());
    assert_eq!(
        frame
            .transform_point(Point3::new([1., 2., 3.]).unwrap())
            .unwrap()
            .coordinates(),
        [6., 8., 10.]
    );
    for reference in [
        z,
        D::new([0., 0., -1.]).unwrap(),
        D::new([1e-16, 0., 1.]).unwrap(),
    ] {
        assert_eq!(
            Affine3::<LocalSpace, ModelSpace>::from_frame(origin, z, reference, tol),
            Err(Error::ParallelAxes)
        );
    }
    let f = T::from_frame(
        origin,
        D::new([1., 1., 1.]).unwrap(),
        D::new([2., -1., 1.]).unwrap(),
        tol,
    )
    .unwrap();
    let cols: [V; 3] =
        std::array::from_fn(|i| V::new(std::array::from_fn(|j| f.linear()[j][i])).unwrap());
    near3(
        cols[0].cross(cols[1]).unwrap().components(),
        cols[2].components(),
    );
    for i in 0..3 {
        for j in 0..3 {
            near(cols[i].dot(cols[j]).unwrap(), if i == j { 1. } else { 0. });
        }
    }
}

#[test]
fn math_seeded_affine_properties() {
    let mut state = 0x6a09e667f3bcc909_u64;
    let mut sample = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / ((1_u64 << 53) as f64) * 2. - 1.
    };
    for _ in 0..2000 {
        // Strict diagonal dominance keeps these inversions away from singularity.
        let a = std::array::from_fn(|i| {
            std::array::from_fn(|j| sample() + if i == j { 4. } else { 0. })
        });
        let t = T::new(a, std::array::from_fn(|_| sample() * 100.)).unwrap();
        let p = P::new(std::array::from_fn(|_| sample() * 100.)).unwrap();
        let inv = t.inverse(Default::default()).unwrap();
        near3(
            inv.transform_point(t.transform_point(p).unwrap())
                .unwrap()
                .coordinates(),
            p.coordinates(),
        );
        near3(
            t.transform_point(inv.transform_point(p).unwrap())
                .unwrap()
                .coordinates(),
            p.coordinates(),
        );
        let id = t.then(inv).unwrap();
        for (a, b) in id.linear().into_iter().zip(T::identity().linear()) {
            near3(a, b);
        }
        near3(id.translation(), [0.; 3]);
    }
}
