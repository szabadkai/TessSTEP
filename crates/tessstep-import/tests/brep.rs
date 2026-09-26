use std::f64::consts::PI;
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const CYLINDER: &str = include_str!("../../../corpus/geometry/brep-cylinder.step");
const SEAM_CYLINDER: &str = include_str!("../../../corpus/geometry/brep-seam-cylinder.step");
const CONE: &str = include_str!("../../../corpus/geometry/brep-cone.step");
const TORUS: &str = include_str!("../../../corpus/geometry/brep-torus.step");
const SPHERE_ZONE: &str = include_str!("../../../corpus/geometry/brep-sphere-zone.step");
const WASHER: &str = include_str!("../../../corpus/geometry/brep-washer.step");
const BSPLINE_CUBE: &str = include_str!("../../../corpus/geometry/brep-bspline-cube.step");

fn model_tolerance() -> ModelTolerance {
    ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap()
}
fn import_with(
    text: &str,
    angle: AngleUnit,
    options: ImportOptions,
) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        angle,
        model_tolerance(),
        options,
    )
}
fn import(text: &str) -> Result<ImportedSolid, Error> {
    import_with(text, AngleUnit::RADIAN, ImportOptions::default())
}
fn mesh(solid: &ImportedSolid, chord: f64) -> tessstep_mesh::Mesh {
    let tolerance =
        TessellationTolerance::new(Length::metres(chord).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    let mesh = solid
        .tessellate(tolerance, TessellationOptions::default())
        .unwrap();
    mesh.require_solid().unwrap();
    mesh
}
/// Relative volume error bound for inscribed facets: twice the chord over the
/// smallest curvature radius, which dominates the polygonal area deficit.
fn check_volume(mesh: &tessstep_mesh::Mesh, exact: f64, chord: f64, radius: f64) {
    let volume = mesh.statistics().signed_volume;
    assert!(
        ((volume - exact) / exact).abs() <= 2. * chord / radius,
        "volume {volume} exact {exact}"
    );
}

#[test]
fn brep_annular_cylinder_inserts_one_seam_and_meshes_as_a_strip() {
    let solid = import(CYLINDER).unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inserted_seams: 1,
            ..Adaptations::default()
        }
    );
    // Two circle edges and one inserted isoparametric seam.
    assert_eq!(solid.brep().data().edges.len(), 3);
    let mesh = mesh(&solid, 1e-6);
    check_volume(&mesh, PI * 100. * 20. * 1e-9, 1e-6, 0.01);
    // The side face is a single strip between its boundary circles: no interior
    // vertices are needed for a cylinder, whatever its aspect ratio.
    assert_eq!(mesh.data().positions.len(), 512);
    for p in &mesh.data().positions {
        assert!(
            (p[0].hypot(p[1]) - 0.01).abs() < 1e-12
                || p[2].abs() < 1e-15
                || (p[2] - 0.02).abs() < 1e-15
        );
        assert!(p[2] >= -1e-15 && p[2] <= 0.02 + 1e-15);
    }
    let faces: std::collections::BTreeSet<u64> = mesh.data().face_ids.iter().copied().collect();
    assert_eq!(
        faces,
        solid.face_entities().iter().map(|id| id.get()).collect()
    );
}

#[test]
fn brep_seam_curve_with_supplied_pcurves_matches_the_seamless_cylinder() {
    let seamed = import(SEAM_CYLINDER).unwrap();
    assert_eq!(seamed.adaptations(), Adaptations::default());
    let a = mesh(&seamed, 1e-6);
    let b = mesh(&import(CYLINDER).unwrap(), 1e-6);
    assert_eq!(a.data().positions.len(), b.data().positions.len());
    assert!((a.statistics().signed_volume - b.statistics().signed_volume).abs() < 1e-18);
}

#[test]
fn brep_elementary_surfaces_close_with_expected_volumes() {
    let chord = 1e-5;
    let cone = import_with(CONE, AngleUnit::DEGREE, ImportOptions::default()).unwrap();
    assert_eq!(cone.adaptations().inserted_seams, 1);
    let (r0, r1, h) = (10., 10. + 10. * (PI / 6.).tan(), 10.);
    check_volume(
        &mesh(&cone, chord),
        PI * h / 3. * (r0 * r0 + r0 * r1 + r1 * r1) * 1e-9,
        chord,
        0.01,
    );
    let torus = import(TORUS).unwrap();
    assert_eq!(torus.adaptations(), Adaptations::default());
    check_volume(
        &mesh(&torus, chord),
        2. * PI * PI * 20. * 25. * 1e-9,
        chord,
        0.005,
    );
    let zone = import(SPHERE_ZONE).unwrap();
    assert_eq!(zone.adaptations().inserted_seams, 1);
    check_volume(&mesh(&zone, chord), 1056. * PI * 1e-9, chord, 0.008);
    let washer = import(WASHER).unwrap();
    assert_eq!(
        washer.adaptations(),
        Adaptations {
            inferred_outer_bounds: 2,
            inserted_seams: 2,
            ..Adaptations::default()
        }
    );
    check_volume(&mesh(&washer, chord), PI * 300. * 5. * 1e-9, chord, 0.01);
}

#[test]
fn brep_edge_splits_and_sphere_recharting_are_reported() {
    // Loop vertices a quarter turn apart: the top circle is split where the seam
    // through the bottom vertex crosses it, in both faces that use it.
    let solid = import(include_str!(
        "../../../corpus/geometry/brep-misaligned.step"
    ))
    .unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inserted_seams: 1,
            split_edges: 1,
            ..Adaptations::default()
        }
    );
    // Two circle edges, one split half and one seam; the split vertex is new.
    assert_eq!(
        (
            solid.brep().data().edges.len(),
            solid.brep().data().vertices.len()
        ),
        (4, 3)
    );
    check_volume(&mesh(&solid, 1e-5), PI * 100. * 20. * 1e-9, 1e-5, 0.01);
    // The STEP frame's pole lies inside the cap; a rotated frame is used instead.
    let cap = import(include_str!(
        "../../../corpus/geometry/brep-sphere-cap.step"
    ))
    .unwrap();
    assert_eq!(
        cap.adaptations(),
        Adaptations {
            recharted_spheres: 1,
            ..Adaptations::default()
        }
    );
    check_volume(&mesh(&cap, 1e-5), PI * 16. * 26. / 3. * 1e-9, 1e-5, 0.008);
}

#[test]
fn brep_poles_and_apices_close_with_collapsed_edges() {
    let chord = 1e-5;
    let seam = |collapsed| Adaptations {
        inserted_seams: 1,
        collapsed_edges: collapsed,
        ..Adaptations::default()
    };
    for (name, text, adaptations, volume) in [
        // Every sphere frame puts a pole on or inside the hemisphere: a seam to the
        // enclosed pole and a collapsed edge along the pole line close its chart.
        (
            "hemisphere",
            include_str!("../../../corpus/geometry/brep-hemisphere.step"),
            seam(1),
            2. / 3. * PI * 1e-6,
        ),
        // A VERTEX_LOOP at the apex names the seam's end vertex.
        (
            "vertex loop",
            include_str!("../../../corpus/geometry/brep-vertex-loop.step"),
            seam(1),
            PI / 3. * 1e-6,
        ),
        // The loop meets the apex between the two uses of its generator seam.
        (
            "apex join",
            include_str!("../../../corpus/geometry/brep-cone-seam.step"),
            Adaptations {
                collapsed_edges: 1,
                ..Adaptations::default()
            },
            PI / 3. * 1e-6,
        ),
        // Only a VERTEX_LOOP: the face is re-charted with its vertex at a pole and
        // spans both pole lines, joined by a seam.
        (
            "full sphere",
            include_str!("../../../corpus/geometry/brep-sphere.step"),
            Adaptations {
                recharted_spheres: 1,
                ..seam(2)
            },
            4. / 3. * PI * 1e-6,
        ),
        // A bilinear B-spline base with one side collapsed to a corner of the loop.
        (
            "collapsed B-spline side",
            include_str!("../../../corpus/geometry/brep-bspline-pole.step"),
            Adaptations {
                collapsed_edges: 1,
                ..Adaptations::default()
            },
            1e-6 / 6.,
        ),
        // A pole-to-pole meridian seam: exact meridian pcurves and a join per pole.
        (
            "meridian seam",
            include_str!("../../../corpus/geometry/brep-sphere-seam.step"),
            Adaptations {
                collapsed_edges: 2,
                ..Adaptations::default()
            },
            4. / 3. * PI * 1e-6,
        ),
    ] {
        let solid = import(text).unwrap();
        assert_eq!(solid.adaptations(), adaptations, "{name}");
        let n = solid.brep();
        let collapsed: Vec<_> = (0..n.data().edges.len())
            .filter(|&e| n.is_collapsed(tessstep_topology::EdgeId(e)))
            .collect();
        assert_eq!(collapsed.len(), adaptations.collapsed_edges, "{name}");
        let mesh = mesh(&solid, chord);
        check_volume(&mesh, volume, chord, 0.01);
        // Each pole is one welded mesh vertex, whatever its chart samples.
        for &e in &collapsed {
            let pole = n.data().vertices[n.data().edges[e].vertices[0].0]
                .position
                .coordinates();
            let at = mesh.data().positions.iter().filter(|p| *p == &pole).count();
            assert_eq!(at, 1, "{name}");
        }
        let again = import(text).unwrap();
        assert_eq!(
            mesh.data().triangles,
            self::mesh(&again, chord).data().triangles,
            "{name}"
        );
    }
}

#[test]
fn brep_swept_surfaces_are_exact_elementary_or_nurbs_charts() {
    use tessstep_topology::SurfaceGeometry;
    let chord = 1e-5;
    for (name, text, nurbs, adaptations, volume, radius) in [
        // An extruded circle along its normal is a cylinder.
        (
            "extruded circle",
            include_str!("../../../corpus/geometry/brep-extruded-cylinder.step"),
            false,
            Adaptations {
                inserted_seams: 1,
                ..Adaptations::default()
            },
            PI * 2000e-9,
            0.01,
        ),
        // A reversed trimmed ellipse: a periodic NURBS extrusion whose STEP normal
        // points inward, cut by an isocurve seam.
        (
            "extruded ellipse",
            include_str!("../../../corpus/geometry/brep-extruded-ellipse.step"),
            true,
            Adaptations {
                inserted_seams: 1,
                ..Adaptations::default()
            },
            PI * 3000e-9,
            0.01,
        ),
        // A revolved generator line is a cone, closed at its apex.
        (
            "revolved line",
            include_str!("../../../corpus/geometry/brep-revolved-cone.step"),
            false,
            Adaptations {
                inserted_seams: 1,
                collapsed_edges: 1,
                ..Adaptations::default()
            },
            PI / 3. * 1e-6,
            0.01,
        ),
        // A revolved B-spline semicircle: NURBS whose profile ends collapse to poles.
        (
            "revolved B-spline",
            include_str!("../../../corpus/geometry/brep-revolved-sphere.step"),
            true,
            Adaptations {
                collapsed_edges: 2,
                ..Adaptations::default()
            },
            4. / 3. * PI * 1e-6,
            0.01,
        ),
        // A circle revolved in a plane through the axis is a ring torus.
        (
            "revolved circle",
            include_str!("../../../corpus/geometry/brep-revolved-torus.step"),
            false,
            Adaptations::default(),
            2. * PI * PI * 20. * 25. * 1e-9,
            0.005,
        ),
    ] {
        let solid = import(text).unwrap();
        assert_eq!(solid.adaptations(), adaptations, "{name}");
        assert_eq!(
            matches!(
                solid.brep().data().faces[0].surface,
                SurfaceGeometry::Nurbs(_)
            ),
            nurbs,
            "{name}"
        );
        check_volume(&mesh(&solid, chord), volume, chord, radius);
    }
    // Outward normals follow the swept curve's sense: reversing the trimmed curve's
    // sense without flipping the face turns the prism inside out.
    let inverted = include_str!("../../../corpus/geometry/brep-extruded-ellipse.step")
        .replace(".F.,.PARAMETER.)", ".T.,.PARAMETER.)");
    let tolerance =
        TessellationTolerance::new(Length::metres(chord).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    assert!(
        import(&inverted)
            .and_then(|s| s.tessellate(tolerance, TessellationOptions::default()))
            .is_err()
    );
}

#[test]
fn brep_cavity_shells_mesh_as_inward_components() {
    let chord = 1e-5;
    for (text, volume, radius) in [
        (
            include_str!("../../../corpus/geometry/brep-box-cavity.step"),
            7000e-9,
            f64::INFINITY,
        ),
        (
            include_str!("../../../corpus/geometry/brep-sphere-cavity.step"),
            PI * 2000e-9 - 4. / 3. * PI * 125e-9,
            0.005,
        ),
    ] {
        let solid = import(text).unwrap();
        let data = solid.brep().data();
        assert_eq!((data.shells.len(), data.solids[0].voids.len()), (2, 1));
        let tolerance = TessellationTolerance::new(
            Length::metres(chord).unwrap(),
            Angle::radians(0.1).unwrap(),
        )
        .unwrap();
        let mesh = solid
            .tessellate(tolerance, TessellationOptions::default())
            .unwrap();
        assert!(mesh.is_watertight());
        assert_eq!(mesh.statistics().components, 2);
        let v = mesh.statistics().signed_volume;
        assert!(
            ((v - volume) / volume).abs() <= 2. * chord / radius + 1e-12,
            "{v} vs {volume}"
        );
    }
    // A cavity shell facing out of its cavity is diagnosed, never meshed.
    let solid = import(include_str!(
        "../../../corpus/geometry/brep-inverted-cavity.step"
    ))
    .unwrap();
    let tolerance =
        TessellationTolerance::new(Length::metres(chord).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    let error = solid
        .tessellate(tolerance, TessellationOptions::default())
        .unwrap_err();
    assert!(error.message.contains("CavityOrientation"), "{error}");
}

#[test]
fn brep_bspline_faces_and_rational_complex_instances() {
    let solid = import(BSPLINE_CUBE).unwrap();
    let mesh = mesh(&solid, 1e-6);
    assert!((mesh.statistics().signed_volume - 1e-6).abs() < 1e-18);
    assert_eq!(
        (mesh.data().positions.len(), mesh.data().triangles.len()),
        (8, 12)
    );
}

#[test]
fn brep_matches_the_planar_profile_on_planar_input() {
    let text = include_str!("../../../corpus/geometry/planar-box.step");
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    let planar = import_planar_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        model_tolerance(),
        ImportOptions::default(),
    )
    .unwrap();
    let brep = import(text).unwrap();
    let (a, b) = (mesh(&planar, 1e-6), mesh(&brep, 1e-6));
    // Rectangles may choose either diagonal; the geometry and provenance agree.
    assert_eq!(a.data().positions, b.data().positions);
    assert_eq!(a.data().triangles.len(), b.data().triangles.len());
    assert!((a.statistics().signed_volume - b.statistics().signed_volume).abs() < 1e-18);
    let count = |m: &tessstep_mesh::Mesh| {
        let mut ids = m.data().face_ids.clone();
        ids.sort();
        ids
    };
    assert_eq!(count(&a), count(&b));
}

#[test]
fn brep_rejections_are_typed_and_located() {
    let cases = [
        (
            include_str!("../../../corpus/geometry/brep-off-surface.step").to_string(),
            ErrorKind::InvalidGeometry,
            "lies 5.000e-4 m from the face surface",
        ),
        (
            include_str!("../../../corpus/geometry/brep-regular-vertex-loop.step").to_string(),
            ErrorKind::Unsupported,
            "VERTEX_LOOP vertex is not at a pole or apex",
        ),
        (
            // A cylinder side face bounded by its bottom circle alone encloses no pole.
            CYLINDER.replace("ADVANCED_FACE('',(#24,#27),", "ADVANCED_FACE('',(#24),"),
            ErrorKind::Unsupported,
            "without enclosing a pole or apex",
        ),
        (
            include_str!("../../../corpus/geometry/brep-ambiguous-outer.step").to_string(),
            ErrorKind::InvalidGeometry,
            "ambiguous outer bound",
        ),
        (
            TORUS.replace("20.,5.)", "5.,20.)"),
            ErrorKind::Unsupported,
            "horn and spindle tori",
        ),
        (
            // 30 read as radians is not a cone semi-angle.
            CONE.to_string(),
            ErrorKind::InvalidGeometry,
            "CONICAL_SURFACE needs",
        ),
    ];
    for (text, kind, message) in cases {
        let error = import(&text).unwrap_err();
        assert_eq!(
            (error.kind, error.stage),
            (kind, Stage::Geometry),
            "{error}"
        );
        assert!(error.message.contains(message), "{error}");
        assert!(error.entity.is_some() && error.source.is_some());
    }
    // BREP_WITH_VOIDS whose cavity reuses the outer shell cannot own it twice.
    let shell = CYLINDER
        .lines()
        .find(|l| l.starts_with("#1000="))
        .and_then(|l| l.split('#').nth(2))
        .and_then(|s| s.split(')').next())
        .unwrap();
    let voids = CYLINDER.replace(
        &format!("#1000=MANIFOLD_SOLID_BREP('',#{shell});"),
        &format!("#1000=BREP_WITH_VOIDS('',#{shell},(#1001));#1001=ORIENTED_CLOSED_SHELL('',*,#{shell},.F.);"),
    );
    assert_ne!(voids, CYLINDER);
    let error = import(&voids).unwrap_err();
    assert_eq!(error.stage, Stage::Topology, "{error}");
    // Types outside the profile are named.
    let offset = CYLINDER.replacen("CYLINDRICAL_SURFACE('',", "OFFSET_SURFACE('',", 1);
    let error = import(&offset).unwrap_err();
    assert_eq!(
        (error.kind, error.stage),
        (ErrorKind::Unsupported, Stage::Profile)
    );
    assert_eq!(
        error.message,
        "OFFSET_SURFACE is outside the tessstep_brep import profile"
    );
}

#[test]
fn brep_unset_derived_slots_are_tolerated_unless_strict() {
    let unset = CYLINDER.replace("ORIENTED_EDGE('',*,*,", "ORIENTED_EDGE('',$,$,");
    assert_ne!(unset, CYLINDER);
    let a = mesh(&import(&unset).unwrap(), 1e-5);
    let b = mesh(&import(CYLINDER).unwrap(), 1e-5);
    assert_eq!(a.data().positions, b.data().positions);
    let strict = ImportOptions {
        strict: true,
        ..ImportOptions::default()
    };
    let error = import_with(&unset, AngleUnit::RADIAN, strict).unwrap_err();
    assert_eq!(error.stage, Stage::Profile);
    import_with(CYLINDER, AngleUnit::RADIAN, strict).unwrap();
}

#[test]
fn brep_budgets_record_order_and_mutation_smoke() {
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
            import_with(CYLINDER, AngleUnit::RADIAN, options)
                .unwrap_err()
                .kind,
            ErrorKind::ResourceLimit
        );
    }
    for text in [CYLINDER, WASHER, BSPLINE_CUBE] {
        let (header, data) = text.split_once("DATA;\n").unwrap();
        let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
        records.reverse();
        let reordered = format!(
            "{header}DATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
            records.join("\n")
        );
        let a = mesh(&import(text).unwrap(), 1e-5);
        let b = mesh(&import(&reordered).unwrap(), 1e-5);
        assert_eq!(a.data().positions, b.data().positions);
        assert_eq!(a.data().triangles, b.data().triangles);
    }
    let mut seed = 91u64;
    for text in [CYLINDER, SEAM_CYLINDER, BSPLINE_CUBE] {
        for _ in 0..150 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mut bytes = text.as_bytes().to_vec();
            let at = seed as usize % bytes.len();
            bytes[at] = (seed >> 32) as u8;
            if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
                let _ = import_brep_solid(
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
                );
            }
        }
    }
}

fn discover(text: &str) -> Vec<SolidRoot> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    discover_solids(&doc, ImportOptions::default()).unwrap()
}

#[test]
fn discovery_reads_context_units_and_uncertainty() {
    let roots = discover(CYLINDER);
    assert_eq!(roots.len(), 1);
    let root = &roots[0];
    assert_eq!(
        (root.entity.get(), root.kind, root.representations.len()),
        (1000, SolidKind::ManifoldSolidBrep, 1)
    );
    let units = root.units.clone().unwrap();
    assert_eq!(units.length, LengthUnit::MILLIMETRE);
    assert_eq!(units.plane_angle, AngleUnit::RADIAN);
    let uncertainty = units.distance_uncertainty.unwrap();
    assert!((uncertainty - 1e-8).abs() < 1e-22);
    // A conversion-based degree unit scales the cone semi-angle.
    let cone = discover(CONE)[0].units.clone().unwrap();
    assert!((cone.plane_angle.scale() - PI / 180.).abs() < 1e-15);
    let solid = import_with(CONE, cone.plane_angle, ImportOptions::default()).unwrap();
    assert_eq!(solid.adaptations().inserted_seams, 1);
    // Conversion-based length units compose with their SI base.
    let inch = CYLINDER.replace(
        "#2001=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));",
        "#2001=(CONVERSION_BASED_UNIT('INCH',#2011)LENGTH_UNIT()NAMED_UNIT(#2010));\
         #2010=DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);\
         #2011=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(25.4),#2012);\
         #2012=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));",
    );
    assert_ne!(inch, CYLINDER);
    let units = discover(&inch)[0].units.clone().unwrap();
    assert!((units.length.scale() - 0.0254).abs() < 1e-15);
    assert!((units.distance_uncertainty.unwrap() - 2.54e-7).abs() < 1e-20);
}

#[test]
fn discovery_reports_missing_and_conflicting_units_without_defaults() {
    // No representation contains this root.
    let roots = discover(SEAM_CYLINDER);
    assert_eq!(roots.len(), 1);
    let error = roots[0].units.clone().unwrap_err();
    assert!(
        error
            .message
            .contains("no supported representation contains the root")
    );
    // A second representation in metres conflicts with the millimetre context.
    let conflicting = CYLINDER.replace(
            "#2009=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#1000),#2008);",
            "#2009=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#1000),#2008);\
             #2020=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT($,.METRE.));\
             #2021=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#2020,#2002,#2006))REPRESENTATION_CONTEXT('',''));\
             #2022=SHAPE_REPRESENTATION('',(#1000),#2021);",
        );
    let roots = discover(&conflicting);
    assert_eq!(roots[0].representations.len(), 2);
    let error = roots[0].units.clone().unwrap_err();
    assert!(error.message.contains("different units"), "{error}");
    // A context without a plane-angle unit is rejected, not defaulted.
    let incomplete = CYLINDER.replace(
        "GLOBAL_UNIT_ASSIGNED_CONTEXT((#2001,#2002,#2006))",
        "GLOBAL_UNIT_ASSIGNED_CONTEXT((#2001,#2006))",
    );
    let error = discover(&incomplete)[0].units.clone().unwrap_err();
    assert!(error.message.contains("no plane angle unit"), "{error}");
}

#[test]
fn brep_unmodified_exporter_cube_with_hole() {
    let root = std::env::var_os("TESSSTEP_CORPUS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("step-corpus"))
        });
    let Some(path) = root
        .map(|r| r.join("vendor/foxtrot/examples/cube_hole.step"))
        .filter(|p| p.is_file())
    else {
        eprintln!("external cube_hole unavailable; authored B-rep tests still run");
        return;
    };
    let doc = tessstep_model::parse(
        std::io::BufReader::new(std::fs::File::open(path).unwrap()),
        ParseLimits::default(),
    )
    .unwrap();
    let roots = discover_solids(&doc, ImportOptions::default()).unwrap();
    assert_eq!(roots.len(), 1);
    let units = roots[0].units.clone().unwrap();
    assert_eq!(units.length, LengthUnit::METRE);
    let tolerance = ModelTolerance::new(
        Length::metres(units.distance_uncertainty.unwrap()).unwrap(),
        Angle::radians(1e-8).unwrap(),
    )
    .unwrap();
    let solid = import_brep_solid(
        &doc,
        roots[0].entity,
        units.length,
        units.plane_angle,
        tolerance,
        ImportOptions::default(),
    )
    .unwrap();
    assert_eq!(
        solid.adaptations(),
        Adaptations {
            inferred_outer_bounds: 2,
            inserted_seams: 1,
            ..Adaptations::default()
        }
    );
    let mesh = mesh(&solid, 1e-6);
    let exact = 0.0508 * 0.0254 * 0.0254 - PI * 0.00635 * 0.00635 * 0.0254;
    check_volume(&mesh, exact, 1e-6, 0.00635);
}
