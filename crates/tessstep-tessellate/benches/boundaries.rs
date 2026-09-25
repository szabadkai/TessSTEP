use std::{
    hint::black_box,
    time::{Duration, Instant},
};
#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_tessellate::{SamplingLimits, sample_edges};
use tessstep_topology::{FaceId, ValidationLimits};
fn main() {
    let raw = support::cylinder_seam();
    let normalized = raw
        .clone()
        .validate(support::tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    let tolerance =
        TessellationTolerance::new(Length::metres(0.001).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    for name in [
        "topology validation and normalization",
        "UV seam reconstruction",
        "shared-edge sampling and face boundary",
    ] {
        let run = || match name {
            "topology validation and normalization" => {
                black_box(
                    raw.clone()
                        .validate(support::tolerance(), ValidationLimits::default())
                        .unwrap()
                        .normalize(),
                );
            }
            "UV seam reconstruction" => {
                black_box(
                    tessstep_trim::reconstruct(
                        &normalized,
                        FaceId(0),
                        tessstep_trim::Options::default(),
                    )
                    .unwrap(),
                );
            }
            _ => {
                let edges =
                    sample_edges(&normalized, tolerance, SamplingLimits::default()).unwrap();
                black_box(
                    edges
                        .face_boundary(FaceId(0), tessstep_trim::Options::default(), 10000)
                        .unwrap(),
                );
            }
        };
        run();
        let start = Instant::now();
        let mut n = 0;
        while start.elapsed() < Duration::from_secs(1) {
            run();
            n += 1;
        }
        println!(
            "{name}: {n} iterations, {:.3} us/iteration",
            start.elapsed().as_secs_f64() * 1e6 / n as f64
        );
    }
}
