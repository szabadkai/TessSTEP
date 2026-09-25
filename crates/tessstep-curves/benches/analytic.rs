use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use tessstep_curves::{Curve3, PlaneFrame};
use tessstep_math::{Length, ModelSpace, Point3, Vector3};
fn main() {
    let frame = PlaneFrame::new(
        Point3::<ModelSpace>::new([1., 2., 3.]).unwrap(),
        Vector3::new([1., 0., 0.]).unwrap(),
        Vector3::new([0., 1., 0.]).unwrap(),
        Default::default(),
    )
    .unwrap();
    let curve = Curve3::ellipse(
        frame,
        Length::metres(3.).unwrap(),
        Length::metres(2.).unwrap(),
    )
    .unwrap();
    let once = || {
        for i in 0..10_000 {
            black_box(
                black_box(curve)
                    .evaluate(black_box(f64::from(i) * 0.001))
                    .unwrap(),
            );
        }
    };
    once();
    let start = Instant::now();
    let mut iterations = 0_u32;
    while start.elapsed() < Duration::from_secs(2) {
        once();
        iterations += 1;
    }
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "analytic ellipse: 10,000 position/first/second evaluations; {iterations} batches, {seconds:.3} s, {:.3} us/batch",
        seconds * 1e6 / f64::from(iterations)
    );
}
