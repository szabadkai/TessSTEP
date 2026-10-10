//! Loop spurs: a face loop that runs out along one edge and back along another over
//! the same line encloses no area between them. The authored slab carries two fins
//! flush with its wall: the wall's top edge spans the whole wall, each fin's base
//! edge only its part, so the slab top's loop runs out and back at both fins. The
//! default import splits the wall edge at the fin corners, merges the coincident
//! halves and drops the spurs; strict import keeps the loop, which trimming rejects
//! as tangent edges at a vertex. See docs/CURVED_IMPORT.md.
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const SPURS: &str = include_str!("../../../corpus/geometry/brep-loop-spurs.step");

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
fn tessellate(solid: &ImportedSolid) -> Result<tessstep_mesh::Mesh, Error> {
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-4).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    solid.tessellate(tolerance, TessellationOptions::default())
}

#[test]
fn loop_spurs_collapse_unless_strict() {
    // One spur ends at the fin corner on the wall edge (the next edge runs within the
    // previous one), the other starts there (the previous edge runs within the next):
    // the wall edge is split at both corners, the fin base edges merge into the
    // halves, and the slab top is a plain rectangle again.
    let solid = import(SPURS, ImportOptions::default()).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            split_edges: 2,
            collapsed_spurs: 2,
            ..Adaptations::default()
        }
    );
    let mesh = tessellate(&solid).unwrap();
    mesh.require_solid().unwrap();
    // The first fin's base corner sits 5 nm off the wall edge, which the mesh keeps.
    let volume = mesh.statistics().signed_volume;
    let exact = 536e-9;
    assert!(
        ((volume - exact) / exact).abs() <= 1e-6,
        "volume {volume} exact {exact}"
    );
    // Every face is a polygon of its corners (the wall gains the two fin corners):
    // 18 faces with 80 corners in all give 44 triangles, none of them a sliver.
    assert_eq!(mesh.data().triangles.len(), 44);
    assert_eq!(mesh.statistics().boundary_edges, 0);
    // Strict import keeps the loop as written, and trimming rejects it.
    let kept = import(
        SPURS,
        ImportOptions {
            strict: true,
            ..ImportOptions::default()
        },
    )
    .unwrap();
    assert_eq!(kept.adaptations(), Adaptations::default());
    let error = tessellate(&kept).unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::InvalidGeometry, Stage::Tessellation),
        "{error}"
    );
    assert!(
        error.message.ends_with("(tangent edges at a vertex)"),
        "{error}"
    );
}
