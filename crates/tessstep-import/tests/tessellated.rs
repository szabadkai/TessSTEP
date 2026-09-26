use tessstep_import::*;
use tessstep_math::LengthUnit;
use tessstep_part21::{EntityId, ParseLimits};
const CUBE: &str = include_str!("../../../corpus/geometry/tessellated-cube.step");
const SHELL: &str = include_str!("../../../corpus/geometry/tessellated-shell.step");
const OPEN: &str = include_str!("../../../corpus/geometry/tessellated-open-solid.step");
const EDGE: &str = include_str!("../../../corpus/geometry/tessellated-edge.step");
fn import_root(
    text: &str,
    root: u64,
    limits: ImportOptions,
    mesh: tessstep_mesh::Limits,
) -> Result<ImportedTessellation, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_tessellated(
        &doc,
        EntityId::new(root).unwrap(),
        LengthUnit::MILLIMETRE,
        limits,
        mesh,
    )
}
fn import(text: &str) -> Result<ImportedTessellation, Error> {
    import_root(
        text,
        1000,
        ImportOptions::default(),
        tessstep_mesh::Limits::default(),
    )
}
fn failure(text: &str) -> (Stage, ErrorKind, Option<u64>) {
    let e = import(text).unwrap_err();
    (e.stage, e.kind, e.entity.map(EntityId::get))
}
/// A one-face tetrahedron (10 mm legs) with replaceable attribute text.
fn tetra(npoints: &str, pnmax: &str, normals: &str, pnindex: &str, triangles: &str) -> String {
    format!(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');\
         FILE_SCHEMA(('AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF'));ENDSEC;DATA;\
         #1=COORDINATES_LIST('',{npoints},((0.,0.,0.),(10.,0.,0.),(0.,10.,0.),(0.,0.,10.)));\
         #2=TRIANGULATED_FACE('',#1,{pnmax},{normals},$,{pnindex},{triangles});\
         #1000=TESSELLATED_SOLID('',(#2),$);ENDSEC;END-ISO-10303-21;"
    )
}
const TETRA: &str = "((1,3,2),(1,2,4),(1,4,3),(2,3,4))";
#[test]
fn tessellated_solid_uses_every_encoding_without_retessellation() {
    let imported = import(CUBE).unwrap();
    assert_eq!(imported.kind(), TessellatedKind::Solid);
    assert_eq!(imported.root().get(), 1000);
    assert_eq!(imported.link(), None);
    // The stitching triangle (4,4 in the left strip) is omitted, not published.
    assert_eq!(imported.skipped_degenerate(), 1);
    let mesh = imported.mesh();
    mesh.require_solid().unwrap();
    assert_eq!(
        (mesh.data().positions.len(), mesh.data().triangles.len()),
        (8, 12)
    );
    assert!((mesh.statistics().signed_volume - 6e-6).abs() < 6e-6 * 1e-12);
    let mut counts = std::collections::BTreeMap::new();
    for id in &mesh.data().face_ids {
        *counts.entry(*id).or_insert(0) += 1;
    }
    assert_eq!(
        counts,
        [(100, 2), (101, 2), (102, 2), (103, 2), (104, 2), (105, 2)].into()
    );
    let faces: Vec<_> = imported
        .faces()
        .iter()
        .map(|f| {
            (
                f.entity.get(),
                f.geometric_link.map(EntityId::get),
                f.supplied_normals,
            )
        })
        .collect();
    assert_eq!(
        faces,
        [
            (100, None, false),
            (101, Some(14), true),
            (102, None, true),
            (103, None, true),
            (104, None, true),
            (105, None, false)
        ]
    );
    // Every corner normal is unit length and points out of the box centre, whether
    // supplied (including the nonunit back normal) or derived from winding.
    let centre = [0.005, 0.01, 0.015];
    for (t, normals) in mesh.data().triangles.iter().zip(&mesh.data().normals) {
        let p = mesh.data().positions[t[0] as usize];
        for n in normals {
            assert!((n.iter().map(|x| x * x).sum::<f64>() - 1.).abs() < 1e-12);
            assert!((0..3).map(|i| (p[i] - centre[i]) * n[i]).sum::<f64>() > 0.);
        }
    }
    let max = |axis: usize| {
        mesh.data()
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f64::NEG_INFINITY, f64::max)
    };
    assert_eq!([max(0), max(1), max(2)], [0.01, 0.02, 0.03]);
}
#[test]
fn tessellated_shells_may_be_open_and_links_are_not_followed() {
    let imported = import(SHELL).unwrap();
    assert_eq!(imported.kind(), TessellatedKind::Shell);
    assert_eq!(imported.link().map(EntityId::get), Some(21));
    assert_eq!(
        imported.faces()[1].geometric_link.map(EntityId::get),
        Some(20)
    );
    let mesh = imported.mesh();
    assert_eq!(
        (
            mesh.data().positions.len(),
            mesh.data().triangles.len(),
            mesh.statistics().boundary_edges,
            mesh.statistics().components
        ),
        (6, 4, 6, 1)
    );
    assert_eq!(mesh.require_solid(), Err(tessstep_mesh::Error::Open));
    // A missing link target is a missing entity, even though targets are not decoded.
    assert_eq!(
        failure(&SHELL.replace("#101),#21);", "#101),#22);")),
        (Stage::Profile, ErrorKind::MissingEntity, Some(1000))
    );
    assert_eq!(
        failure(&SHELL.replace("#101),#21);", "#101),*);")).1,
        ErrorKind::InvalidGeometry
    );
}
#[test]
fn surface_sets_import_directly() {
    let text = tetra("4", "4", "()", "()", TETRA)
        .replace(
            "TRIANGULATED_FACE('',#1,4,(),$,",
            "TRIANGULATED_SURFACE_SET('',#1,4,(),",
        )
        .replace("#1000=TESSELLATED_SOLID('',(#2),$);", "");
    let imported = import_root(
        &text,
        2,
        ImportOptions::default(),
        tessstep_mesh::Limits::default(),
    )
    .unwrap();
    assert_eq!(imported.kind(), TessellatedKind::SurfaceSet);
    assert_eq!(imported.faces().len(), 1);
    assert!(imported.mesh().is_watertight());
    assert!((imported.mesh().statistics().signed_volume - 1e-6 / 6.).abs() < 1e-18);
    // Strip (1,3,2),(2,3,4) and fan (1,2,4),(1,4,3) close the same tetrahedron.
    let complex = text.replace(
        &format!("TRIANGULATED_SURFACE_SET('',#1,4,(),(),{TETRA})"),
        "COMPLEX_TRIANGULATED_SURFACE_SET('',#1,4,(),(),((1,3,2,4)),((1,2,4,3)))",
    );
    let imported = import_root(
        &complex,
        2,
        ImportOptions::default(),
        tessstep_mesh::Limits::default(),
    )
    .unwrap();
    assert!(imported.mesh().is_watertight());
    assert!((imported.mesh().statistics().signed_volume - 1e-6 / 6.).abs() < 1e-18);
    // Strips and fans need at least three indices.
    let short = complex.replace("((1,3,2,4)),", "((1,3,2,4),(4,1)),");
    assert_eq!(
        import_root(
            &short,
            2,
            ImportOptions::default(),
            tessstep_mesh::Limits::default()
        )
        .unwrap_err()
        .stage,
        Stage::Profile
    );
}
#[test]
fn tessellated_solid_rejections_are_typed_and_located() {
    assert!(import(&tetra("4", "4", "()", "()", TETRA)).is_ok());
    let reversed = "((1,2,3),(1,4,2),(1,3,4),(2,4,3))";
    let geometry = |entity| (Stage::Geometry, ErrorKind::InvalidGeometry, Some(entity));
    for (text, expected) in [
        (
            tetra("4", "4", "()", "()", reversed),
            (Stage::Topology, ErrorKind::InvalidGeometry, Some(1000)),
        ),
        (tetra("5", "4", "()", "()", TETRA), geometry(1)),
        (tetra("4", "3", "()", "()", TETRA), geometry(2)),
        (
            tetra("4", "4", "()", "()", "((1,3,2),(1,2,5))"),
            geometry(2),
        ),
        (
            tetra("4", "4", "()", "()", "((1,3,2),(1,1,2))"),
            geometry(2),
        ),
        (
            tetra("4", "4", "()", "()", "((1,3,2),(0,1,2))"),
            geometry(2),
        ),
        (tetra("4", "4", "((0.,0.,1.))", "()", TETRA), geometry(2)),
        (tetra("4", "4", "((0.,0.,0.))", "()", TETRA), geometry(2)),
        (
            tetra("4", "4", "((0.,0.,-1.),(0.,-1.,0.))", "()", TETRA),
            geometry(2),
        ),
        (tetra("4", "2", "()", "(1,2)", TETRA), geometry(2)),
        (tetra("4", "4", "()", "(1,2,3)", TETRA), geometry(2)),
        (tetra("4", "4", "()", "(1,2,3,5)", TETRA), geometry(2)),
        (
            tetra("4", "4", "()", "()", "((1,3,2),(1,2,4),(1,4,3))"),
            (Stage::Topology, ErrorKind::InvalidGeometry, Some(1000)),
        ),
    ] {
        assert_eq!(failure(&text), expected, "{text}");
    }
    // Per-point normals follow pnindex order and must agree with winding.
    let outward = "((-1.,-1.,-1.),(3.,-1.,-1.),(-1.,3.,-1.),(-1.,-1.,3.))";
    let imported = import(&tetra("4", "4", outward, "()", TETRA)).unwrap();
    assert!(imported.faces()[0].supplied_normals);
    let n = imported.mesh().data().normals[0][0];
    assert!((n[2] + 1. / 3f64.sqrt()).abs() < 1e-12);
    assert_eq!(
        failure(&tetra("4", "4", outward, "(4,3,2,1)", TETRA)),
        geometry(2)
    );
    // Collinear corners are rejected at the face that supplied them.
    let collinear = tetra("4", "4", "()", "()", TETRA).replace("(0.,10.,0.)", "(5.,0.,0.)");
    assert_eq!(failure(&collinear), geometry(2));
    // Same coordinates in separate lists are never welded.
    let split = OPEN.replace(
        "#1000=TESSELLATED_SOLID('open box',(",
        "#2=COORDINATES_LIST('',8,((0.,0.,0.),(10.,0.,0.),(10.,20.,0.),(0.,20.,0.),(0.,0.,30.),(10.,0.,30.),(10.,20.,30.),(0.,20.,30.)));\
         #101=TRIANGULATED_FACE('top',#2,4,(),$,(5,6,7,8),((1,2,3),(1,3,4)));\
         #1000=TESSELLATED_SOLID('split box',(#101,",
    );
    assert_eq!(
        failure(&split),
        (Stage::Topology, ErrorKind::InvalidGeometry, Some(1000))
    );
    assert!(
        import(&split.replace(
            "#101=TRIANGULATED_FACE('top',#2,",
            "#101=TRIANGULATED_FACE('top',#1,"
        ))
        .is_ok()
    );
    assert_eq!(
        failure(OPEN),
        (Stage::Topology, ErrorKind::InvalidGeometry, Some(1000))
    );
    assert_eq!(
        failure(EDGE),
        (Stage::Profile, ErrorKind::Unsupported, Some(101))
    );
    assert!(
        import(EDGE)
            .unwrap_err()
            .message
            .contains("TESSELLATED_EDGE is outside the tessstep_tessellated import profile")
    );
    // Faces, coordinate lists and unrelated entities are not tessellated roots.
    for root in [1, 2] {
        let e = import_root(
            &tetra("4", "4", "()", "()", TETRA),
            root,
            ImportOptions::default(),
            tessstep_mesh::Limits::default(),
        )
        .unwrap_err();
        assert_eq!((e.stage, e.kind), (Stage::Geometry, ErrorKind::Unsupported));
    }
    let empty = CUBE.replace(
        "COMPLEX_TRIANGULATED_FACE('right',#1,8,(),$,(),(),((2,3,7,6)))",
        "COMPLEX_TRIANGULATED_FACE('right',#1,8,(),$,(),(),())",
    );
    assert_eq!(failure(&empty), geometry(105));
}
#[test]
fn tessellated_budgets_order_and_mutation_smoke() {
    for (limits, mesh) in [
        (
            ImportOptions {
                max_work: 0,
                ..ImportOptions::default()
            },
            tessstep_mesh::Limits::default(),
        ),
        (
            ImportOptions {
                max_records: 3,
                ..ImportOptions::default()
            },
            tessstep_mesh::Limits::default(),
        ),
        (
            ImportOptions::default(),
            tessstep_mesh::Limits {
                max_triangles: 11,
                ..tessstep_mesh::Limits::default()
            },
        ),
        (
            ImportOptions::default(),
            tessstep_mesh::Limits {
                max_vertices: 7,
                ..tessstep_mesh::Limits::default()
            },
        ),
        (
            ImportOptions::default(),
            tessstep_mesh::Limits {
                max_work: 10,
                ..tessstep_mesh::Limits::default()
            },
        ),
    ] {
        assert_eq!(
            import_root(CUBE, 1000, limits, mesh).unwrap_err().kind,
            ErrorKind::ResourceLimit
        );
    }
    let (header, data) = CUBE.split_once("DATA;\n").unwrap();
    let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
    records.reverse();
    let reordered = format!(
        "{header}DATA;\n{}\nENDSEC;END-ISO-10303-21;",
        records.join("\n")
    );
    let a = import(CUBE).unwrap().into_mesh().into_data();
    let b = import(&reordered).unwrap().into_mesh().into_data();
    assert_eq!(
        (a.positions, a.triangles, a.face_ids),
        (b.positions, b.triangles, b.face_ids)
    );
    let mut seed = 53u64;
    for _ in 0..300 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut bytes = CUBE.as_bytes().to_vec();
        let at = seed as usize % bytes.len();
        bytes[at] = (seed >> 32) as u8;
        if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
            let _ = import_tessellated(
                &doc,
                EntityId::new(1000).unwrap(),
                LengthUnit::MILLIMETRE,
                ImportOptions {
                    max_work: 100_000,
                    max_records: 1000,
                    ..ImportOptions::default()
                },
                tessstep_mesh::Limits::default(),
            );
        }
    }
}
#[test]
fn tessellated_unmodified_linked_exporter_solid() {
    let root = std::env::var_os("TESSSTEP_CORPUS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("step-corpus"))
        });
    let Some(path) = root
        .map(|r| {
            r.join("vendor/nist-pmi/unpacked/NIST-PMI-STEP-Files/nist_ftc_08_asme1_ap242-e1-tg.stp")
        })
        .filter(|p| p.is_file())
    else {
        eprintln!("external NIST tessellated solid unavailable; authored tests still run");
        return;
    };
    let doc = tessstep_model::parse(
        std::io::BufReader::new(std::fs::File::open(path).unwrap()),
        ParseLimits::default(),
    )
    .unwrap();
    let imported = import_tessellated(
        &doc,
        EntityId::new(11436).unwrap(),
        LengthUnit::MILLIMETRE,
        ImportOptions::default(),
        tessstep_mesh::Limits::default(),
    )
    .unwrap();
    imported.mesh().require_solid().unwrap();
    assert_eq!(imported.faces().len(), 272);
    assert!(imported.faces().iter().all(|f| f.geometric_link.is_some()));
    assert_eq!(imported.skipped_degenerate(), 20);
    assert_eq!(
        (
            imported.mesh().data().positions.len(),
            imported.mesh().data().triangles.len()
        ),
        (1636, 3368)
    );
}
