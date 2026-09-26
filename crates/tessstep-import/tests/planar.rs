use tessstep_import::*;
use tessstep_math::{Angle, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};
const BOX: &str = include_str!("../../../corpus/geometry/planar-box.step");
fn model_tolerance() -> ModelTolerance {
    ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap()
}
fn import(text: &str) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_planar_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        model_tolerance(),
        ImportOptions::default(),
    )
}
fn mesh(solid: &ImportedSolid) -> tessstep_mesh::Mesh {
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-6).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    solid
        .tessellate(tolerance, TessellationOptions::default())
        .unwrap()
}
fn check_box(mesh: &tessstep_mesh::Mesh, bounds: [f64; 3], volume: f64) {
    mesh.require_solid().unwrap();
    assert_eq!(
        (mesh.data().positions.len(), mesh.data().triangles.len()),
        (8, 12)
    );
    assert!((mesh.statistics().signed_volume - volume).abs() < volume * 1e-10);
    for (axis, &bound) in bounds.iter().enumerate() {
        let max = mesh
            .data()
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        let min = mesh
            .data()
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f64::INFINITY, f64::min);
        assert!((max - bound).abs() < 1e-12);
        assert!(min.abs() < 1e-12);
    }
    for (t, n) in mesh.data().triangles.iter().zip(&mesh.data().normals) {
        let p = mesh.data().positions[t[0] as usize];
        assert!(
            (0..3)
                .map(|i| (p[i] - bounds[i] / 2.) * n[0][i])
                .sum::<f64>()
                > 0.
        );
    }
}
#[test]
fn planar_edge_brep_to_mesh_preserves_topology_and_line_trims() {
    let solid = import(BOX).unwrap();
    assert_eq!(
        (
            solid.brep().data().vertices.len(),
            solid.brep().data().edges.len()
        ),
        (8, 12)
    );
    for edge in &solid.brep().data().edges {
        assert!(edge.range[0] < 0. && edge.range[1] > 0.);
    }
    let mesh = mesh(&solid);
    check_box(&mesh, [0.01, 0.02, 0.03], 6e-6);
    let mut counts = std::collections::BTreeMap::new();
    for id in &mesh.data().face_ids {
        *counts.entry(*id).or_insert(0) += 1;
    }
    assert_eq!(
        counts,
        [(106, 2), (116, 2), (126, 2), (136, 2), (146, 2), (156, 2)].into()
    );
}
#[test]
fn planar_edge_and_face_orientation_semantics() {
    let reversed = BOX
        .replace(
            "#3001=DIRECTION('',(0.0,20.0,0.0))",
            "#3001=DIRECTION('',(0.0,-20.0,0.0))",
        )
        .replace(
            "#3004=EDGE_CURVE('',#2001,#2004,#3003,.T.)",
            "#3004=EDGE_CURVE('',#2001,#2004,#3003,.F.)",
        );
    check_box(&mesh(&import(&reversed).unwrap()), [0.01, 0.02, 0.03], 6e-6);
    let reversed = BOX
        .replace("#3005,#3011,#3017,#3023", "#3023,#3017,#3011,#3005")
        .replace(
            "#3005=ORIENTED_EDGE('',*,*,#3004,.T.)",
            "#3005=ORIENTED_EDGE('',*,*,#3004,.F.)",
        )
        .replace(
            "#3011=ORIENTED_EDGE('',*,*,#3010,.F.)",
            "#3011=ORIENTED_EDGE('',*,*,#3010,.T.)",
        )
        .replace(
            "#3017=ORIENTED_EDGE('',*,*,#3016,.F.)",
            "#3017=ORIENTED_EDGE('',*,*,#3016,.T.)",
        )
        .replace(
            "#3023=ORIENTED_EDGE('',*,*,#3022,.F.)",
            "#3023=ORIENTED_EDGE('',*,*,#3022,.T.)",
        )
        .replace(
            "#101=FACE_BOUND('',#100,.T.)",
            "#101=FACE_BOUND('',#100,.F.)",
        );
    check_box(&mesh(&import(&reversed).unwrap()), [0.01, 0.02, 0.03], 6e-6);
    let reversed = BOX
        .replace("#102=DIRECTION('',(0,0,-1))", "#102=DIRECTION('',(0,0,1))")
        .replace(
            "#106=ADVANCED_FACE('',(#101),#105,.T.)",
            "#106=ADVANCED_FACE('',(#101),#105,.F.)",
        );
    check_box(&mesh(&import(&reversed).unwrap()), [0.01, 0.02, 0.03], 6e-6);
    let tube = mesh(&import(include_str!("../../../corpus/geometry/planar-tube.step")).unwrap());
    tube.require_solid().unwrap();
    assert!((tube.statistics().signed_volume - 3.84e-6).abs() < 1e-15);
    assert_eq!(tube.data().triangles.len(), 32);
}
#[test]
fn planar_import_rejects_geometry_contradictions_without_welding() {
    for (old, new, stage) in [
        (
            "#3000=CARTESIAN_POINT('',(0.0,10.0,0.0))",
            "#3000=CARTESIAN_POINT('',(1.0,10.0,0.0))",
            Stage::Geometry,
        ),
        (
            "#3004=EDGE_CURVE('',#2001,#2004,#3003,.T.)",
            "#3004=EDGE_CURVE('',#2001,#2004,#3003,.F.)",
            Stage::Geometry,
        ),
        (
            "#3002=VECTOR('',#3001,2.)",
            "#3002=VECTOR('',#3001,0.)",
            Stage::Geometry,
        ),
        (
            "#3002=VECTOR('',#3001,2.)",
            "#3002=VECTOR('',#3001,-2.)",
            Stage::Geometry,
        ),
        (
            "#3005=ORIENTED_EDGE('',*,*,#3004,.T.)",
            "#3005=ORIENTED_EDGE('',*,*,#3004,.F.)",
            Stage::Topology,
        ),
        (
            "#3005=ORIENTED_EDGE('',*,*,#3004,.T.)",
            "#3005=ORIENTED_EDGE('',#2001,*,#3004,.T.)",
            Stage::Profile,
        ),
        (
            "#3004=EDGE_CURVE('',#2001,#2004,#3003,.T.)",
            "#3004=EDGE_CURVE('',#9000,#2004,#3003,.T.);#9000=VERTEX_POINT('',#1)",
            Stage::Topology,
        ),
        (
            "#3005=ORIENTED_EDGE('',*,*,#3004,.T.)",
            "#3005=ORIENTED_EDGE('',*,*,#9000,.T.);#9000=EDGE_CURVE('',#2001,#2004,#3003,.T.)",
            Stage::Topology,
        ),
    ] {
        assert!(BOX.contains(old));
        let error = import(&BOX.replace(old, new)).unwrap_err();
        assert_eq!(
            (error.kind, error.stage),
            (ErrorKind::InvalidGeometry, stage),
            "{error}"
        );
        assert!(error.entity.is_some() && error.source.is_some());
    }
    let curved = BOX.replace("#3003=LINE('',#3000,#3002)", "#3003=CIRCLE('',#104,10.)");
    assert_eq!(import(&curved).unwrap_err().kind, ErrorKind::Unsupported);
}
#[test]
fn planar_unsupported_entities_are_named_as_outside_the_profile() {
    let curved = BOX.replace("#3003=LINE('',#3000,#3002)", "#3003=CIRCLE('',#104,10.)");
    let error = import(&curved).unwrap_err();
    assert_eq!(
        (error.kind, error.stage, error.entity),
        (ErrorKind::Unsupported, Stage::Profile, EntityId::new(3003))
    );
    assert_eq!(
        error.message,
        "CIRCLE is outside the tessstep_planar import profile"
    );
    let complex = BOX.replace(
        "#3003=LINE('',#3000,#3002)",
        "#3003=(BOUNDED_CURVE()CURVE()LINE(#3000,#3002)REPRESENTATION_ITEM(''))",
    );
    let error = import(&complex).unwrap_err();
    assert_eq!(
        (error.kind, error.entity),
        (ErrorKind::Unsupported, EntityId::new(3003))
    );
    assert_eq!(
        error.message,
        "complex instance (BOUNDED_CURVE, CURVE, LINE, REPRESENTATION_ITEM) has components \
         outside the tessstep_planar import profile: BOUNDED_CURVE, CURVE"
    );
}
#[test]
fn planar_scope_budgets_order_and_mutation_smoke() {
    let doc = tessstep_model::parse(BOX.as_bytes(), ParseLimits::default()).unwrap();
    for limits in [
        ImportOptions {
            max_work: 0,
            ..ImportOptions::default()
        },
        ImportOptions {
            max_records: 0,
            ..ImportOptions::default()
        },
    ] {
        assert_eq!(
            import_planar_solid(
                &doc,
                EntityId::new(1000).unwrap(),
                LengthUnit::MILLIMETRE,
                model_tolerance(),
                limits
            )
            .unwrap_err()
            .kind,
            ErrorKind::ResourceLimit
        );
    }
    let (header, data) = BOX.split_once("DATA;\n").unwrap();
    let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
    records.reverse();
    let reordered = format!(
        "{header}DATA;\n{}\nENDSEC;END-ISO-10303-21;",
        records.join("\n")
    );
    assert_eq!(
        mesh(&import(BOX).unwrap()).data().triangles,
        mesh(&import(&reordered).unwrap()).data().triangles
    );
    let mut seed = 47u64;
    for _ in 0..300 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut bytes = BOX.as_bytes().to_vec();
        let at = seed as usize % bytes.len();
        bytes[at] = (seed >> 32) as u8;
        if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
            let _ = import_planar_solid(
                &doc,
                EntityId::new(1000).unwrap(),
                LengthUnit::MILLIMETRE,
                model_tolerance(),
                ImportOptions {
                    max_work: 100_000,
                    max_records: 1000,
                    ..ImportOptions::default()
                },
            );
        }
    }
}
#[test]
fn planar_unmodified_exporter_cuboid() {
    let root = std::env::var_os("TESSSTEP_CORPUS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("step-corpus"))
        });
    let Some(path) = root
        .map(|r| r.join("vendor/foxtrot/examples/cuboid.step"))
        .filter(|p| p.is_file())
    else {
        eprintln!("external cuboid unavailable; authored planar tests still run");
        return;
    };
    let doc = tessstep_model::parse(
        std::io::BufReader::new(std::fs::File::open(path).unwrap()),
        ParseLimits::default(),
    )
    .unwrap();
    let solid = import_planar_solid(
        &doc,
        EntityId::new(121).unwrap(),
        LengthUnit::METRE,
        model_tolerance(),
        ImportOptions::default(),
    )
    .unwrap();
    let mesh = mesh(&solid);
    check_box(&mesh, [0.0508, 0.0254, 0.0762], 0.000098322384);
    assert!(
        mesh.data()
            .face_ids
            .iter()
            .all(|id| (106..=111).contains(id))
    );
}
#[test]
fn planar_unset_derived_slots_are_tolerated_unless_strict() {
    let unset = BOX.replace("ORIENTED_EDGE('',*,*,", "ORIENTED_EDGE('',$,$,");
    assert_ne!(unset, BOX);
    let tolerant = mesh(&import(&unset).unwrap());
    check_box(&tolerant, [0.01, 0.02, 0.03], 6e-6);
    let reference = mesh(&import(BOX).unwrap());
    assert_eq!(tolerant.data().positions, reference.data().positions);
    assert_eq!(tolerant.data().triangles, reference.data().triangles);
    assert_eq!(tolerant.data().face_ids, reference.data().face_ids);
    let doc = tessstep_model::parse(unset.as_bytes(), ParseLimits::default()).unwrap();
    let strict = ImportOptions {
        strict: true,
        ..ImportOptions::default()
    };
    let error = import_planar_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        model_tolerance(),
        strict,
    )
    .unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::InvalidGeometry, Stage::Profile)
    );
    let doc = tessstep_model::parse(BOX.as_bytes(), ParseLimits::default()).unwrap();
    import_planar_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        model_tolerance(),
        strict,
    )
    .unwrap();
    let mixed = BOX.replace(
        "#3005=ORIENTED_EDGE('',*,*,#3004,.T.)",
        "#3005=ORIENTED_EDGE('',$,*,#3004,.T.)",
    );
    import(&mixed).unwrap();
    for other in ["#2001,#2002,", "#2001,$,"] {
        let explicit = BOX.replacen(
            "ORIENTED_EDGE('',*,*,",
            &format!("ORIENTED_EDGE('',{other}"),
            1,
        );
        assert_eq!(
            import(&explicit).unwrap_err().stage,
            Stage::Profile,
            "{other}"
        );
    }
}
