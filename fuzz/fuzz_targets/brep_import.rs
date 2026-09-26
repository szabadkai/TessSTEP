#![no_main]
//! Selected-root curved B-rep import, root discovery and tessellation under small
//! finite budgets. Results must be deterministic; a published mesh has passed the
//! owned-mesh solid checks.
use libfuzzer_sys::fuzz_target;
#[path = "../limits.rs"]
mod budgets;
fuzz_target!(|data: &[u8]| {
    let Ok(document) = tessstep_model::parse(data, budgets::limits()) else {
        return;
    };
    let options = tessstep_import::ImportOptions {
        max_work: 65536,
        max_records: 256,
        ..tessstep_import::ImportOptions::default()
    };
    let _ = tessstep_import::discover_solids(&document, options);
    let tolerance = tessstep_math::ModelTolerance::new(
        tessstep_math::Length::metres(1e-8).unwrap(),
        tessstep_math::Angle::radians(1e-8).unwrap(),
    )
    .unwrap();
    let chord = tessstep_import::TessellationTolerance::new(
        tessstep_math::Length::metres(1e-4).unwrap(),
        tessstep_math::Angle::radians(0.3).unwrap(),
    )
    .unwrap();
    let mut limits = tessstep_import::TessellationOptions::default();
    limits.max_work = 262144;
    limits.max_evaluations = 65536;
    limits.planar.max_vertices = 4096;
    limits.planar.max_triangles = 8192;
    let import = || {
        tessstep_import::import_brep_solid(
            &document,
            tessstep_part21::EntityId::new(1000).unwrap(),
            tessstep_math::LengthUnit::MILLIMETRE,
            tessstep_math::AngleUnit::RADIAN,
            tolerance,
            options,
        )
        .and_then(|solid| solid.tessellate(chord, limits))
    };
    match (import(), import()) {
        (Ok(a), Ok(b)) => {
            a.require_solid().unwrap();
            assert_eq!(a.data().triangles, b.data().triangles);
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("nondeterministic curved B-rep import"),
    }
});
