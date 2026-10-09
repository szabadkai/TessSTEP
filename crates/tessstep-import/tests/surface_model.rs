use std::f64::consts::PI;
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const OPEN: &str = include_str!("../../../corpus/geometry/surface-open-patches.step");
const CLOSED: &str = include_str!("../../../corpus/geometry/surface-closed-shells.step");
const CYLINDER: &str = include_str!("../../../corpus/geometry/brep-cylinder.step");

fn model_tolerance() -> ModelTolerance {
    ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap()
}
fn import_with(
    text: &str,
    root: u64,
    options: ImportOptions,
) -> Result<ImportedSurfaceModel, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_surface_model(
        &doc,
        EntityId::new(root).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        model_tolerance(),
        options,
    )
}
fn import(text: &str, root: u64) -> Result<ImportedSurfaceModel, Error> {
    import_with(text, root, ImportOptions::default())
}
fn tolerance(chord: f64) -> TessellationTolerance {
    TessellationTolerance::new(Length::metres(chord).unwrap(), Angle::radians(0.1).unwrap())
        .unwrap()
}
fn meshes(model: &ImportedSurfaceModel, chord: f64) -> Vec<tessstep_mesh::Mesh> {
    model
        .tessellate(tolerance(chord), TessellationOptions::default())
        .unwrap()
}
fn area(mesh: &tessstep_mesh::Mesh) -> f64 {
    let d = mesh.data();
    d.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| d.positions[i as usize]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.
        })
        .sum()
}

#[test]
fn surface_model_open_patches_mesh_as_one_open_shell() {
    let model = import(OPEN, 1000).unwrap();
    assert_eq!(model.root().get(), 1000);
    assert_eq!(model.adaptations(), Adaptations::default());
    // One declared open shell of three faces with eight boundary edges (the two
    // flange outer rails, four flange ends and the two arcs); no solid is built.
    let [shell] = model.shells() else {
        panic!("one shell")
    };
    assert_eq!(
        (
            shell.entity().get(),
            shell.closed(),
            shell.face_entities().len(),
            shell.boundary_edges()
        ),
        (105, false, 3, 8)
    );
    let data = shell.brep().data();
    assert!(data.solids.is_empty());
    assert_eq!(data.shells.len(), 1);
    assert!(!data.shells[0].closed);
    assert_eq!((data.vertices.len(), data.edges.len()), (8, 10));
    let chord = 1e-5;
    let [mesh] = &meshes(&model, chord)[..] else {
        panic!("one mesh")
    };
    let stats = mesh.statistics();
    assert_eq!(stats.components, 1);
    assert!(stats.boundary_edges > 0 && !mesh.is_watertight());
    // An open shell is never a solid.
    assert_eq!(
        mesh.require_solid().unwrap_err(),
        tessstep_mesh::Error::Open
    );
    // Half cylinder r=10 h=20 and two 5 x 20 flanges, within the chord bound.
    let exact = (200. * PI + 200.) * 1e-6;
    assert!(((area(mesh) - exact) / exact).abs() <= 2. * chord / 0.01);
    let faces: std::collections::BTreeSet<u64> = mesh.data().face_ids.iter().copied().collect();
    assert_eq!(
        faces,
        shell.face_entities().iter().map(|id| id.get()).collect()
    );
    assert_eq!(faces.len(), 3);
    // The cylinder's normals point away from its axis and the flanges' along +y.
    for (normals, tri) in mesh.data().normals.iter().zip(&mesh.data().triangles) {
        let p = mesh.data().positions[tri[0] as usize];
        let n = normals[0];
        if p[0].abs() < 0.0099 && p[1] > 1e-4 {
            assert!(n[0] * p[0] + n[1] * p[1] > 0.);
        } else if p[0].abs() > 0.0101 {
            assert!(n[1] > 0.99);
        }
    }
}

#[test]
fn surface_model_oriented_open_shell_reverses_every_face() {
    let forward = import(OPEN, 1000).unwrap();
    let reversed = import(OPEN, 1001).unwrap();
    assert_eq!(reversed.shells()[0].entity().get(), 106);
    assert!(!reversed.shells()[0].closed());
    let a = &meshes(&forward, 1e-5)[0];
    let b = &meshes(&reversed, 1e-5)[0];
    assert_eq!(a.data().positions, b.data().positions);
    assert_eq!(a.data().triangles.len(), b.data().triangles.len());
    for (x, y) in a.data().triangles.iter().zip(&b.data().triangles) {
        assert_eq!([x[0], x[2], x[1]], *y);
    }
    for (x, y) in a.data().normals.iter().zip(&b.data().normals) {
        assert!((x[0][1] + y[0][1]).abs() < 1e-12 || (x[0][1] + y[2][1]).abs() < 1e-12);
    }
    assert!((a.statistics().signed_volume + b.statistics().signed_volume).abs() < 1e-18);
}

#[test]
fn surface_model_closed_shells_are_watertight_shells_not_solids() {
    let model = import(CLOSED, 1000).unwrap();
    assert_eq!(model.adaptations().inserted_seams, 1);
    let shells = model.shells();
    assert_eq!(shells.len(), 2);
    assert!(shells.iter().all(|s| s.closed() && s.boundary_edges() == 0));
    assert_eq!(
        (
            shells[0].face_entities().len(),
            shells[1].face_entities().len()
        ),
        (3, 6)
    );
    // Each shell is its own validated B-rep without a solid: the cylinder keeps its two
    // circles and inserted seam, the box its eight corners and twelve edges.
    for (shell, (vertices, edges)) in shells.iter().zip([(2, 3), (8, 12)]) {
        let data = shell.brep().data();
        assert!(data.solids.is_empty() && data.shells.len() == 1 && data.shells[0].closed);
        assert_eq!((data.vertices.len(), data.edges.len()), (vertices, edges));
    }
    let chord = 1e-5;
    let meshes = meshes(&model, chord);
    assert_eq!(meshes.len(), 2);
    for mesh in &meshes {
        assert!(mesh.is_watertight());
        assert_eq!(mesh.statistics().components, 1);
    }
    // The cylinder shell faces out; the ORIENTED_CLOSED_SHELL(.F.) box faces in and is
    // still accepted, because a closed shell of a surface model is not a solid.
    let cylinder = meshes[0].statistics().signed_volume;
    assert!(((cylinder - 2000. * PI * 1e-9) / (2000. * PI * 1e-9)).abs() <= 2. * chord / 0.01);
    assert!((meshes[1].statistics().signed_volume + 1e-6).abs() < 1e-18);
    assert!(meshes[1].require_solid().is_err());
    // Single shells mesh independently.
    let single = shells[1]
        .tessellate(tolerance(chord), TessellationOptions::default())
        .unwrap();
    assert_eq!(single.data().positions, meshes[1].data().positions);
}

#[test]
fn surface_model_rejections_are_typed_and_located() {
    // Declared CLOSED_SHELL whose faces leave boundary edges.
    let error = import(CLOSED, 1001).unwrap_err();
    assert_eq!(
        (error.kind, error.stage, error.entity.map(EntityId::get)),
        (ErrorKind::InvalidGeometry, Stage::Topology, Some(1001))
    );
    assert!(error.message.contains("OpenShell"), "{error}");
    // Vertex shells are outside the profile and named.
    let error = import(CLOSED, 1002).unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::Unsupported, Stage::Profile)
    );
    assert!(
        error
            .message
            .contains("VERTEX_SHELL is outside the tessstep_brep import profile"),
        "{error}"
    );
    // A solid root is not a surface model and a surface model is not a solid.
    let error = import(CYLINDER, 1000).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Unsupported);
    assert!(
        error.message.contains("SHELL_BASED_SURFACE_MODEL"),
        "{error}"
    );
    let doc = tessstep_model::parse(OPEN.as_bytes(), ParseLimits::default()).unwrap();
    let error = import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        model_tolerance(),
        ImportOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Unsupported);
    // An open shell cannot bound a solid, and a face is not a shell: both are typed
    // profile rejections naming the referenced record.
    let misused = OPEN.replace(
        "#1001=SHELL_BASED_SURFACE_MODEL('',(#106));",
        "#1001=SHELL_BASED_SURFACE_MODEL('',(#97));#1002=MANIFOLD_SOLID_BREP('',#105);",
    );
    assert_ne!(misused, OPEN);
    let doc = tessstep_model::parse(misused.as_bytes(), ParseLimits::default()).unwrap();
    let error = import_brep_solid(
        &doc,
        EntityId::new(1002).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        model_tolerance(),
        ImportOptions::default(),
    )
    .unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::InvalidGeometry, Stage::Profile)
    );
    assert!(
        error.message.contains("OUTER references OPEN_SHELL #105"),
        "{error}"
    );
    let error = import(&misused, 1001).unwrap_err();
    assert!(
        error
            .message
            .contains("SBSM_BOUNDARY references ADVANCED_FACE #97"),
        "{error}"
    );
    // An open shell's faces that are not connected through edges are rejected.
    let split = OPEN.replace(
        "#105=OPEN_SHELL('',(#90,#97,#104));",
        "#105=OPEN_SHELL('',(#90,#104));",
    );
    assert_ne!(split, OPEN);
    let error = import(&split, 1000).unwrap_err();
    assert!(error.message.contains("DisconnectedShell"), "{error}");
    // Strict policy and budgets apply as for solids.
    let unset = OPEN.replace("ORIENTED_EDGE('',*,*,", "ORIENTED_EDGE('',$,$,");
    import(&unset, 1000).unwrap();
    let strict = ImportOptions {
        strict: true,
        ..ImportOptions::default()
    };
    assert_eq!(
        import_with(&unset, 1000, strict).unwrap_err().stage,
        Stage::Profile
    );
    for options in [
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
            import_with(OPEN, 1000, options).unwrap_err().kind,
            ErrorKind::ResourceLimit
        );
    }
}

#[test]
fn surface_model_discovery_reads_context_units() {
    let doc = tessstep_model::parse(OPEN.as_bytes(), ParseLimits::default()).unwrap();
    let roots = discover_surface_models(&doc, ImportOptions::default()).unwrap();
    assert_eq!(
        roots.iter().map(|r| r.entity.get()).collect::<Vec<_>>(),
        [1000, 1001]
    );
    for root in &roots {
        assert_eq!(root.representations.len(), 1);
        let units = root.units.clone().unwrap();
        assert_eq!(units.length, LengthUnit::MILLIMETRE);
        assert!((units.distance_uncertainty.unwrap() - 1e-8).abs() < 1e-22);
    }
    // Surface models are a separate root class.
    assert!(
        discover_solids(&doc, ImportOptions::default())
            .unwrap()
            .is_empty()
    );
    let doc = tessstep_model::parse(CYLINDER.as_bytes(), ParseLimits::default()).unwrap();
    assert!(
        discover_surface_models(&doc, ImportOptions::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn surface_model_record_order_and_mutation_smoke() {
    let (header, data) = OPEN.split_once("DATA;\n").unwrap();
    let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
    records.reverse();
    let reordered = format!(
        "{header}DATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        records.join("\n")
    );
    let a = &meshes(&import(OPEN, 1000).unwrap(), 1e-5)[0];
    let b = &meshes(&import(&reordered, 1000).unwrap(), 1e-5)[0];
    assert_eq!(a.data().positions, b.data().positions);
    assert_eq!(a.data().triangles, b.data().triangles);
    let mut seed = 22u64;
    for text in [OPEN, CLOSED] {
        for _ in 0..150 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mut bytes = text.as_bytes().to_vec();
            let at = seed as usize % bytes.len();
            bytes[at] = (seed >> 32) as u8;
            if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
                if let Ok(model) = import_surface_model(
                    &doc,
                    EntityId::new(1000).unwrap(),
                    LengthUnit::MILLIMETRE,
                    AngleUnit::RADIAN,
                    model_tolerance(),
                    ImportOptions {
                        max_work: 200_000,
                        max_records: 1000,
                        ..ImportOptions::default()
                    },
                ) {
                    let _ = model.tessellate(tolerance(1e-4), TessellationOptions::default());
                }
            }
        }
    }
}

#[test]
fn surface_model_unmodified_catia_surface_export() {
    let root = std::env::var_os("TESSSTEP_CORPUS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("step-corpus"))
        });
    let Some(path) = root
        .map(|r| r.join("vendor/opencascade-dataset/step/trj5_k1-ct-214.stp"))
        .filter(|p| p.is_file())
    else {
        eprintln!("external surface model unavailable; authored surface-model tests still run");
        return;
    };
    let doc = tessstep_model::parse(
        std::io::BufReader::new(std::fs::File::open(path).unwrap()),
        ParseLimits::default(),
    )
    .unwrap();
    // CATIA V5 R5 writes one SHELL_BASED_SURFACE_MODEL of 58 single-face open shells.
    let roots = discover_surface_models(&doc, ImportOptions::default()).unwrap();
    assert_eq!(roots.len(), 1);
    let units = roots[0].units.clone().unwrap();
    assert_eq!(units.length, LengthUnit::MILLIMETRE);
    let tolerance_m = units.distance_uncertainty.unwrap().max(1e-7);
    let model = import_surface_model(
        &doc,
        roots[0].entity,
        units.length,
        units.plane_angle,
        ModelTolerance::new(
            Length::metres(tolerance_m).unwrap(),
            Angle::radians(1e-8).unwrap(),
        )
        .unwrap(),
        ImportOptions::for_document(&doc),
    )
    .unwrap();
    let shells = model.shells();
    assert_eq!(shells.len(), 58);
    assert!(
        shells
            .iter()
            .all(|s| !s.closed() && s.face_entities().len() == 1)
    );
    assert_eq!(
        shells.iter().map(|s| s.boundary_edges()).sum::<usize>(),
        294
    );
    let coarse =
        TessellationTolerance::new(Length::metres(1e-3).unwrap(), Angle::radians(0.5).unwrap())
            .unwrap();
    for mesh in model
        .tessellate(coarse, TessellationOptions::default())
        .unwrap()
    {
        assert_eq!(mesh.statistics().components, 1);
        assert!(!mesh.is_watertight());
    }
}
