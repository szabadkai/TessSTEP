//! Milestone 22.4: UV loop validity of curved imports. Chart causes that are the
//! importer's are fixed; loops that are invalid as given fail with located, named
//! errors.
use std::f64::consts::PI;
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const UNKNOWN_CLOSURE: &str =
    include_str!("../../../corpus/geometry/brep-unknown-closure-cylinder.step");
const CUT_SPHERE: &str = include_str!("../../../corpus/geometry/brep-cut-sphere.step");
const PINCHED_POCKET: &str = include_str!("../../../corpus/geometry/brep-pinched-pocket.step");
const ENCLOSED_HOLE: &str = include_str!("../../../corpus/geometry/brep-enclosed-hole.step");

fn import(text: &str) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap(),
        ImportOptions::default(),
    )
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
fn closed_bspline_axes_with_unknown_closure_get_periodic_charts() {
    // U_CLOSED .U. on a closed surface: a periodic chart and an inserted seam.
    let solid = import(UNKNOWN_CLOSURE).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inserted_seams: 1,
            ..Adaptations::default()
        }
    );
    check_solid(&solid, PI * 100. * 20. * 1e-9, 1e-4, 0.01);
    // The same surface flagged open (.F.) keeps its non-periodic chart, where both
    // circles map onto chart boundaries and neither loop is an outer bound.
    let open = UNKNOWN_CLOSURE.replace(".CYLINDRICAL_SURF.,.U.,", ".CYLINDRICAL_SURF.,.F.,");
    let e = import(&open).unwrap_err();
    assert_eq!(
        (e.stage, e.kind),
        (Stage::Geometry, ErrorKind::InvalidGeometry)
    );
    assert!(e.message.contains("ambiguous outer bound"), "{}", e.message);
}

#[test]
fn spheres_outside_their_only_loop_are_charted_through_it() {
    // In the STEP frame (axis x) the rim does not wind around the axis and the face
    // lies outside it; a frame through the rim closes the face at the far pole.
    let solid = import(CUT_SPHERE).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inserted_seams: 1,
            recharted_spheres: 1,
            collapsed_edges: 1,
            ..Adaptations::default()
        }
    );
    check_solid(&solid, 3584. / 3. * PI * 1e-9, 1e-4, 0.01);
}

#[test]
fn self_touching_loops_fail_at_their_edges_with_a_named_cause() {
    let solid = import(PINCHED_POCKET).unwrap();
    let e = tessellate(&solid, 1e-4).unwrap_err();
    assert_eq!(
        (e.stage, e.kind),
        (Stage::Tessellation, ErrorKind::InvalidGeometry)
    );
    // The top face's loop passes through C = (10, 0, 10), VERTEX_POINT #18, twice;
    // the error names one of the two edges that touch there.
    let doc = tessstep_model::parse(PINCHED_POCKET.as_bytes(), ParseLimits::default()).unwrap();
    let entity = e.entity.unwrap();
    assert_eq!(
        doc.entities().get(entity).unwrap().kind.records()[0]
            .name
            .as_ref(),
        "EDGE_CURVE"
    );
    assert!(
        e.message.contains(&format!("EDGE_CURVE #{}", entity.get()))
            && e.message.contains(
                "touch at vertex #18, which the outer bound passes through twice (self-touching loop)"
            ),
        "{}",
        e.message
    );
}

#[test]
fn holes_that_later_holes_enclose_are_bridged_after_them() {
    // Both plate faces list their square hole before the two L-shaped holes that
    // hide every face corner from it, so in face order the square has no bridge into
    // the face polygon; the top face's loops are all FACE_BOUND, its outer inferred.
    let solid = import(ENCLOSED_HOLE).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inferred_outer_bounds: 1,
            ..Adaptations::default()
        }
    );
    let mesh = tessellate(&solid, 1e-5).unwrap();
    mesh.require_solid().unwrap();
    let volume = mesh.statistics().signed_volume;
    assert!((volume - 140e-9).abs() <= 1e-12 * 140e-9, "volume {volume}");
}
