//! Sliver faces under the tolerance policy: a sliver whose edges coincide only within
//! the working tolerance an accepted vertex deviation implies is collapsed there.
//! See docs/CURVED_IMPORT.md#face-charts (sliver faces).
use std::f64::consts::PI;
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const CRESCENT: &str = include_str!("../../../corpus/geometry/brep-crescent-sliver.step");

/// The fixture's context uncertainty is 1 um.
fn import(options: ImportOptions) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(CRESCENT.as_bytes(), ParseLimits::default()).unwrap();
    import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        ModelTolerance::new(Length::metres(1e-6).unwrap(), Angle::radians(1e-8).unwrap()).unwrap(),
        options,
    )
}

#[test]
fn sliver_faces_collapse_at_the_working_tolerance_a_vertex_deviation_implies() {
    // The cap of a r = 10 mm cylinder is bounded by an arc 3 um inside the side's
    // top arc at its apex, with a crescent face between the two arcs; both arcs
    // end at a vertex 3 um outside them. The vertex deviation (accepted, 3x the 1 um
    // tolerance) widens every later check to the working tolerance of 7 um, at
    // which the crescent encloses no area: it is collapsed as a sliver face there,
    // the cap takes the outer arc, and the mesh is the full cylinder. (At the model
    // tolerance alone the arcs do not coincide, and trimming rejected the crescent
    // as a degenerate loop.)
    let solid = import(ImportOptions::default()).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inserted_seams: 1,
            collapsed_faces: 1,
            off_curve_vertices: 3,
            ..Adaptations::default()
        }
    );
    assert!((solid.largest_deviation() - 3e-6).abs() < 1e-12);
    assert!((solid.brep().model_tolerance() - 7e-6).abs() < 1e-18);
    assert_eq!(solid.face_entities().len(), 3);
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-5).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    let mesh = solid
        .tessellate(tolerance, TessellationOptions::default())
        .unwrap();
    mesh.require_solid().unwrap();
    let exact = PI * 100. * 20. * 1e-9;
    let volume = mesh.statistics().signed_volume;
    assert!(
        ((volume - exact) / exact).abs() <= 2. * 1e-5 / 0.01,
        "{volume}"
    );
    // Strict import accepts no deviation: the vertex is rejected before any face is
    // planned, so no sliver is collapsed either.
    let error = import(ImportOptions {
        strict: true,
        ..ImportOptions::default()
    })
    .unwrap_err();
    assert_eq!(error.stage, Stage::Geometry);
    assert!(
        error.message.contains("from the edge curve"),
        "{}",
        error.message
    );
}

const STACKED: &str = include_str!("../../../corpus/geometry/brep-stacked-slivers.step");

#[test]
fn stacked_sliver_faces_collapse_onto_one_edge() {
    // Two sliver faces between three edges on one half circle: the first collapsed
    // keeps the middle edge, which the second then merges into the side's arc. Each
    // use follows the chain to the last survivor (this panicked before; seen in an
    // OpenCASCADE surface model sewn under PreForm's settings).
    let doc = tessstep_model::parse(STACKED.as_bytes(), ParseLimits::default()).unwrap();
    let solid = import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        ModelTolerance::new(Length::metres(1e-7).unwrap(), Angle::radians(1e-8).unwrap()).unwrap(),
        ImportOptions::default(),
    )
    .unwrap();
    assert_eq!(solid.adaptations().collapsed_faces, 2);
    assert_eq!(solid.face_entities().len(), 3);
    assert_eq!(solid.brep().data().edges.len(), 4);
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-5).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    let mesh = solid
        .tessellate(tolerance, TessellationOptions::default())
        .unwrap();
    mesh.require_solid().unwrap();
    let exact = PI * 100. * 20. * 1e-9;
    let volume = mesh.statistics().signed_volume;
    assert!(
        ((volume - exact) / exact).abs() <= 2. * 1e-5 / 0.01,
        "{volume}"
    );
}
