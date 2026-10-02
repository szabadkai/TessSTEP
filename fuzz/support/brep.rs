//! Selected-root curved B-rep import, root discovery and tessellation under small
//! finite budgets. Results must be deterministic; a published mesh is watertight with
//! positive volume and one component per shell, since each `BREP_WITH_VOIDS` cavity
//! meshes as an inward component (fuzz/regressions/brep_import).
#[path = "../limits.rs"]
mod budgets;
pub fn exercise(data: &[u8]) {
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
    let defaults = tessstep_import::TessellationOptions::default();
    let limits = tessstep_import::TessellationOptions {
        max_work: 262144,
        max_evaluations: 65536,
        planar: tessstep_tessellate::PlanarOptions {
            max_vertices: 4096,
            max_triangles: 8192,
            ..defaults.planar
        },
        ..defaults
    };
    let import = || {
        tessstep_import::import_brep_solid(
            &document,
            tessstep_part21::EntityId::new(1000).unwrap(),
            tessstep_math::LengthUnit::MILLIMETRE,
            tessstep_math::AngleUnit::RADIAN,
            tolerance,
            options,
        )
        .and_then(|solid| {
            let solids = &solid.brep().data().solids;
            let shells = solids.len() + solids.iter().map(|s| s.voids.len()).sum::<usize>();
            solid.tessellate(chord, limits).map(|mesh| (mesh, shells))
        })
    };
    match (import(), import()) {
        (Ok((a, shells)), Ok((b, _))) => {
            let statistics = a.statistics();
            assert!(a.is_watertight() && statistics.signed_volume > 0.);
            assert_eq!(statistics.components, shells);
            assert_eq!(a.data().triangles, b.data().triangles);
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("nondeterministic curved B-rep import"),
    }
}
