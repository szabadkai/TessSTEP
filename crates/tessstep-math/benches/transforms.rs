use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use tessstep_math::{Affine3, ModelSpace, Point3};
fn main() {
    let t = Affine3::<ModelSpace, ModelSpace>::new(
        [[2., 1., 0.], [0., 3., 0.], [0., 0., -4.]],
        [4., 6., 8.],
    )
    .unwrap();
    let points: Vec<_> = (0..10_000)
        .map(|i| Point3::new([f64::from(i), 2., 3.]).unwrap())
        .collect();
    let once = || {
        let inverse = black_box(t).inverse(Default::default()).unwrap();
        for &p in &points {
            black_box(inverse.transform_point(black_box(p)).unwrap());
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
        "math: inverse + 10,000 points; {iterations} iterations, {seconds:.3} s, {:.3} us/batch",
        seconds * 1e6 / f64::from(iterations)
    );
}
