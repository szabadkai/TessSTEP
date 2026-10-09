//! Curved B-rep tolerance policy (Milestone 22.3): edges off their face surfaces,
//! vertices off their edge curves and B-spline sides that collapse only within the
//! model tolerance. See docs/CURVED_IMPORT.md#tolerance-policy.
use std::f64::consts::PI;
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const NEAR: &str = include_str!("../../../corpus/geometry/brep-near-surface.step");
const FAR: &str = include_str!("../../../corpus/geometry/brep-far-surface.step");
const VERTEX: &str = include_str!("../../../corpus/geometry/brep-vertex-off-curve.step");
const SHORT_SIDE: &str = include_str!("../../../corpus/geometry/brep-short-side.step");

/// The fixtures' context uncertainty (1 um, or 50 um for the short side).
fn import(text: &str, tolerance: f64, options: ImportOptions) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        ModelTolerance::new(
            Length::metres(tolerance).unwrap(),
            Angle::radians(1e-8).unwrap(),
        )
        .unwrap(),
        options,
    )
}
fn chord(metres: f64) -> TessellationTolerance {
    TessellationTolerance::new(
        Length::metres(metres).unwrap(),
        Angle::radians(0.1).unwrap(),
    )
    .unwrap()
}
fn mesh(solid: &ImportedSolid, metres: f64) -> tessstep_mesh::Mesh {
    let mesh = solid
        .tessellate(chord(metres), TessellationOptions::default())
        .unwrap();
    mesh.require_solid().unwrap();
    mesh
}
fn strict() -> ImportOptions {
    ImportOptions {
        strict: true,
        ..ImportOptions::default()
    }
}

#[test]
fn deviations_within_the_policy_are_counted_and_widen_the_working_tolerance() {
    assert_eq!(ImportOptions::default().deviation_factor, 16.);
    // The top circle edge lies 3 um outside its r = 10 mm side cylinder.
    let near = import(NEAR, 1e-6, ImportOptions::default()).unwrap();
    assert_eq!(
        near.adaptations(),
        Adaptations {
            inserted_seams: 1,
            off_surface_edges: 1,
            ..Adaptations::default()
        }
    );
    assert!((near.largest_deviation() - 3e-6).abs() < 1e-12);
    // Validation, trimming and tessellation allow a vertex and an edge end beside it
    // to deviate on either side: the model tolerance plus twice the deviation.
    let working = near.brep().model_tolerance();
    assert!((working - (1e-6 + 2. * near.largest_deviation())).abs() < 1e-18);
    // The mesh follows the 3D circle: a frustum from r = 10 to r = 10.003 mm.
    let (r0, r1) = (10., 10.003);
    let exact = PI * 20. / 3. * (r0 * r0 + r0 * r1 + r1 * r1) * 1e-9;
    let volume = mesh(&near, 1e-5).statistics().signed_volume;
    assert!(
        ((volume - exact) / exact).abs() <= 2. * 1e-5 / 0.01,
        "{volume}"
    );

    // The top vertex lies 3 um outside its circle; the edge is bounded at its
    // projection and the mesh uses the vertex.
    let vertex = import(VERTEX, 1e-6, ImportOptions::default()).unwrap();
    assert_eq!(
        vertex.adaptations(),
        Adaptations {
            inserted_seams: 1,
            off_curve_vertices: 1,
            ..Adaptations::default()
        }
    );
    assert!((vertex.largest_deviation() - 3e-6).abs() < 1e-12);
    let exact = PI * 100. * 20. * 1e-9;
    let volume = mesh(&vertex, 1e-5).statistics().signed_volume;
    assert!(
        ((volume - exact) / exact).abs() <= 2. * 1e-5 / 0.01,
        "{volume}"
    );

    // Within the model tolerance nothing deviates: no adaptation, no widening.
    let loose = import(NEAR, 4e-6, ImportOptions::default()).unwrap();
    assert_eq!(loose.adaptations().off_surface_edges, 0);
    assert_eq!(loose.largest_deviation(), 0.);
    assert_eq!(loose.brep().model_tolerance(), 4e-6);
    // Record order does not change the outcome.
    let (header, data) = NEAR.split_once("DATA;\n").unwrap();
    let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
    records.reverse();
    let reordered = format!(
        "{header}DATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        records.join("\n")
    );
    let again = import(&reordered, 1e-6, ImportOptions::default()).unwrap();
    assert_eq!(again.adaptations(), near.adaptations());
    assert_eq!(again.largest_deviation(), near.largest_deviation());
}

#[test]
fn deviations_beyond_the_policy_are_typed_rejections_naming_distance_and_bound() {
    let check = |result: Result<ImportedSolid, Error>, parts: &[&str]| {
        let error = result.unwrap_err();
        assert_eq!(
            (error.kind, error.stage),
            (ErrorKind::InvalidGeometry, Stage::Geometry),
            "{error}"
        );
        for part in parts {
            assert!(error.message.contains(part), "{error}");
        }
        assert!(error.entity.is_some() && error.source.is_some());
    };
    // 50 um off: beyond 16 x 1 um.
    check(
        import(FAR, 1e-6, ImportOptions::default()),
        &[
            "lies 5.000e-5 m from the face surface at curve parameter",
            "beyond the accepted deviation 1.600e-5 m (16 × the model tolerance 1.000e-6 m)",
        ],
    );
    let far_vertex = VERTEX.replace("(10.003,0.0,20.0)", "(10.05,0.0,20.0)");
    assert_ne!(far_vertex, VERTEX);
    check(
        import(&far_vertex, 1e-6, ImportOptions::default()),
        &[
            "VERTEX_POINT #4 of EDGE_CURVE #16 lies 5.000e-5 m from the edge curve",
            "beyond the accepted deviation",
        ],
    );
    // Strict import keeps rejecting every deviation; a smaller factor narrows the
    // bound.
    check(
        import(NEAR, 1e-6, strict()),
        &[
            "lies 3.000e-6 m from the face surface",
            "beyond the model tolerance 1.000e-6 m",
        ],
    );
    check(
        import(VERTEX, 1e-6, strict()),
        &["lies 3.000e-6 m from the edge curve, beyond the model tolerance 1.000e-6 m"],
    );
    let factor = |deviation_factor| ImportOptions {
        deviation_factor,
        ..ImportOptions::default()
    };
    check(
        import(NEAR, 1e-6, factor(2.)),
        &["beyond the accepted deviation 2.000e-6 m (2 × the model tolerance 1.000e-6 m)"],
    );
    assert_eq!(
        import(NEAR, 1e-6, factor(1.)).unwrap_err().stage,
        Stage::Geometry
    );
    for invalid in [0.5, f64::NAN, f64::INFINITY] {
        let error = import(NEAR, 1e-6, factor(invalid)).unwrap_err();
        assert_eq!(
            (error.kind, error.stage),
            (ErrorKind::InvalidOptions, Stage::Profile),
            "{error}"
        );
    }
    // A chord tolerance below the accepted deviation is refused.
    let near = import(NEAR, 1e-6, ImportOptions::default()).unwrap();
    let error = near
        .tessellate(chord(1e-6), TessellationOptions::default())
        .unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::InvalidGeometry, Stage::Tessellation)
    );
    assert_eq!(
        error.message,
        "the largest accepted deviation 3.000e-6 m exceeds the chord tolerance 1.000e-6 m"
    );
}

#[test]
fn bspline_sides_collapsed_only_within_tolerance_are_regular_and_poles_are_rejected() {
    // A 20 um side under a 50 um uncertainty collapses within the model tolerance;
    // the edge along it makes it a regular side of the face chart.
    let t = 0.014;
    let exact = (1000. + 100. * t) / 6. * 1e-9;
    let short = import(SHORT_SIDE, 5e-5, ImportOptions::default()).unwrap();
    assert_eq!(
        short.adaptations(),
        Adaptations {
            uncollapsed_sides: 1,
            ..Adaptations::default()
        }
    );
    assert_eq!(short.largest_deviation(), 0.);
    let volume = mesh(&short, 1e-5).statistics().signed_volume;
    assert!(((volume - exact) / exact).abs() < 1e-9, "{volume}");
    // Representation, not tolerance: strict import charts it the same way.
    assert_eq!(
        import(SHORT_SIDE, 5e-5, strict()).unwrap().adaptations(),
        short.adaptations()
    );
    // Under a 1 um tolerance the side is not collapsed at all.
    let fine = import(SHORT_SIDE, 1e-6, ImportOptions::default()).unwrap();
    assert_eq!(fine.adaptations(), Adaptations::default());
    let volume = mesh(&fine, 1e-5).statistics().signed_volume;
    assert!(((volume - exact) / exact).abs() < 1e-9, "{volume}");
    // A side of 14 nm is a pole: an edge through its point stays a typed rejection
    // naming the parameter, the distance, the side's extent and the tolerance.
    let pole = SHORT_SIDE.replace("(-0.014,0.014,0.0)", "(-1.0E-05,1.0E-05,0.0)");
    assert_ne!(pole, SHORT_SIDE);
    let error = import(&pole, 5e-5, ImportOptions::default()).unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::Unsupported, Stage::Geometry),
        "{error}"
    );
    for part in [
        "EDGE_CURVE #30 crosses a singular point of the face surface at curve parameter",
        "whose singular line spans 7.071e-9 m, within the model tolerance 5.000e-5 m",
    ] {
        assert!(error.message.contains(part), "{error}");
    }
}

#[test]
fn tolerance_fixtures_survive_byte_mutations() {
    let mut seed = 2203u64;
    for text in [NEAR, VERTEX, SHORT_SIDE] {
        for _ in 0..100 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mut bytes = text.as_bytes().to_vec();
            let at = seed as usize % bytes.len();
            bytes[at] = (seed >> 32) as u8;
            if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
                let _ = import_brep_solid(
                    &doc,
                    EntityId::new(1000).unwrap(),
                    LengthUnit::MILLIMETRE,
                    AngleUnit::RADIAN,
                    ModelTolerance::new(
                        Length::metres(1e-6).unwrap(),
                        Angle::radians(1e-8).unwrap(),
                    )
                    .unwrap(),
                    ImportOptions {
                        max_work: 200_000,
                        max_records: 1000,
                        ..ImportOptions::default()
                    },
                );
            }
        }
    }
}
