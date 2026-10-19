use std::f64::consts::PI;
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../../../corpus/geometry/brep-repair-",
            $name,
            ".step"
        ))
    };
}
const DUPLICATED: &str = fixture!("duplicated");
const BROKEN_WIRE: &str = fixture!("broken-wire");
const FLIPPED_FACE: &str = fixture!("flipped-face");
const REVERSED_LOOP: &str = fixture!("reversed-loop");
const WRONG_SENSE: &str = fixture!("wrong-sense");
const INSIDE_OUT: &str = fixture!("inside-out");
const UNSEWN: &str = fixture!("unsewn");
const UNSEWN_CYLINDER: &str = fixture!("unsewn-cylinder");
const UNSEWN_GAP: &str = fixture!("unsewn-gap");
const GAP: &str = fixture!("gap");
const INVERTED_CAVITY: &str = include_str!("../../../corpus/geometry/brep-inverted-cavity.step");
const BOX_VOLUME: f64 = 6e-6;
const BOX_AREA: f64 = 2. * (200. + 300. + 600.) * 1e-6;

fn model_tolerance() -> ModelTolerance {
    ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap()
}
fn options(repair: RepairOptions) -> ImportOptions {
    ImportOptions {
        repair,
        ..ImportOptions::default()
    }
}
fn import(text: &str, repair: RepairOptions) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_brep_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        AngleUnit::RADIAN,
        model_tolerance(),
        options(repair),
    )
}
fn planar(text: &str, repair: RepairOptions) -> Result<ImportedSolid, Error> {
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    import_planar_solid(
        &doc,
        EntityId::new(1000).unwrap(),
        LengthUnit::MILLIMETRE,
        model_tolerance(),
        options(repair),
    )
}
fn mesh(solid: &ImportedSolid) -> Result<tessstep_mesh::Mesh, Error> {
    let tolerance =
        TessellationTolerance::new(Length::metres(1e-5).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    solid.tessellate(tolerance, TessellationOptions::default())
}
fn repairs(merged_vertices: usize, merged_edges: usize) -> RepairReport {
    RepairReport {
        merged_vertices,
        merged_edges,
        sewing_tolerance: 1e-7,
        ..RepairReport::default()
    }
}
/// The report without its measured merge distance, which is checked separately.
fn counts(solid: &ImportedSolid) -> RepairReport {
    RepairReport {
        max_merge_distance: 0.,
        ..solid.repairs()
    }
}

#[test]
fn repair_is_opt_in_and_strict_import_rejects_every_defect() {
    assert_eq!(ImportOptions::default().repair, RepairOptions::default());
    assert!(!RepairOptions::default().is_enabled());
    assert_eq!(
        ImportOptions::default().repair.sewing_factor,
        RepairOptions::DEFAULT_SEWING_FACTOR
    );
    for (text, stage, message) in [
        (DUPLICATED, Stage::Topology, "OpenShell at shell 0"),
        (BROKEN_WIRE, Stage::Topology, "BrokenWire at wire 1"),
        (
            FLIPPED_FACE,
            Stage::Topology,
            "InconsistentOrientation at shell 0",
        ),
        (
            REVERSED_LOOP,
            Stage::Topology,
            "InconsistentOrientation at shell 0",
        ),
        (UNSEWN, Stage::Topology, "UnsewnShell at shell 0"),
        (UNSEWN_CYLINDER, Stage::Topology, "UnsewnShell at shell 0"),
        (UNSEWN_GAP, Stage::Topology, "UnsewnShell at shell 0"),
        (GAP, Stage::Topology, "OpenShell at shell 0"),
    ] {
        let e = import(text, RepairOptions::default()).unwrap_err();
        assert_eq!((e.stage, e.kind), (stage, ErrorKind::InvalidGeometry));
        assert_eq!(e.message, message);
        assert_eq!(e.entity, EntityId::new(1000));
    }
    // Inside-out shells pass topology and are rejected by the volume check, and a
    // loop running against its stated face normal by the trimmer.
    for text in [INSIDE_OUT, INVERTED_CAVITY, WRONG_SENSE] {
        let solid = import(text, RepairOptions::default()).unwrap();
        assert_eq!(solid.repairs(), RepairReport::default());
        assert_eq!(mesh(&solid).unwrap_err().stage, Stage::Tessellation);
    }
}

#[test]
fn repair_sews_duplicated_and_unsewn_boundaries() {
    let on = RepairOptions::enabled();
    let duplicated = import(DUPLICATED, on).unwrap();
    assert_eq!(counts(&duplicated), repairs(1, 3));
    // The copied corner sits 5e-5 mm from the original; the sewn B-rep is checked at
    // the model tolerance plus that distance, and the vertex keeps one position.
    let distance = duplicated.repairs().max_merge_distance;
    assert!((distance - 5e-8).abs() < 1e-12, "{distance}");
    assert!((duplicated.brep().model_tolerance() - (1e-8 + distance)).abs() < 1e-20);
    let volume = mesh(&duplicated).unwrap().statistics().signed_volume;
    assert!(
        (volume - BOX_VOLUME).abs() <= distance * BOX_AREA,
        "{volume}"
    );

    let broken = import(BROKEN_WIRE, on).unwrap();
    assert_eq!(counts(&broken), repairs(1, 0));
    assert_eq!(broken.brep().data().vertices.len(), 8);

    let unsewn = import(UNSEWN, on).unwrap();
    assert_eq!(
        counts(&unsewn),
        RepairReport {
            disconnected_shells: 1,
            ..repairs(16, 12)
        }
    );
    assert!(unsewn.repairs().max_merge_distance < 1e-15);
    let data = unsewn.brep().data();
    assert_eq!((data.vertices.len(), data.edges.len()), (8, 12));
    let m = mesh(&unsewn).unwrap();
    m.require_solid().unwrap();
    assert!((m.statistics().signed_volume - BOX_VOLUME).abs() < 1e-15);

    // Curved closed edges sew too, one of them running the other way round, and the
    // pcurves of the surviving circles are computed afterwards (one annulus seam).
    let cylinder = import(UNSEWN_CYLINDER, on).unwrap();
    assert_eq!(
        counts(&cylinder),
        RepairReport {
            disconnected_shells: 1,
            ..repairs(2, 2)
        }
    );
    assert_eq!(cylinder.adaptations().inserted_seams, 1);
    let m = mesh(&cylinder).unwrap();
    m.require_solid().unwrap();
    let exact = PI * 100. * 20. * 1e-9;
    assert!(((m.statistics().signed_volume - exact) / exact).abs() <= 2. * 1e-5 / 0.01);
}

#[test]
fn repair_reorients_flipped_faces_and_inside_out_shells() {
    let on = RepairOptions::enabled();
    let flipped = import(FLIPPED_FACE, on).unwrap();
    assert_eq!(
        counts(&flipped),
        RepairReport {
            flipped_faces: 1,
            ..repairs(0, 0)
        }
    );
    assert!((mesh(&flipped).unwrap().statistics().signed_volume - BOX_VOLUME).abs() < 1e-15);
    // A loop written clockwise about its face normal (whose uses then disagree with
    // the neighbours) and a wrong SAME_SENSE (whose loop then runs against the stated
    // normal) both reverse the face's loops; the shell keeps its incidence.
    for text in [REVERSED_LOOP, WRONG_SENSE] {
        let solid = import(text, on).unwrap();
        assert_eq!(
            counts(&solid),
            RepairReport {
                reversed_loops: 1,
                ..repairs(0, 0)
            }
        );
        assert!((mesh(&solid).unwrap().statistics().signed_volume - BOX_VOLUME).abs() < 1e-15);
    }
    let inside_out = import(INSIDE_OUT, on).unwrap();
    assert_eq!(
        counts(&inside_out),
        RepairReport {
            reoriented_shells: 1,
            ..repairs(0, 0)
        }
    );
    assert!((mesh(&inside_out).unwrap().statistics().signed_volume - BOX_VOLUME).abs() < 1e-15);
    // A cavity shell facing out of its cavity is reversed; the outer shell is kept.
    let cavity = import(INVERTED_CAVITY, on).unwrap();
    assert_eq!(cavity.repairs().reoriented_shells, 1);
    let m = mesh(&cavity).unwrap();
    assert_eq!(m.statistics().components, 2);
    assert!((m.statistics().signed_volume - 7e-6).abs() < 1e-15);
    // Orientation alone: the flipped face is repaired, the duplicated edges are not.
    let orient = RepairOptions { sew: false, ..on };
    assert_eq!(
        import(FLIPPED_FACE, orient)
            .unwrap()
            .repairs()
            .flipped_faces,
        1
    );
    let e = import(DUPLICATED, orient).unwrap_err();
    assert_eq!(e.message, "OpenShell at shell 0");
}

#[test]
fn repair_rejects_gaps_beyond_the_sewing_tolerance_with_the_distance() {
    let on = RepairOptions::enabled();
    let e = import(GAP, on).unwrap_err();
    assert_eq!(
        (e.stage, e.kind),
        (Stage::Topology, ErrorKind::InvalidGeometry)
    );
    assert!(
        e.message.starts_with("OpenShell at shell 0: ")
            && e.message
                .contains("needs 5.000e-5 m; sewing tolerance 1.000e-7 m"),
        "{}",
        e.message
    );
    // Located at a free STEP edge, not the root.
    assert_ne!(e.entity, EntityId::new(1000));
    // A disconnected closed shell beyond the tolerance keeps its own classification.
    let e = import(UNSEWN_GAP, on).unwrap_err();
    assert!(
        e.message.starts_with("UnsewnShell at shell 0: ") && e.message.contains("needs 1.414e-5 m"),
        "{}",
        e.message
    );
    // A tighter sewing factor rejects the duplicated corner 5e-8 m away by name.
    let tight = RepairOptions {
        sewing_factor: 1.,
        ..on
    };
    let e = import(DUPLICATED, tight).unwrap_err();
    assert!(
        e.message
            .contains("needs 5.000e-8 m; sewing tolerance 1.000e-8 m"),
        "{}",
        e.message
    );
    // An absolute sewing distance of 0.1 mm, as PreForm asks HOOPS Exchange to sew,
    // raises the tolerance above the factor's 1e-7 m and closes the 0.05 mm gap, in
    // the curved and the planar profile alike.
    let distance = RepairOptions {
        sewing_distance: 1e-4,
        ..on
    };
    for solid in [import(GAP, distance), planar(GAP, distance)] {
        let solid = solid.unwrap();
        assert_eq!(
            counts(&solid),
            RepairReport {
                sewing_tolerance: 1e-4,
                ..repairs(1, 2)
            }
        );
        assert!((solid.repairs().max_merge_distance - 5e-5).abs() < 1e-12);
        let mesh = mesh(&solid).unwrap();
        mesh.require_solid().unwrap();
        // The corner keeps one of its two positions, 0.05 mm apart: moving one corner
        // of the 20 x 30 mm face changes the volume by about 0.05 x 20 x 30 / 3 mm3.
        let volume = mesh.statistics().signed_volume;
        assert!((volume - BOX_VOLUME).abs() <= 1.5e-8, "{volume}");
    }
    for sewing_distance in [-1e-6, f64::NAN] {
        let e = import(
            DUPLICATED,
            RepairOptions {
                sewing_distance,
                ..on
            },
        )
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::InvalidOptions, "{e}");
        let e = planar(
            DUPLICATED,
            RepairOptions {
                sewing_distance,
                ..on
            },
        )
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::InvalidOptions, "{e}");
    }
    for sewing_factor in [0., f64::NAN] {
        let e = import(
            DUPLICATED,
            RepairOptions {
                sewing_factor,
                ..on
            },
        )
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::InvalidOptions);
    }
}

#[test]
fn repair_applies_to_the_planar_and_faceted_profiles() {
    let on = RepairOptions::enabled();
    for (text, expected) in [
        (DUPLICATED, repairs(1, 3)),
        (BROKEN_WIRE, repairs(1, 0)),
        (
            UNSEWN,
            RepairReport {
                disconnected_shells: 1,
                ..repairs(16, 12)
            },
        ),
        (
            FLIPPED_FACE,
            RepairReport {
                flipped_faces: 1,
                ..repairs(0, 0)
            },
        ),
        (
            INSIDE_OUT,
            RepairReport {
                reoriented_shells: 1,
                ..repairs(0, 0)
            },
        ),
        (
            REVERSED_LOOP,
            RepairReport {
                reversed_loops: 1,
                ..repairs(0, 0)
            },
        ),
    ] {
        assert!(
            planar(text, RepairOptions::default())
                .and_then(|s| mesh(&s))
                .is_err()
        );
        let solid = planar(text, on).unwrap();
        assert_eq!(counts(&solid), expected);
        let volume = mesh(&solid).unwrap().statistics().signed_volume;
        assert!(
            (volume - BOX_VOLUME).abs() <= 1e-15 + solid.repairs().max_merge_distance * BOX_AREA
        );
    }
    assert!(
        planar(GAP, on)
            .unwrap_err()
            .message
            .contains("needs 5.000e-5 m")
    );
    // A faceted box whose six POLY_LOOPs each own their four points.
    let box_ = include_str!("../../../corpus/geometry/box.step");
    let mut unsewn = String::new();
    let mut next = 5000;
    for line in box_.lines() {
        if let Some(rest) = line.strip_prefix('#').filter(|l| l.contains("=POLY_LOOP(")) {
            let (id, body) = rest.split_once('=').unwrap();
            let inner = body.split_once(",(").unwrap().1.trim_end_matches("));");
            let mut points = Vec::new();
            for point in inner.split(',') {
                let defined = box_
                    .lines()
                    .find(|l| l.starts_with(&format!("{point}=")))
                    .unwrap();
                unsewn.push_str(&format!("#{next}={}\n", defined.split_once('=').unwrap().1));
                points.push(format!("#{next}"));
                next += 1;
            }
            unsewn.push_str(&format!("#{id}=POLY_LOOP('',({}));\n", points.join(",")));
        } else {
            unsewn.push_str(line);
            unsewn.push('\n');
        }
    }
    let doc = tessstep_model::parse(unsewn.as_bytes(), ParseLimits::default()).unwrap();
    let faceted = |repair| {
        import_faceted_solid(
            &doc,
            EntityId::new(1000).unwrap(),
            LengthUnit::MILLIMETRE,
            model_tolerance(),
            options(repair),
        )
    };
    assert_eq!(
        faceted(RepairOptions::default()).unwrap_err().message,
        "UnsewnShell at shell 0"
    );
    let solid = faceted(on).unwrap();
    assert_eq!(
        counts(&solid),
        RepairReport {
            disconnected_shells: 1,
            ..repairs(16, 12)
        }
    );
    assert!((mesh(&solid).unwrap().statistics().signed_volume - 6e-6).abs() < 1e-15);
}

#[test]
fn repair_leaves_valid_solids_unchanged() {
    for name in [
        "cylinder",
        "washer",
        "vertex-loop",
        "box-cavity",
        "bspline-cube",
    ] {
        let text = std::fs::read_to_string(format!(
            "{}/../../corpus/geometry/brep-{name}.step",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let strict = import(&text, RepairOptions::default()).unwrap();
        let repaired = import(&text, RepairOptions::enabled()).unwrap();
        assert!(
            repaired.repairs().is_empty(),
            "{name}: {:?}",
            repaired.repairs()
        );
        assert_eq!(repaired.repairs().undecided_shells, 0, "{name}");
        assert_eq!(strict.brep().data(), repaired.brep().data(), "{name}");
        assert_eq!(
            mesh(&strict).unwrap().data().positions,
            mesh(&repaired).unwrap().data().positions
        );
    }
}

#[test]
fn repair_budgets_determinism_and_mutation_smoke() {
    let options = |max_work, max_records| ImportOptions {
        max_work,
        max_records,
        repair: RepairOptions::enabled(),
        ..ImportOptions::default()
    };
    let run = |doc: &tessstep_model::Document, options| {
        import_brep_solid(
            doc,
            EntityId::new(1000).unwrap(),
            LengthUnit::MILLIMETRE,
            AngleUnit::RADIAN,
            model_tolerance(),
            options,
        )
    };
    // A repair-on import keeps the import budgets (each repair pass charges its own
    // `max_work` counter; `repair_budgets_and_invalid_models_are_explicit` in
    // tessstep-topology exhausts the sewing one).
    let doc = tessstep_model::parse(UNSEWN.as_bytes(), ParseLimits::default()).unwrap();
    let defaults = ImportOptions::default();
    assert!(run(&doc, options(defaults.max_work, defaults.max_records)).is_ok());
    assert_eq!(
        run(&doc, options(0, defaults.max_records))
            .unwrap_err()
            .kind,
        ErrorKind::ResourceLimit
    );
    // Record order does not change the repaired model.
    for text in [UNSEWN, DUPLICATED, REVERSED_LOOP] {
        let (header, data) = text.split_once("DATA;\n").unwrap();
        let mut records: Vec<_> = data.lines().filter(|s| s.starts_with('#')).collect();
        records.reverse();
        let reordered = format!(
            "{header}DATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
            records.join("\n")
        );
        let a = import(text, RepairOptions::enabled()).unwrap();
        let b = import(&reordered, RepairOptions::enabled()).unwrap();
        assert_eq!(counts(&a), counts(&b));
        let (ma, mb) = (mesh(&a).unwrap(), mesh(&b).unwrap());
        assert!((ma.statistics().signed_volume - mb.statistics().signed_volume).abs() < 1e-15);
    }
    // Byte mutations never panic, and repeat deterministically.
    let mut seed = 2205u64;
    for text in [UNSEWN, DUPLICATED, BROKEN_WIRE, UNSEWN_CYLINDER] {
        for _ in 0..60 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mut bytes = text.as_bytes().to_vec();
            let at = seed as usize % bytes.len();
            bytes[at] = (seed >> 32) as u8;
            if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
                let repairs = || run(&doc, options(200_000, 1000)).map(|s| s.repairs());
                assert_eq!(repairs(), repairs());
            }
        }
    }
}
