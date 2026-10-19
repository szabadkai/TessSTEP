use std::sync::Arc;
use tessstep_import::*;
use tessstep_math::Point3;
use tessstep_mesh::scene::AssetId;
use tessstep_part21::{EntityId, ParseLimits};
const NESTED: &str = include_str!("../../../corpus/geometry/assembly-nested.step");
const MISSING: &str = include_str!("../../../corpus/geometry/assembly-missing-placement.step");
const UNPLACED: &str = include_str!("../../../corpus/geometry/assembly-unplaced.step");
const INCH: &str = include_str!("../../../corpus/geometry/assembly-inch.step");

fn parse(text: &str) -> tessstep_model::Document {
    tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap()
}

/// World bounding boxes of every asset instance, in millimetres, sorted.
fn boxes(assembly: &LinkedAssembly) -> Vec<[i64; 6]> {
    let scene = assembly.scene();
    let mut out = Vec::new();
    for resolved in scene.instances() {
        let Some(asset) = resolved.source.asset.and_then(|a| scene.asset(a)) else {
            continue;
        };
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for &p in &asset.mesh.data().positions {
            let w = resolved
                .world_transform
                .transform_point(Point3::new(p).unwrap())
                .unwrap()
                .coordinates();
            for k in 0..3 {
                lo[k] = lo[k].min(w[k]);
                hi[k] = hi[k].max(w[k]);
            }
        }
        let mm = |x: f64| (x * 1e3).round() as i64;
        out.push([
            mm(lo[0]),
            mm(lo[1]),
            mm(lo[2]),
            mm(hi[0]),
            mm(hi[1]),
            mm(hi[2]),
        ]);
    }
    out.sort();
    out
}

#[test]
fn nested_occurrences_compose_their_placements() {
    let doc = parse(NESTED);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    assert!(imported.roots.iter().all(|r| r.result.is_ok()));
    assert_eq!(imported.roots.len(), 1, "one block solid, used four times");
    let assembly = &imported.assembly;
    assert!(assembly.is_complete());
    assert_eq!(assembly.occurrence_count(), 4);
    let mut names: Vec<_> = assembly
        .products()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    names.sort();
    assert_eq!(names, ["block", "pair", "rig"]);
    // rig, two pairs, four blocks; four asset leaves share one asset.
    let definitions = assembly.nodes().iter().filter(|n| n.root.is_none()).count();
    assert_eq!(definitions, 7);
    assert_eq!(assembly.scene().assets().len(), 1);
    assert_eq!(
        boxes(assembly),
        [
            [0, 0, 0, 10, 20, 30],
            [0, 40, 0, 10, 60, 30],
            // The second pair is turned a quarter about z and moved 100 mm along x.
            [40, 0, 0, 60, 10, 30],
            [80, 0, 0, 100, 10, 30],
        ]
    );
    let occurrences = assembly
        .nodes()
        .iter()
        .filter(|n| n.occurrence.is_some())
        .count();
    assert_eq!(occurrences, 6, "two pair and four block occurrence nodes");
    assert!(assembly.unplaced().is_empty() && assembly.unimported().is_empty());
}

#[test]
fn an_unplaced_occurrence_fails_without_a_default() {
    let doc = parse(MISSING);
    let e = import_assembly(&doc, AssemblyImportOptions::default()).unwrap_err();
    assert_eq!(
        (e.stage, e.kind),
        (Stage::Product, ErrorKind::InvalidGeometry)
    );
    assert!(e.message.contains("MissingPlacement"), "{e}");
}

#[test]
fn solids_outside_product_shapes_are_reported_unplaced() {
    let doc = parse(UNPLACED);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    let assembly = &imported.assembly;
    assert_eq!(imported.roots.len(), 2);
    assert_eq!(assembly.unplaced().len(), 1);
    assert_eq!(boxes(assembly), [[0, 0, 0, 10, 20, 30]]);
    // Linking without meshes lists the placed root as unimported, never placed.
    let linked = link_assembly(&doc, &[], AssemblyOptions::default()).unwrap();
    assert_eq!(linked.unimported().len(), 1);
    assert!(
        linked
            .scene()
            .instances()
            .iter()
            .all(|i| i.source.asset.is_none())
    );
}

#[test]
fn starred_conversion_units_are_tolerated_by_default_and_rejected_in_strict_mode() {
    let doc = parse(INCH);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    assert_eq!(
        boxes(&imported.assembly),
        [[0, 0, 0, 254, 508, 762], [0, 1016, 0, 254, 1524, 762]]
    );
    let counted = ToleratedEncodings {
        omitted_conversion_dimensions: 1,
        ..ToleratedEncodings::default()
    };
    assert_eq!(imported.assembly.tolerated(), counted);
    assert!(imported.roots.iter().all(|r| r.tolerated == counted));
    let mut strict = AssemblyImportOptions::default();
    strict.assembly.import.strict = true;
    // Strict mode rejects the unit in the solid's context and in every product
    // record that reaches it: the solid is not imported and the structure is
    // incomplete, each with a located typed reason.
    let rejected = import_assembly(&doc, strict).unwrap();
    let starred = |e: &Error| {
        e.kind == ErrorKind::InvalidGeometry
            && e.entity.map(EntityId::get) == Some(6)
            && e.message.contains("derived marker")
    };
    assert!(
        rejected
            .roots
            .iter()
            .all(|r| r.result.as_ref().is_err_and(starred))
    );
    assert!(!rejected.assembly.is_complete());
    assert!(
        rejected
            .assembly
            .excluded()
            .iter()
            .all(|x| starred(&x.error))
    );
}

#[test]
fn supplied_shapes_are_keyed_by_root() {
    let doc = parse(NESTED);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    let asset = imported.assembly.scene().assets()[0].clone();
    let root = EntityId::new(asset.id.0).unwrap();
    let shape = ShapeAsset {
        root,
        shell: None,
        mesh: Arc::clone(&asset.mesh),
    };
    let e = link_assembly(
        &doc,
        &[shape.clone(), shape.clone()],
        AssemblyOptions::default(),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::InvalidOptions);
    let linked = link_assembly(&doc, &[shape], AssemblyOptions::default()).unwrap();
    assert_eq!(boxes(&linked), boxes(&imported.assembly));
    assert!(linked.scene().asset(AssetId(root.get())).is_some());
}

const STYLED: &str = include_str!("../../../corpus/geometry/assembly-styled.step");

#[test]
fn surface_styles_colour_solids_and_faces() {
    let doc = parse(STYLED);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    let styles = import_appearance(&doc, &imported.assembly, StyleOptions::default()).unwrap();
    let counts = styles.counts();
    assert_eq!(
        (counts.styled_items, counts.asset_styles, counts.face_styles),
        (3, 1, 1)
    );
    assert_eq!(
        (counts.no_surface_colour, counts.layers, counts.conflicts),
        (1, 1, 0)
    );
    let appearance = styles.appearance();
    assert_eq!(appearance.materials().len(), 2);
    let linear = |v: f64| ((v + 0.055) / 1.055_f64).powf(2.4);
    let red = [linear(0.8), linear(0.2), linear(0.2), 0.75];
    let leaf = imported
        .assembly
        .nodes()
        .iter()
        .find(|n| n.root.is_some())
        .unwrap();
    let asset = imported.assembly.scene().assets()[0].clone();
    let data = asset.mesh.data();
    // The top face (z = 30 mm) is blue; every other face is the translucent red.
    for (t, triangle) in data.triangles.iter().enumerate() {
        let resolved = appearance
            .resolve_triangle(leaf.instance, t)
            .unwrap()
            .unwrap();
        let colour = appearance
            .material(resolved.material)
            .unwrap()
            .color
            .components();
        let top = triangle
            .iter()
            .all(|&v| (data.positions[v as usize][2] - 0.03).abs() < 1e-12);
        if top {
            assert_eq!(colour, [0., 0., 1., 1.]);
        } else {
            for k in 0..4 {
                assert!((colour[k] - red[k]).abs() < 1e-12, "{colour:?}");
            }
        }
    }
}

const MAPPED: &str = include_str!("../../../corpus/geometry/assembly-mapped.step");
const OPERATOR: &str = include_str!("../../../corpus/geometry/assembly-operator.step");
const SI_DIMENSIONS: &str = include_str!("../../../corpus/geometry/assembly-si-dimensions.step");
const UNTYPED: &str = include_str!("../../../corpus/geometry/assembly-untyped-measure.step");

/// A fixture with one exact record text replaced.
fn edit(text: &str, old: &str, new: &str) -> String {
    assert!(text.contains(old), "{old}");
    text.replace(old, new)
}

fn strict() -> AssemblyImportOptions {
    let mut options = AssemblyImportOptions::default();
    options.assembly.import.strict = true;
    options
}

#[test]
fn mapped_items_place_occurrences_by_target_times_inverse_origin() {
    let doc = parse(MAPPED);
    for options in [AssemblyImportOptions::default(), strict()] {
        let imported = import_assembly(&doc, options).unwrap();
        assert!(imported.roots.iter().all(|r| r.result.is_ok()));
        let assembly = &imported.assembly;
        assert!(assembly.is_complete());
        assert_eq!(assembly.occurrence_count(), 4);
        assert_eq!(assembly.mapped_placements(), 6, "two pairs and four blocks");
        assert_eq!(assembly.tolerated(), ToleratedEncodings::default());
        // `pair` is mapped from its second block frame, 40 mm along y: in `rig` its
        // blocks sit 40 mm lower, and the turned copy turns about that frame.
        assert_eq!(
            boxes(assembly),
            [
                [0, -40, 0, 10, -20, 30],
                [0, 0, 0, 10, 20, 30],
                [80, 0, 0, 100, 10, 30],
                [120, 0, 0, 140, 10, 30],
            ]
        );
    }
    // Without the occurrence's own representation no item says which occurrence it
    // places, so the occurrence has no placement; two qualifying items are ambiguous.
    for (text, kind) in [
        (
            edit(MAPPED, "#96=SHAPE_DEFINITION_REPRESENTATION(#94,#95);", ""),
            "MissingPlacement",
        ),
        (
            edit(MAPPED, "('block-1',(#80),#8)", "('block-1',(#80,#82),#8)"),
            "AmbiguousPlacement",
        ),
    ] {
        let e = link_assembly(&parse(&text), &[], AssemblyOptions::default()).unwrap_err();
        assert_eq!(
            (e.stage, e.kind),
            (Stage::Product, ErrorKind::InvalidGeometry)
        );
        assert!(e.message.contains(kind), "{e}");
    }
}

#[test]
fn cartesian_operators_place_with_reflection_and_uniform_scale() {
    let doc = parse(OPERATOR);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    let assembly = &imported.assembly;
    assert!(assembly.is_complete());
    assert_eq!(
        (assembly.occurrence_count(), assembly.mapped_placements()),
        (3, 1)
    );
    // Mirrored in y; scaled by 2 at 100 mm along x; the mapped item's target operator
    // scales by 0.5 at 100 mm along y.
    assert_eq!(
        boxes(assembly),
        [
            [0, -20, 0, 10, 0, 30],
            [0, 100, 0, 5, 110, 15],
            [100, 0, 0, 120, 40, 60]
        ]
    );
    let scales: Vec<(bool, i64)> = assembly
        .scene()
        .instances()
        .iter()
        .filter(|i| i.source.asset.is_some())
        .map(|i| {
            let l = i.world_transform.linear();
            let column = (l[0][0].powi(2) + l[1][0].powi(2) + l[2][0].powi(2)).sqrt();
            (i.mirrored, (column * 1e3).round() as i64)
        })
        .collect();
    let mut sorted = scales.clone();
    sorted.sort();
    assert_eq!(sorted, [(false, 500), (false, 2000), (true, 1000)]);
    // Operators the scene cannot place as evaluated similarities are typed rejections
    // that name the operator.
    let flat = edit(
        OPERATOR,
        "#89=CARTESIAN_TRANSFORMATION_OPERATOR_3D('','',$,#85,#86,#88,$,#87);",
        "#89=CARTESIAN_TRANSFORMATION_OPERATOR_2D('','',$,#85,#86,#88,$);",
    );
    let e = link_assembly(&parse(&flat), &[], AssemblyOptions::default()).unwrap_err();
    assert_eq!((e.stage, e.kind), (Stage::Product, ErrorKind::Unsupported));
    assert_eq!(e.entity.map(EntityId::get), Some(89));
    assert!(
        e.message.contains("MissingPlacement")
            && e.message
                .contains("CARTESIAN_TRANSFORMATION_OPERATOR_2D is outside"),
        "{e}"
    );
    let negative = edit(OPERATOR, "#90,2.0,$)", "#90,-2.0,$)");
    let e = link_assembly(&parse(&negative), &[], AssemblyOptions::default()).unwrap_err();
    assert_eq!(
        (e.stage, e.kind, e.entity.map(EntityId::get)),
        (Stage::Product, ErrorKind::InvalidGeometry, Some(91))
    );
    assert!(e.message.contains("operator scale"), "{e}");
}

/// The tolerant default imports with the encodings counted in the product records,
/// the discovered context units and every root; strict excludes every product record
/// reaching the units and imports no root.
fn tolerated_then_rejected(text: &str, expected: ToleratedEncodings, boxes_mm: &[[i64; 6]]) {
    let doc = parse(text);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    assert!(imported.assembly.is_complete());
    assert_eq!(boxes(&imported.assembly), boxes_mm);
    assert_eq!(imported.assembly.tolerated(), expected);
    assert!(
        imported
            .roots
            .iter()
            .all(|r| r.result.is_ok() && r.tolerated == expected)
    );
    let solids = discover_solids(&doc, ImportOptions::default()).unwrap();
    assert_eq!(solids[0].units.as_ref().unwrap().tolerated, expected);
    let rejected = import_assembly(&doc, strict()).unwrap();
    assert!(!rejected.assembly.is_complete());
    assert!(rejected.assembly.nodes().is_empty());
    assert!(rejected.roots.iter().all(|r| r.result.is_err()));
    let solids = discover_solids(&doc, strict().assembly.import).unwrap();
    assert!(solids[0].units.is_err());
}

#[test]
fn explicit_si_dimensions_and_unordered_unit_components_are_counted() {
    let pair = [[0, 0, 0, 10, 20, 30], [0, 40, 0, 10, 60, 30]];
    tolerated_then_rejected(
        SI_DIMENSIONS,
        ToleratedEncodings {
            explicit_si_dimensions: 3,
            unordered_complex: 2,
            ..ToleratedEncodings::default()
        },
        &pair,
    );
    // In canonical component order strict still rejects the explicit exponents.
    let ordered = edit(
        &edit(
            SI_DIMENSIONS,
            "#5=(NAMED_UNIT(#4)LENGTH_UNIT()SI_UNIT(",
            "#5=(LENGTH_UNIT()NAMED_UNIT(#4)SI_UNIT(",
        ),
        "#8=(NAMED_UNIT(#6)SOLID_ANGLE_UNIT()SI_UNIT($,.STERADIAN.))",
        "#8=(NAMED_UNIT(#6)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())",
    );
    tolerated_then_rejected(
        &ordered,
        ToleratedEncodings {
            explicit_si_dimensions: 3,
            ..ToleratedEncodings::default()
        },
        &pair,
    );
    let e = import_assembly(&parse(&ordered), strict())
        .unwrap()
        .assembly
        .excluded()[0]
        .error
        .clone();
    assert!(e.message.contains("requires a derived marker"), "{e}");
    // Explicit exponents must agree with the unit role.
    let wrong = edit(
        SI_DIMENSIONS,
        "#6=DIMENSIONAL_EXPONENTS(0.,0.,0.,0.,0.,0.,0.)",
        "#6=DIMENSIONAL_EXPONENTS(0.,0.,1.,0.,0.,0.,0.)",
    );
    let doc = parse(&wrong);
    let e = discover_solids(&doc, ImportOptions::default()).unwrap()[0]
        .units
        .clone()
        .unwrap_err();
    assert!(e.message.contains("SI unit dimensions disagree"), "{e}");
    let e = link_assembly(&doc, &[], AssemblyOptions::default()).unwrap_err();
    assert!(
        e.message.contains("explicit SI unit dimensions disagree"),
        "{e}"
    );
}

#[test]
fn untyped_measure_values_are_counted_and_keep_their_unit() {
    // The bare conversion factor still makes the unit an inch.
    tolerated_then_rejected(
        UNTYPED,
        ToleratedEncodings {
            untyped_measures: 2,
            ..ToleratedEncodings::default()
        },
        &[[0, 0, 0, 254, 508, 762], [0, 1016, 0, 254, 1524, 762]],
    );
    let e = import_assembly(&parse(UNTYPED), strict())
        .unwrap()
        .assembly
        .excluded()[0]
        .error
        .clone();
    assert!(
        e.message.contains("not a permitted SELECT alternative"),
        "{e}"
    );
}

const REVERSED: &str = include_str!("../../../corpus/geometry/assembly-reversed-items.step");

#[test]
fn reversed_item_transformations_are_read_by_membership_and_counted() {
    let doc = parse(REVERSED);
    let imported = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    assert!(imported.roots.iter().all(|r| r.result.is_ok()));
    let assembly = &imported.assembly;
    assert!(assembly.is_complete());
    assert_eq!(
        (assembly.occurrence_count(), assembly.mapped_placements()),
        (4, 0)
    );
    // The draughting models' mapped items are presentation, not placements.
    assert_eq!(
        assembly.tolerated(),
        ToleratedEncodings {
            reversed_transform_items: 4,
            ..ToleratedEncodings::default()
        }
    );
    // Read by membership, each placement maps the child onto the parent's frame, so
    // the world boxes are those of the conformant nested fixture; read as written,
    // the second block would sit 40 mm below the first.
    let nested = import_assembly(&parse(NESTED), AssemblyImportOptions::default()).unwrap();
    assert_eq!(boxes(assembly), boxes(&nested.assembly));
    let mut strict = AssemblyOptions::default();
    strict.import.strict = true;
    let e = link_assembly(&doc, &[], strict).unwrap_err();
    assert_eq!(
        (e.stage, e.kind),
        (Stage::Product, ErrorKind::InvalidGeometry)
    );
    assert!(e.message.contains("reverse order"), "{e}");
    let relationship = doc.entities().get(e.entity.unwrap()).unwrap();
    assert!(
        relationship
            .kind
            .records()
            .iter()
            .any(|r| r.name.as_ref() == "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION")
    );
}

const SURFACE: &str = include_str!("../../../corpus/geometry/assembly-surface.step");
const SURFACE_ONLY: &str = include_str!("../../../corpus/geometry/assembly-surface-only.step");
const UNSEWN: &str = include_str!("../../../corpus/geometry/surface-unsewn.step");
const PATCHES: &str = include_str!("../../../corpus/geometry/surface-open-patches.step");

fn with_surface_models() -> AssemblyImportOptions {
    let mut options = AssemblyImportOptions::default();
    options.assembly.surface_models = true;
    options
}

fn id(n: u64) -> EntityId {
    EntityId::new(n).unwrap()
}

/// The surface model of `root` imported alone, at the model tolerance the assembly
/// import gives it (the 1e-8 m context uncertainty floored at 1e-7 m).
fn free_surface_model(
    doc: &tessstep_model::Document,
    root: u64,
    options: ImportOptions,
) -> Result<ImportedSurfaceModel, Error> {
    let units = discover_surface_models(doc, options).unwrap()[0]
        .units
        .clone()
        .unwrap();
    let tolerance = tessstep_math::ModelTolerance::new(
        tessstep_math::Length::metres(1e-7).unwrap(),
        tessstep_math::Angle::radians(1e-8).unwrap(),
    )
    .unwrap();
    import_surface_model(
        doc,
        id(root),
        units.length,
        units.plane_angle,
        tolerance,
        options,
    )
}

#[test]
fn surface_models_join_an_assembly_only_on_request() {
    let doc = parse(SURFACE);
    // By default the open trough is neither imported, placed nor listed as unimported.
    let plain = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    assert_eq!(
        plain.roots.iter().map(|r| r.kind).collect::<Vec<_>>(),
        [ShapeRootKind::Solid(SolidKind::FacetedBrep)]
    );
    assert_eq!(boxes(&plain.assembly), [[0, 0, 0, 10, 20, 30]]);
    assert!(plain.assembly.unplaced().is_empty() && plain.assembly.unimported().is_empty());
    assert!(plain.assembly.nodes().iter().all(|n| n.shell.is_none()));
    let linked = link_assembly(&doc, &[], AssemblyOptions::default()).unwrap();
    assert_eq!(linked.unimported(), [id(60)]);

    // On request the surface model is placed through both of its occurrences, like a
    // solid: 50 mm along x, and turned a quarter about z at 100 mm along x.
    let imported = import_assembly(&doc, with_surface_models()).unwrap();
    let reports: Vec<_> = imported
        .roots
        .iter()
        .map(|r| (r.root.get(), r.kind, r.selected, r.result.is_ok()))
        .collect();
    assert_eq!(
        reports,
        [
            (60, ShapeRootKind::Solid(SolidKind::FacetedBrep), true, true),
            (176, ShapeRootKind::SurfaceModel, true, true)
        ]
    );
    let assembly = &imported.assembly;
    assert!(assembly.is_complete());
    assert_eq!(
        boxes(assembly),
        [
            [0, 0, 0, 10, 20, 30],
            [35, 0, 0, 65, 10, 20],
            [90, -15, 0, 100, 15, 20]
        ]
    );
    // One asset per shell, keyed by the shell entity; its leaves name root and shell.
    let ids: Vec<u64> = assembly.scene().assets().iter().map(|a| a.id.0).collect();
    assert_eq!(ids, [60, 175]);
    let leaves: Vec<_> = assembly
        .nodes()
        .iter()
        .filter(|n| n.shell.is_some())
        .map(|n| (n.root, n.shell, n.definition.get(), n.representation.get()))
        .collect();
    assert_eq!(leaves, [(Some(id(176)), Some(id(175)), 184, 181); 2]);
    assert!(assembly.unplaced().is_empty() && assembly.unimported().is_empty());
    assert_eq!(imported.asset_class(AssetId(60)), Some(AssetClass::Solid));
    assert_eq!(
        imported.asset_class(AssetId(175)),
        Some(AssetClass::OpenShell)
    );
    assert_eq!(imported.asset_class(AssetId(176)), None);
    // A consumer that makes bodies of surface models only without solids skips them.
    assert!(imported.has_solid_roots());
    assert!(!ShapeRootKind::SurfaceModel.is_solid());
    // Linking without meshes lists the surface model as unimported only on request,
    // and a supplied shell asset is placed by its root.
    let mut options = AssemblyOptions {
        surface_models: true,
        ..AssemblyOptions::default()
    };
    let linked = link_assembly(&doc, &[], options).unwrap();
    assert_eq!(linked.unimported(), [id(60), id(176)]);
    let shell = ShapeAsset {
        root: id(176),
        shell: Some(id(175)),
        mesh: Arc::clone(&assembly.scene().asset(AssetId(175)).unwrap().mesh),
    };
    assert_eq!(shell.asset(), AssetId(175));
    let linked = link_assembly(&doc, std::slice::from_ref(&shell), options).unwrap();
    assert_eq!(boxes(&linked), boxes(assembly)[1..]);
    assert_eq!(linked.unimported(), [id(60)]);
    let e = link_assembly(&doc, &[shell.clone(), shell], options).unwrap_err();
    assert_eq!(
        (e.kind, e.entity, e.message.as_str()),
        (
            ErrorKind::InvalidOptions,
            Some(id(175)),
            "duplicate surface-model shell"
        )
    );
    options.surface_models = false;
    assert_eq!(
        link_assembly(&doc, &[], options).unwrap().unimported(),
        [id(60)]
    );
}

#[test]
fn a_closed_surface_model_imports_as_a_watertight_shell_asset_not_a_solid() {
    let doc = parse(SURFACE_ONLY);
    // Without surface models the part has a structure and no body, and the style of
    // its surface model has no asset to colour.
    let plain = import_assembly(&doc, AssemblyImportOptions::default()).unwrap();
    assert!(plain.roots.is_empty() && plain.assembly.scene().assets().is_empty());
    assert_eq!(plain.assembly.products().len(), 1);
    let styles = import_appearance(&doc, &plain.assembly, StyleOptions::default()).unwrap();
    assert_eq!(
        (styles.counts().unimported, styles.counts().asset_styles),
        (1, 0)
    );

    let options = with_surface_models();
    let imported = import_assembly(&doc, options).unwrap();
    assert_eq!(imported.roots.len(), 2);
    assert!(
        imported
            .roots
            .iter()
            .all(|r| r.kind == ShapeRootKind::SurfaceModel && r.selected && r.result.is_ok())
    );
    let assembly = &imported.assembly;
    // The closed cylinder shell is placed by its part; the loose open model is
    // unplaced, its asset kept without an instance, as an unplaced solid is.
    assert_eq!(boxes(assembly), [[-10, -10, 0, 10, 10, 20]]);
    assert_eq!(assembly.unplaced(), [id(172)]);
    assert!(assembly.unimported().is_empty());
    // No solid root: here a no-solids rule keeps the shells as bodies.
    assert!(!imported.has_solid_roots());
    assert_eq!(
        imported.asset_class(AssetId(55)),
        Some(AssetClass::ClosedShell)
    );
    assert_eq!(
        imported.asset_class(AssetId(171)),
        Some(AssetClass::OpenShell)
    );
    let skin = assembly.scene().asset(AssetId(55)).unwrap();
    let stats = skin.mesh.statistics();
    assert!(skin.mesh.is_watertight());
    assert_eq!(stats.components, 1);
    // Volume 2000 pi mm3 within the 0.1 mm chord over the 10 mm radius.
    let exact = 2000. * std::f64::consts::PI * 1e-9;
    assert!(((stats.signed_volume - exact) / exact).abs() <= 2. * 1e-4 / 0.01);
    // The surface model's style colours its shell asset.
    let styles = import_appearance(&doc, assembly, StyleOptions::default()).unwrap();
    assert_eq!(
        (styles.counts().unimported, styles.counts().asset_styles),
        (0, 1)
    );
    let leaf = assembly.nodes().iter().find(|n| n.shell.is_some()).unwrap();
    let appearance = styles.appearance();
    let resolved = appearance
        .resolve_triangle(leaf.instance, 0)
        .unwrap()
        .unwrap();
    let linear = |v: f64| ((v + 0.055) / 1.055_f64).powf(2.4);
    let colour = appearance
        .material(resolved.material)
        .unwrap()
        .color
        .components();
    let red = [linear(0.8), linear(0.2), linear(0.2), 1.];
    assert!(
        (0..4).all(|k| (colour[k] - red[k]).abs() < 1e-12),
        "{colour:?}"
    );
    // The shell mesh is the free import's.
    let free = free_surface_model(&doc, 56, ImportOptions::default()).unwrap();
    let mesh = free.shells()[0]
        .tessellate(options.tolerance, options.tessellation)
        .unwrap();
    assert_eq!(
        format!("{:?}", mesh.data()),
        format!("{:?}", skin.mesh.data())
    );
}

#[test]
fn surface_model_failures_are_reported_per_root_with_their_text() {
    let doc = parse(UNSEWN);
    let imported = import_assembly(&doc, with_surface_models()).unwrap();
    let outcomes: Vec<_> = imported
        .roots
        .iter()
        .map(|r| {
            let failure = r.result.as_ref().err();
            (
                r.root.get(),
                r.kind,
                failure.map(|e| (e.stage, e.kind, e.message.clone())),
            )
        })
        .collect();
    let disconnected = Some((
        Stage::Topology,
        ErrorKind::InvalidGeometry,
        "DisconnectedShell at shell 0".to_string(),
    ));
    assert_eq!(
        outcomes,
        [
            (1000, ShapeRootKind::SurfaceModel, disconnected.clone()),
            (1001, ShapeRootKind::SurfaceModel, None),
            (1002, ShapeRootKind::SurfaceModel, disconnected)
        ]
    );
    // Only the imported model has assets: its two single-face shells.
    let ids: Vec<u64> = imported
        .assembly
        .scene()
        .assets()
        .iter()
        .map(|a| a.id.0)
        .collect();
    assert_eq!(ids, [282, 323]);
    assert_eq!(imported.assembly.unplaced(), [id(1001)]);
    // A shell listed by two surface models is imported once, with the first.
    let text = edit(
        PATCHES,
        "#1001=SHELL_BASED_SURFACE_MODEL('',(#106));",
        "#1001=SHELL_BASED_SURFACE_MODEL('',(#105));",
    );
    let imported = import_assembly(&parse(&text), with_surface_models()).unwrap();
    assert!(imported.roots[0].result.is_ok());
    let e = imported.roots[1].result.clone().unwrap_err();
    assert_eq!(
        (e.stage, e.kind, e.entity, e.message.as_str()),
        (
            Stage::Product,
            ErrorKind::Unsupported,
            Some(id(1001)),
            "shell #105 is already a shell asset of surface model #1000"
        )
    );
    assert_eq!(imported.assembly.scene().assets().len(), 1);
}

#[test]
fn opt_in_repair_sews_each_declared_shell_of_a_surface_model_on_its_own() {
    let doc = parse(UNSEWN);
    let mut options = with_surface_models();
    options.assembly.import.repair = RepairOptions::enabled();
    let imported = import_assembly(&doc, options).unwrap();
    assert!(imported.roots.iter().all(|r| r.result.is_ok()));
    let classes: Vec<_> = imported
        .assembly
        .scene()
        .assets()
        .iter()
        .map(|a| (a.id.0, imported.asset_class(a.id).unwrap()))
        .collect();
    // The unsewn box shell closes, though declared open; the two squares stay two
    // shells, each with its whole boundary; written as one shell they sew into one.
    assert_eq!(
        classes,
        [
            (241, AssetClass::ClosedShell),
            (282, AssetClass::OpenShell),
            (323, AssetClass::OpenShell),
            (404, AssetClass::OpenShell)
        ]
    );
    let scene = imported.assembly.scene();
    let stats = |asset: u64| scene.asset(AssetId(asset)).unwrap().mesh.statistics();
    assert_eq!(stats(241).boundary_edges, 0);
    assert_eq!(stats(241).components, 1);
    assert_eq!(stats(404).components, 1);
    assert_eq!(
        stats(404).boundary_edges,
        stats(282).boundary_edges + stats(323).boundary_edges - 2
    );
    // The single-root import never repairs a surface model.
    let e = free_surface_model(&doc, 1000, options.assembly.import).unwrap_err();
    assert_eq!(e.message, "DisconnectedShell at shell 0");
}
