use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use tessstep_curves::{PlaneFrame, spline::NurbsCurve};
use tessstep_math::{Length, ModelSpace, Point3, Vector3};
use tessstep_surfaces::{NurbsSurface, Surface};
fn measure(name: &str, mut f: impl FnMut(f64)) {
    let once = |f: &mut dyn FnMut(f64)| {
        for i in 0..1000 {
            f(black_box(f64::from(i) / 1000.));
        }
    };
    once(&mut f);
    let t = Instant::now();
    let mut n = 0_u32;
    while t.elapsed() < Duration::from_secs(1) {
        once(&mut f);
        n += 1;
    }
    let s = t.elapsed().as_secs_f64();
    println!(
        "{name}: {n} batches of 1000, {s:.3} s, {:.3} us/batch",
        s * 1e6 / f64::from(n)
    );
}
fn main() {
    let frame = PlaneFrame::new(
        Point3::<ModelSpace>::new([0.; 3]).unwrap(),
        Vector3::new([1., 0., 0.]).unwrap(),
        Vector3::new([0., 1., 0.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    let a = Surface::torus(
        frame,
        Length::metres(4.).unwrap(),
        Length::metres(1.).unwrap(),
    )
    .unwrap();
    let c = NurbsCurve::new(
        2,
        &[0., 0., 0., 1., 1., 1.],
        &[
            Point3::<ModelSpace>::new([1., 0., 0.]).unwrap(),
            Point3::new([1., 1., 0.]).unwrap(),
            Point3::new([0., 1., 0.]).unwrap(),
        ],
        &[1., std::f64::consts::FRAC_1_SQRT_2, 1.],
        Default::default(),
    )
    .unwrap();
    let p: Vec<_> = [[0., 0., 0.], [0., 1., 0.], [1., 0., 0.], [1., 1., 1.]]
        .into_iter()
        .map(|p| Point3::<ModelSpace>::new(p).unwrap())
        .collect();
    let s = NurbsSurface::new(
        [1, 1],
        [&[0., 0., 1., 1.]; 2],
        [2, 2],
        &p,
        &[1., 2., 3., 4.],
        Default::default(),
    )
    .unwrap();
    measure("analytic torus jet", |u| {
        black_box(a.evaluate(u, 0.4).unwrap());
    });
    measure("rational curve jet", |u| {
        black_box(c.evaluate(u).unwrap());
    });
    measure("rational surface jet", |u| {
        black_box(s.evaluate(u, 0.4).unwrap());
    });
}
