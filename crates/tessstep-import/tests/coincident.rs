//! Faces that lie on a neighbouring face's surface, facing away from it: the lens
//! between them is thinner than the model tolerance, so the face is merged into its
//! neighbour before planning (`Adaptations::collapsed_faces`). See
//! docs/CURVED_IMPORT.md#face-charts (coincident faces).
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const FLAP: &str = include_str!("../../../corpus/geometry/brep-lens-flap.step");

fn import(text: &str, options: ImportOptions) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap(),
        options,
    )
}
fn mesh(solid: &ImportedSolid) -> tessstep_mesh::Mesh {
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-5).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    solid
        .tessellate(tolerance, TessellationOptions::default())
        .unwrap()
}

#[test]
fn a_face_on_its_neighbours_surface_facing_away_is_merged_into_it() {
    // A 10 mm box whose top face also covers a 1 mm strip beyond the front edge (its
    // loop detours around the strip) and a second planar face under the strip facing
    // down: a flap of no thickness. The strip face shares three edges with the top
    // face and its fourth edge with the front face; it lies on the top face's plane
    // and faces away from it, so the top face takes the front edge in its place: the
    // solid is the box, 12 triangles with every corner a box corner.
    for options in [
        ImportOptions::default(),
        ImportOptions {
            strict: true,
            ..ImportOptions::default()
        },
    ] {
        let solid = import(FLAP, options).unwrap();
        assert_eq!(
            solid.adaptations(),
            Adaptations {
                collapsed_faces: 1,
                ..Adaptations::default()
            }
        );
        assert_eq!(solid.face_entities().len(), 6);
        let mesh = mesh(&solid);
        mesh.require_solid().unwrap();
        let data = mesh.data();
        assert_eq!((data.positions.len(), data.triangles.len()), (8, 12));
        assert!(
            data.positions
                .iter()
                .all(|p| p.iter().all(|&x| x == 0. || (x - 0.01).abs() < 1e-15)),
            "{:?}",
            data.positions
        );
        assert!((mesh.statistics().signed_volume - 1e-6).abs() < 1e-18);
        // The strip face is gone from the labels; the top face's strip edges with it.
        assert!(!data.face_ids.contains(&181));
    }
}

#[test]
fn a_face_that_continues_its_neighbour_on_one_surface_is_not_a_lens() {
    // The same box with the strip face facing up like the top face: two faces on one
    // plane, facing the same way, are a fold of the shell, not a lens. The strip
    // face then winds the wrong way around its normal and the face is rejected as
    // written; nothing is merged.
    let folded = FLAP.replace(
        "#177=DIRECTION('',(0.0,0.0,-1.0));",
        "#177=DIRECTION('',(0.0,0.0,1.0));",
    );
    assert_ne!(folded, FLAP);
    let solid = import(&folded, ImportOptions::default()).unwrap();
    assert_eq!(solid.adaptations().collapsed_faces, 0);
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-5).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    let error = solid
        .tessellate(tolerance, TessellationOptions::default())
        .unwrap_err();
    assert_eq!(error.entity, EntityId::new(181));
    assert!(
        error.message.contains("wrong orientation"),
        "{}",
        error.message
    );
}
