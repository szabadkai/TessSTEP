//! Loop healing for exporter parity: authored reproductions of the face loop
//! defects that kept two NIST PMI test-case bodies (stc_06 and stc_07) from meshing.
//! A FACE_OUTER_BOUND written on a hole bounds a hole; a cone generator written far
//! from its apex keeps its exact chart image; an edge off its surface between the
//! pcurve samples is measured at its peak. See docs/CURVED_IMPORT.md.
use std::f64::consts::PI;
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const HOLE_OUTER: &str = include_str!("../../../corpus/geometry/brep-hole-outer-bound.step");
const FAR_GENERATOR: &str = include_str!("../../../corpus/geometry/brep-far-generator.step");
const SAMPLED_PEAK: &str = include_str!("../../../corpus/geometry/brep-sampled-peak.step");

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
fn strict() -> ImportOptions {
    ImportOptions {
        strict: true,
        ..ImportOptions::default()
    }
}
fn tessellate(solid: &ImportedSolid, chord: f64) -> Result<tessstep_mesh::Mesh, Error> {
    let tolerance =
        TessellationTolerance::new(Length::metres(chord).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    solid.tessellate(tolerance, TessellationOptions::default())
}
/// The mesh closes and its volume is within twice the chord over the smallest
/// curvature radius of `exact`.
fn check_solid(solid: &ImportedSolid, exact: f64, chord: f64, radius: f64) {
    let mesh = tessellate(solid, chord).unwrap();
    mesh.require_solid().unwrap();
    let volume = mesh.statistics().signed_volume;
    assert!(
        ((volume - exact) / exact).abs() <= 2. * chord / radius,
        "volume {volume} exact {exact}"
    );
}

#[test]
fn outer_bound_labels_on_holes_bound_holes_unless_strict() {
    // The washer's top face labels its bore FACE_OUTER_BOUND; both loops keep their
    // geometric senses.
    let solid = import(HOLE_OUTER, 1e-8, ImportOptions::default()).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inserted_seams: 2,
            relabelled_outer_bounds: 1,
            ..Adaptations::default()
        }
    );
    check_solid(&solid, PI * 1500. * 1e-9, 1e-4, 0.01);
    // Strict import keeps the label, and trimming rejects the clockwise outer bound.
    let kept = import(HOLE_OUTER, 1e-8, strict()).unwrap();
    assert_eq!(kept.adaptations().relabelled_outer_bounds, 0);
    let error = tessellate(&kept, 1e-4).unwrap_err();
    assert_eq!(
        (error.kind, error.stage, error.entity),
        (
            ErrorKind::InvalidGeometry,
            Stage::Tessellation,
            EntityId::new(80)
        ),
        "{error}"
    );
    assert!(
        error
            .message
            .contains("the outer bound winds the wrong way"),
        "{error}"
    );
    // Reversed loops are not mislabelled ones: with the rim reversed no loop winds
    // counterclockwise, and with both reversed the counterclockwise bore encloses
    // nothing. Neither is relabelled, and neither meshes.
    let rim = HOLE_OUTER.replace("#74=FACE_BOUND('',#73,.T.)", "#74=FACE_BOUND('',#73,.F.)");
    let both = rim.replace(
        "#71=FACE_OUTER_BOUND('',#70,.T.)",
        "#71=FACE_OUTER_BOUND('',#70,.F.)",
    );
    for reversed in [rim, both] {
        assert_ne!(reversed, HOLE_OUTER);
        match import(&reversed, 1e-8, ImportOptions::default()) {
            Ok(solid) => {
                assert_eq!(solid.adaptations().relabelled_outer_bounds, 0);
                assert!(tessellate(&solid, 1e-4).is_err());
            }
            Err(error) => assert_ne!(error.stage, Stage::Profile, "{error}"),
        }
    }
}

#[test]
fn cone_generators_written_far_from_their_apex_keep_exact_images() {
    // One generator's LINE origin lies 5 km away and the line passes 10 nm from the
    // apex: its exact image ends a rounding error below the apex.
    let solid = import(FAR_GENERATOR, 1e-6, ImportOptions::default()).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            collapsed_edges: 2,
            ..Adaptations::default()
        }
    );
    assert_eq!(solid.largest_deviation(), 0.);
    check_solid(&solid, PI / 3. * 1e-6, 1e-4, 0.01);
    assert!(import(FAR_GENERATOR, 1e-6, strict()).is_ok());
}

#[test]
fn edge_deviations_between_pcurve_samples_are_measured_at_their_peak() {
    // The top edge bulges 2.7253 um off its side at 21% and 79% of each quarter arc,
    // past the 2.7 um tolerance, and at most 2.668 um at the fit's samples.
    let solid = import(SAMPLED_PEAK, 2.7e-6, ImportOptions::default()).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inserted_seams: 1,
            off_surface_edges: 1,
            ..Adaptations::default()
        }
    );
    assert!(
        (solid.largest_deviation() - 2.7253e-6).abs() < 1e-10,
        "{}",
        solid.largest_deviation()
    );
    let working = solid.brep().model_tolerance();
    assert!((working - (2.7e-6 + 2. * solid.largest_deviation())).abs() < 1e-18);
    check_solid(&solid, PI * 2000. * 1e-9, 1e-4, 0.01);
    // Strict import accepts no deviation: a typed rejection at the peak.
    let error = import(SAMPLED_PEAK, 2.7e-6, strict()).unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::InvalidGeometry, Stage::Geometry),
        "{error}"
    );
    assert!(
        error
            .message
            .contains("lies 2.725e-6 m from the face surface"),
        "{error}"
    );
}
