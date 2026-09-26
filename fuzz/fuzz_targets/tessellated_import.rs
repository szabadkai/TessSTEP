#![no_main]
//! Selected-root existing-tessellation import under small finite budgets. Results must
//! be deterministic; any published mesh has already passed owned-mesh validation.
use libfuzzer_sys::fuzz_target;
#[path = "../limits.rs"]
mod budgets;
fuzz_target!(|data: &[u8]| {
    let Ok(document) = tessstep_model::parse(data, budgets::limits()) else {
        return;
    };
    let import = || {
        tessstep_import::import_tessellated(
            &document,
            tessstep_part21::EntityId::new(1000).unwrap(),
            tessstep_math::LengthUnit::MILLIMETRE,
            tessstep_import::ImportOptions {
                max_work: 65536,
                max_records: 256,
                ..tessstep_import::ImportOptions::default()
            },
            tessstep_mesh::Limits {
                max_vertices: 4096,
                max_triangles: 8192,
                max_work: 262144,
            },
        )
    };
    match (import(), import()) {
        (Ok(a), Ok(b)) => assert_eq!(a.mesh().data().triangles, b.mesh().data().triangles),
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("nondeterministic tessellated import"),
    }
});
