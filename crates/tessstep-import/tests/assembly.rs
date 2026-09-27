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
