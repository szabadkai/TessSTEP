#![no_main]
//! Selected-root presentation-tessellation import (root 300, as in the authored
//! annotation fixture) under small finite budgets. Results must be deterministic and
//! every published index must address the published positions.
use libfuzzer_sys::fuzz_target;
#[path = "../limits.rs"]
mod budgets;
fuzz_target!(|data: &[u8]| {
    let Ok(document) = tessstep_model::parse(data, budgets::limits()) else {
        return;
    };
    let import = || {
        tessstep_import::import_presentation(
            &document,
            tessstep_part21::EntityId::new(300).unwrap(),
            tessstep_math::LengthUnit::MILLIMETRE,
            tessstep_import::ImportOptions {
                max_work: 65536,
                max_records: 256,
                ..tessstep_import::ImportOptions::default()
            },
            tessstep_import::PresentationLimits {
                max_vertices: 4096,
                max_triangles: 8192,
                max_polyline_points: 8192,
                max_depth: 8,
            },
        )
    };
    match (import(), import()) {
        (Ok(a), Ok(b)) => {
            let n = a.positions().len() as u32;
            assert!(a.triangles().iter().flatten().all(|&v| v < n));
            assert!(a.polyline_points().iter().chain(a.points()).all(|&v| v < n));
            assert!(a.positions().iter().flatten().all(|x| x.is_finite()));
            assert_eq!(a, b);
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("nondeterministic presentation import"),
    }
});
