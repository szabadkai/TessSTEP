use tessstep_import::*;
use tessstep_math::LengthUnit;
use tessstep_part21::{EntityId, ParseLimits};
const PRESENTATION: &str = include_str!("../../../corpus/geometry/presentation.step");
const SELECTION: &str = include_str!("../../../corpus/geometry/tessellated-selection.step");
fn parse(text: &str) -> tessstep_model::Document {
    tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap()
}
fn id(n: u64) -> EntityId {
    EntityId::new(n).unwrap()
}
fn import_with(
    text: &str,
    root: u64,
    options: ImportOptions,
    limits: PresentationLimits,
) -> Result<ImportedPresentation, Error> {
    import_presentation(
        &parse(text),
        id(root),
        LengthUnit::MILLIMETRE,
        options,
        limits,
    )
}
fn import(text: &str, root: u64) -> Result<ImportedPresentation, Error> {
    import_with(
        text,
        root,
        ImportOptions::default(),
        PresentationLimits::default(),
    )
}
fn failure(text: &str, root: u64) -> (Stage, ErrorKind, Option<u64>) {
    let e = import(text, root).unwrap_err();
    (e.stage, e.kind, e.entity.map(EntityId::get))
}
fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-15)
}

#[test]
fn annotation_occurrences_import_as_placed_graphics() {
    let p = import(PRESENTATION, 300).unwrap();
    assert_eq!(p.occurrence(), Some(id(300)));
    assert_eq!(p.geometric_set(), id(100));
    assert_eq!(p.styles(), [id(203)]);
    let items: Vec<_> = p
        .items()
        .iter()
        .map(|i| (i.entity.get(), i.kind, i.supplied_normals, i.placements))
        .collect();
    assert_eq!(
        items,
        [
            (110, PresentationItemKind::CurveSet, false, 1),
            (120, PresentationItemKind::SurfaceSet, true, 1),
            (130, PresentationItemKind::PointSet, false, 1),
            (150, PresentationItemKind::CurveSet, false, 2),
        ]
    );
    // Coordinate list #111 is used under two placements: 4 + 5 + 1 + 2 vertices.
    assert_eq!(p.positions().len(), 12);
    assert_eq!(p.polyline_count(), 3);
    assert_eq!(p.polyline_items(), [110, 110, 150]);
    assert_eq!(p.polyline_offsets(), [0, 5, 7, 9]);
    // The placement maps local (x, y, z) to (100 + z, x, y) millimetres.
    let at = |v: u32| p.positions()[v as usize];
    let frame: Vec<[f64; 3]> = p.polyline(0).iter().map(|&v| at(v)).collect();
    for (actual, expected) in frame.iter().zip([
        [0.1, 0., 0.],
        [0.1, 0.01, 0.],
        [0.1, 0.01, 0.005],
        [0.1, 0., 0.005],
        [0.1, 0., 0.],
    ]) {
        assert!(close(*actual, expected), "{actual:?}");
    }
    assert_eq!(p.polyline(0)[0], p.polyline(0)[4]);
    assert_eq!(p.polyline(1), [p.polyline(0)[0], p.polyline(0)[2]]);
    // The nested set composes its own 5 mm offset along the local z axis.
    let offset = p.polyline(2);
    assert!(!p.polyline(0).contains(&offset[0]));
    assert!(close(at(offset[0]), [0.105, 0., 0.]));
    assert!(close(at(offset[1]), [0.105, 0.01, 0.]));
    // The stitching fan triangle is skipped; the collinear one is kept and counted.
    assert_eq!((p.triangles().len(), p.skipped_degenerate()), (3, 1));
    assert_eq!(p.zero_area_triangles(), 1);
    assert_eq!(p.triangle_items(), [120, 120, 120]);
    // The fill's supplied normal lies in the annotation plane and is not used.
    assert_eq!((p.points().len(), p.point_items()), (1, &[130u64][..]));
    assert!(close(at(p.points()[0]), [0.1, 0.005, 0.0025]));
    // The geometric set imports directly with the same graphics and no styles.
    let set = import(PRESENTATION, 100).unwrap();
    assert_eq!((set.occurrence(), set.styles().len()), (None, 0));
    assert_eq!(set.positions(), p.positions());
    assert_eq!(set.triangles(), p.triangles());
    // Record order does not change the result.
    let (header, data) = PRESENTATION.split_once("DATA;\n").unwrap();
    let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
    records.reverse();
    let reordered = format!(
        "{header}DATA;\n{}\nENDSEC;END-ISO-10303-21;",
        records.join("\n")
    );
    assert_eq!(import(&reordered, 300).unwrap(), p);
}

#[test]
fn presentation_rejections_are_typed_and_located() {
    let geometry = |entity| (Stage::Geometry, ErrorKind::InvalidGeometry, Some(entity));
    let unsupported = |entity| (Stage::Geometry, ErrorKind::Unsupported, Some(entity));
    for (from, to, root, expected) in [
        (
            "((1,2,3,4,1),(1,3))",
            "((1,2,2,3),(1,3))",
            300,
            geometry(110),
        ),
        (
            "((1,2,3,4,1),(1,3))",
            "((1,2,3,4,1),(1,5))",
            300,
            geometry(110),
        ),
        ("#131,(1));", "#131,(2));", 300, geometry(130)),
        (
            "#121,5,((1.,0.,0.)),",
            "#121,5,((1.,0.,0.),(0.,1.,0.)),",
            300,
            geometry(120),
        ),
        (
            "#121,5,((1.,0.,0.)),",
            "#121,6,((1.,0.,0.)),",
            300,
            geometry(120),
        ),
        (
            "#104=DIRECTION('',(0.,1.,0.));",
            "#104=DIRECTION('',(1.,0.,0.));",
            300,
            geometry(101),
        ),
        // An occurrence must style a geometric set.
        (
            "('orphan',(),#140)",
            "('orphan',(),#150)",
            301,
            unsupported(301),
        ),
        // Solids and shells are shape tessellations, not presentation children.
        (
            "#130=TESSELLATED_POINT_SET('centre',#131,(1));",
            "#130=TESSELLATED_SHELL('shell',(#132),$);\
             #132=TRIANGULATED_FACE('',#131,1,(),$,(),((1,1,1)));",
            300,
            unsupported(130),
        ),
    ] {
        let text = PRESENTATION.replace(from, to);
        assert_ne!(text, PRESENTATION, "{from}");
        assert_eq!(failure(&text, root), expected, "{to}");
    }
    // A PNMAX below NPOINTS without PNINDEX is a tolerated, counted deviation.
    let understated = PRESENTATION.replace("#121,5,((1.,0.,0.)),", "#121,4,((1.,0.,0.)),");
    let tolerated = import(&understated, 300).unwrap();
    assert_eq!(tolerated.pnmax_deviations(), 1);
    assert_eq!(
        tolerated.triangles(),
        import(PRESENTATION, 300).unwrap().triangles()
    );
    let strict = ImportOptions {
        strict: true,
        ..ImportOptions::default()
    };
    assert_eq!(
        import_with(&understated, 300, strict, PresentationLimits::default())
            .unwrap_err()
            .entity,
        Some(id(120))
    );
    // Curve sets are not presentation roots, and outside-profile children are named.
    assert_eq!(failure(PRESENTATION, 110), unsupported(110));
    let wire = PRESENTATION.replace(
        "#130=TESSELLATED_POINT_SET('centre',#131,(1));",
        "#130=TESSELLATED_WIRE('wire',(#131),$);",
    );
    let e = import(&wire, 300).unwrap_err();
    assert_eq!((e.stage, e.kind), (Stage::Profile, ErrorKind::Unsupported));
    assert!(e.message.contains("TESSELLATED_WIRE is outside"), "{e}");
    // Every budget is explicit.
    for (options, limits) in [
        (
            ImportOptions {
                max_work: 0,
                ..ImportOptions::default()
            },
            PresentationLimits::default(),
        ),
        (
            ImportOptions {
                max_records: 5,
                ..ImportOptions::default()
            },
            PresentationLimits::default(),
        ),
        (
            ImportOptions::default(),
            PresentationLimits {
                max_vertices: 11,
                ..PresentationLimits::default()
            },
        ),
        (
            ImportOptions::default(),
            PresentationLimits {
                max_triangles: 2,
                ..PresentationLimits::default()
            },
        ),
        (
            ImportOptions::default(),
            PresentationLimits {
                max_polyline_points: 8,
                ..PresentationLimits::default()
            },
        ),
        (
            ImportOptions::default(),
            PresentationLimits {
                max_depth: 1,
                ..PresentationLimits::default()
            },
        ),
    ] {
        assert_eq!(
            import_with(PRESENTATION, 300, options, limits)
                .unwrap_err()
                .kind,
            ErrorKind::ResourceLimit
        );
    }
    let mut seed = 71u64;
    for _ in 0..300 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut bytes = PRESENTATION.as_bytes().to_vec();
        let at = seed as usize % bytes.len();
        bytes[at] = (seed >> 32) as u8;
        if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
            let _ = import_presentation(
                &doc,
                id(300),
                LengthUnit::MILLIMETRE,
                ImportOptions {
                    max_work: 100_000,
                    max_records: 1000,
                    ..ImportOptions::default()
                },
                PresentationLimits::default(),
            );
        }
    }
}

#[test]
fn presentation_discovery_follows_callouts_into_draughting_models() {
    let roots = discover_presentations(&parse(PRESENTATION), ImportOptions::default()).unwrap();
    assert_eq!(roots.len(), 2);
    let note = &roots[0];
    assert_eq!(note.entity, id(300));
    assert_eq!(note.containers, [id(310)]);
    // A plain and a characterized complex draughting model both list the callout.
    assert_eq!(note.representations, [id(320), id(321)]);
    let units = note.units.clone().unwrap();
    assert_eq!(units.length, LengthUnit::MILLIMETRE);
    assert!((units.distance_uncertainty.unwrap() - 1e-8).abs() < 1e-22);
    let orphan = &roots[1];
    assert_eq!(
        (
            orphan.entity,
            orphan.containers.len(),
            orphan.representations.len()
        ),
        (id(301), 0, 0)
    );
    assert!(orphan.units.is_err());
    assert_eq!(
        discover_presentations(
            &parse(PRESENTATION),
            ImportOptions {
                max_work: 10,
                ..ImportOptions::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::ResourceLimit
    );
}

fn selection(
    text: &str,
    preference: RepresentationPreference,
) -> (
    Vec<SolidRoot>,
    Vec<TessellatedRoot>,
    Vec<RepresentationChoice>,
) {
    let doc = parse(text);
    let solids = discover_solids(&doc, ImportOptions::default()).unwrap();
    let tessellations = discover_tessellations(&doc, ImportOptions::default()).unwrap();
    let choices = select_representations(
        &doc,
        &solids,
        &tessellations,
        preference,
        ImportOptions::default(),
    )
    .unwrap();
    (solids, tessellations, choices)
}
fn choice(
    selected: RootRef,
    alternatives: &[RootRef],
    pairing: Option<Pairing>,
) -> RepresentationChoice {
    RepresentationChoice {
        selected,
        alternatives: alternatives.to_vec(),
        pairing,
    }
}

#[test]
fn tessellated_discovery_and_representation_selection() {
    use RootRef::{Solid, Tessellation};
    let (solids, tessellations, exact) = selection(SELECTION, RepresentationPreference::Exact);
    let found: Vec<_> = solids.iter().map(|s| (s.entity.get(), s.kind)).collect();
    assert_eq!(
        found,
        [
            (1000, SolidKind::ManifoldSolidBrep),
            (11000, SolidKind::FacetedBrep)
        ]
    );
    // The surface set inside a geometric set (#40003) is graphics, not a shape root.
    let found: Vec<_> = tessellations
        .iter()
        .map(|t| (t.entity.get(), t.kind, t.link.map(EntityId::get)))
        .collect();
    assert_eq!(
        found,
        [
            (21000, TessellatedKind::Solid, Some(1000)),
            (31000, TessellatedKind::Shell, Some(30021)),
            (40002, TessellatedKind::SurfaceSet, None),
        ]
    );
    for t in &tessellations {
        assert_eq!(t.units.clone().unwrap().length, LengthUnit::MILLIMETRE);
        assert_eq!(t.representations.len(), 1);
    }
    assert_eq!(
        exact,
        [
            choice(Solid(0), &[Tessellation(0)], Some(Pairing::Link)),
            choice(Solid(1), &[Tessellation(1)], Some(Pairing::Relationship)),
            choice(Tessellation(2), &[], None),
        ]
    );
    let (_, _, tessellated) = selection(SELECTION, RepresentationPreference::Tessellated);
    assert_eq!(
        tessellated,
        [
            choice(Tessellation(0), &[Solid(0)], Some(Pairing::Link)),
            choice(Tessellation(1), &[Solid(1)], Some(Pairing::Relationship)),
            choice(Tessellation(2), &[], None),
        ]
    );
    // Both representations of the linked pair import to the same box volume.
    let doc = parse(SELECTION);
    let units = solids[0].units.clone().unwrap();
    let model = tessstep_math::ModelTolerance::new(
        tessstep_math::Length::metres(1e-7).unwrap(),
        tessstep_math::Angle::radians(1e-8).unwrap(),
    )
    .unwrap();
    let brep = import_planar_solid(
        &doc,
        id(1000),
        units.length,
        model,
        ImportOptions::default(),
    )
    .unwrap()
    .tessellate(
        TessellationTolerance::new(
            tessstep_math::Length::metres(1e-5).unwrap(),
            tessstep_math::Angle::radians(0.1).unwrap(),
        )
        .unwrap(),
        TessellationOptions::default(),
    )
    .unwrap();
    let mesh = import_tessellated(
        &doc,
        id(21000),
        units.length,
        ImportOptions::default(),
        tessstep_mesh::Limits::default(),
    )
    .unwrap();
    let (a, b) = (
        brep.statistics().signed_volume,
        mesh.mesh().statistics().signed_volume,
    );
    assert!(
        (a - 6e-6).abs() < 1e-17 && (b - 6e-6).abs() < 1e-17,
        "{a} {b}"
    );
    // A representation holding two tessellations is ambiguous and never paired.
    let ambiguous = SELECTION.replace("('shell only',(#31000),", "('shell only',(#31000,#40002),");
    let (_, _, choices) = selection(&ambiguous, RepresentationPreference::Exact);
    assert_eq!(
        choices,
        [
            choice(Solid(0), &[Tessellation(0)], Some(Pairing::Link)),
            choice(Solid(1), &[], None),
            choice(Tessellation(1), &[], None),
            choice(Tessellation(2), &[], None),
        ]
    );
    // A shell linked to a solid's outer shell pairs with that solid by link.
    let shell = SELECTION.replace("(#30100,#30101),#30021);", "(#30100,#30101),#900);");
    assert_ne!(shell, SELECTION);
    let (_, _, choices) = selection(&shell, RepresentationPreference::Tessellated);
    assert_eq!(
        choices,
        [
            choice(
                Tessellation(0),
                &[Solid(0), Tessellation(1)],
                Some(Pairing::Link)
            ),
            choice(Solid(1), &[], None),
            choice(Tessellation(2), &[], None),
        ]
    );
    // A transformed relationship places a representation; it is not an alternative.
    let placed = SELECTION.replace(
        "#50013=REPRESENTATION_RELATIONSHIP('','',#50011,#50012);",
        "#50013=(REPRESENTATION_RELATIONSHIP('','',#50011,#50012)\
         REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#50014)\
         SHAPE_REPRESENTATION_RELATIONSHIP());#50014=ITEM_DEFINED_TRANSFORMATION('','',#104,#104);",
    );
    let (_, _, choices) = selection(&placed, RepresentationPreference::Exact);
    assert_eq!(choices[1], choice(Solid(1), &[], None));
}
