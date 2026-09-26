use tessstep_import::*;
use tessstep_math::{Angle, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};
const BOX: &str = include_str!("../../../corpus/geometry/box.step");
fn model_tolerance() -> ModelTolerance {
    ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap()
}
fn import(source: &str, unit: LengthUnit) -> Result<ImportedSolid, Error> {
    let document = tessstep_model::parse(source.as_bytes(), ParseLimits::default()).unwrap();
    import_faceted_solid(
        &document,
        EntityId::new(1000).unwrap(),
        unit,
        model_tolerance(),
        ImportLimits::default(),
    )
}
fn mesh(solid: &ImportedSolid) -> tessstep_mesh::Mesh {
    let tol =
        TessellationTolerance::new(Length::metres(1e-6).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    solid
        .tessellate(tol, TessellationOptions::default())
        .unwrap()
}
#[test]
fn step_faceted_box_to_owned_solid_mesh() {
    let solid = import(BOX, LengthUnit::MILLIMETRE).unwrap();
    assert_eq!(solid.brep().data().vertices.len(), 8);
    assert_eq!(solid.brep().data().edges.len(), 12);
    let mesh = mesh(&solid);
    mesh.require_solid().unwrap();
    assert_eq!(mesh.data().positions.len(), 8);
    assert_eq!(mesh.data().triangles.len(), 12);
    assert!((mesh.statistics().signed_volume - 6e-6).abs() < 1e-15);
    assert_eq!(mesh.statistics().boundary_edges, 0);
    assert_eq!(mesh.statistics().components, 1);
    let mut counts = std::collections::BTreeMap::new();
    for id in &mesh.data().face_ids {
        *counts.entry(*id).or_insert(0) += 1;
    }
    assert_eq!(
        counts,
        [(106, 2), (116, 2), (126, 2), (136, 2), (146, 2), (156, 2)].into()
    );
    // Independent axis-aligned bounds and outward normals, not just incidence.
    for axis in 0..3 {
        let max = mesh
            .data()
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max - [0.01, 0.02, 0.03][axis]).abs() < 1e-15);
    }
    let center = [0.005, 0.01, 0.015];
    for (t, n) in mesh.data().triangles.iter().zip(&mesh.data().normals) {
        let p = mesh.data().positions[t[0] as usize];
        assert!((0..3).map(|i| (p[i] - center[i]) * n[0][i]).sum::<f64>() > 0.);
    }
    let metres = mesh_unit(BOX, LengthUnit::METRE);
    assert!((metres.statistics().signed_volume - 6000.).abs() < 1e-8);
}
fn mesh_unit(s: &str, u: LengthUnit) -> tessstep_mesh::Mesh {
    mesh(&import(s, u).unwrap())
}
#[test]
fn faceted_orientation_defaults_and_complex_mapping() {
    let expected = mesh_unit(BOX, LengthUnit::MILLIMETRE);
    let reverse_bound = BOX.replace("#1,#4,#3,#2", "#2,#3,#4,#1").replace(
        "#101=FACE_OUTER_BOUND('',#100,.T.)",
        "#101=FACE_OUTER_BOUND('',#100,.F.)",
    );
    let reversed = mesh_unit(&reverse_bound, LengthUnit::MILLIMETRE);
    assert_eq!(expected.data().triangles, reversed.data().triangles);
    let reverse_face = BOX
        .replace("#102=DIRECTION('',(0,0,-1))", "#102=DIRECTION('',(0,0,1))")
        .replace(
            "#106=FACE_SURFACE('',(#101),#105,.T.)",
            "#106=FACE_SURFACE('',(#101),#105,.F.)",
        );
    assert!(
        (mesh_unit(&reverse_face, LengthUnit::MILLIMETRE)
            .statistics()
            .signed_volume
            - 6e-6)
            .abs()
            < 1e-15
    );
    let defaults = BOX.replace(
        "#114=AXIS2_PLACEMENT_3D('',#5,#112,#113)",
        "#114=AXIS2_PLACEMENT_3D('',#5,$,$)",
    );
    assert_eq!(
        mesh_unit(&defaults, LengthUnit::MILLIMETRE)
            .data()
            .triangles,
        expected.data().triangles
    );
    let complex = BOX.replace(
        "#1000=FACETED_BREP('box',#900)",
        "#1000=(FACETED_BREP()MANIFOLD_SOLID_BREP(#900)REPRESENTATION_ITEM('box'))",
    );
    assert_eq!(
        import(&complex, LengthUnit::MILLIMETRE).unwrap_err().stage,
        Stage::Profile
    );
}
#[test]
fn faceted_rejects_invalid_and_unsupported_without_healing() {
    for (text, kind, stage) in [
        (
            include_str!("../../../corpus/geometry/open-shell.step"),
            ErrorKind::InvalidGeometry,
            Stage::Topology,
        ),
        (
            include_str!("../../../corpus/geometry/nonplanar.step"),
            ErrorKind::InvalidGeometry,
            Stage::Geometry,
        ),
        (
            include_str!("../../../corpus/geometry/duplicate-point.step"),
            ErrorKind::InvalidGeometry,
            Stage::Profile,
        ),
        (
            include_str!("../../../corpus/geometry/missing-point.step"),
            ErrorKind::MissingEntity,
            Stage::Profile,
        ),
        (
            include_str!("../../../corpus/geometry/curved.step"),
            ErrorKind::Unsupported,
            Stage::Profile,
        ),
    ] {
        let error = import(text, LengthUnit::MILLIMETRE).unwrap_err();
        assert_eq!((error.kind, error.stage), (kind, stage), "{error}");
        assert!(error.entity.is_some() && error.source.is_some());
    }
    let wrong = BOX.replace("#105=PLANE('',#104)", "#105=CARTESIAN_POINT('',(1.,2.,3.))");
    assert_eq!(
        import(&wrong, LengthUnit::METRE).unwrap_err().kind,
        ErrorKind::InvalidGeometry
    );
    let shell_type = BOX.replace("#900=CLOSED_SHELL", "#900=OPEN_SHELL");
    assert!(import(&shell_type, LengthUnit::METRE).is_err());
    let coincident = BOX
        .replace("#1,#4,#3,#2", "#3000,#4,#3,#2")
        .replace("#2000=", "#3000=CARTESIAN_POINT('',(0.,0.,0.));\n#2000=");
    assert_eq!(
        import(&coincident, LengthUnit::METRE).unwrap_err().stage,
        Stage::Topology
    );
}
#[test]
fn faceted_limits_and_selected_root_are_explicit() {
    let doc = tessstep_model::parse(BOX.as_bytes(), ParseLimits::default()).unwrap();
    for limits in [
        ImportLimits {
            max_work: 0,
            ..ImportLimits::default()
        },
        ImportLimits {
            max_records: 0,
            ..ImportLimits::default()
        },
    ] {
        assert_eq!(
            import_faceted_solid(
                &doc,
                EntityId::new(1000).unwrap(),
                LengthUnit::METRE,
                model_tolerance(),
                limits
            )
            .unwrap_err()
            .kind,
            ErrorKind::ResourceLimit
        );
    }
    assert_eq!(
        import_faceted_solid(
            &doc,
            EntityId::new(1).unwrap(),
            LengthUnit::METRE,
            model_tolerance(),
            ImportLimits::default()
        )
        .unwrap_err()
        .kind,
        ErrorKind::Unsupported
    );
    let solid = import(BOX, LengthUnit::METRE).unwrap();
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-6).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    let opts = TessellationOptions {
        max_work: 0,
        ..TessellationOptions::default()
    };
    assert_eq!(
        solid.tessellate(tolerance, opts).unwrap_err().kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn faceted_through_hole_preserves_material_volume() {
    let mesh = mesh_unit(
        include_str!("../../../corpus/geometry/tube.step"),
        LengthUnit::MILLIMETRE,
    );
    assert_eq!(mesh.data().positions.len(), 16);
    assert_eq!(mesh.data().triangles.len(), 32);
    mesh.require_solid().unwrap();
    assert!((mesh.statistics().signed_volume - 3.84e-6).abs() < 1e-15);
    for t in &mesh.data().triangles {
        let p = t.map(|i| mesh.data().positions[i as usize]);
        let center: [f64; 3] = std::array::from_fn(|i| (p[0][i] + p[1][i] + p[2][i]) / 3.);
        assert!(
            !(center[0] > 0.002 + 1e-10
                && center[0] < 0.008 - 1e-10
                && center[1] > 0.004 + 1e-10
                && center[1] < 0.016 - 1e-10)
        );
    }
}

#[test]
fn faceted_order_and_bounded_mutation_smoke() {
    let expected = mesh_unit(BOX, LengthUnit::MILLIMETRE);
    let (header, data) = BOX.split_once("DATA;\n").unwrap();
    let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
    records.reverse();
    let reordered = format!(
        "{header}DATA;\n{}\nENDSEC;END-ISO-10303-21;",
        records.join("\n")
    );
    assert_eq!(
        mesh_unit(&reordered, LengthUnit::MILLIMETRE)
            .data()
            .triangles,
        expected.data().triangles
    );
    let mut seed = 17u64;
    for _ in 0..500 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut bytes = BOX.as_bytes().to_vec();
        let at = (seed as usize) % bytes.len();
        bytes[at] = (seed >> 32) as u8;
        if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
            let imported = import_faceted_solid(
                &doc,
                EntityId::new(1000).unwrap(),
                LengthUnit::MILLIMETRE,
                model_tolerance(),
                ImportLimits {
                    max_work: 50_000,
                    max_records: 1000,
                },
            );
            if let Ok(solid) = imported {
                let tol = TessellationTolerance::new(
                    Length::metres(1e-6).unwrap(),
                    Angle::radians(0.1).unwrap(),
                )
                .unwrap();
                let opts = TessellationOptions {
                    max_work: 50_000,
                    max_evaluations: 50_000,
                    ..TessellationOptions::default()
                };
                if let Ok(mesh) = solid.tessellate(tol, opts) {
                    mesh.require_solid().unwrap();
                }
            }
        }
    }
}
