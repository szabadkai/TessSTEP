//! Document import sessions: shared discovery and decoding, identical results to the
//! free functions, and named session budgets.
use std::collections::BTreeSet;
use std::fmt::Write;
use tessstep_import::*;
use tessstep_math::{Angle, Length, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const WASHER: &str = include_str!("../../../corpus/geometry/brep-washer.step");
const SELECTION: &str = include_str!("../../../corpus/geometry/tessellated-selection.step");
const PRESENTATION: &str = include_str!("../../../corpus/geometry/presentation.step");
/// One millimetre context with a 0.1 µm distance uncertainty, shared by every copy.
const CONTEXT: &str = "#9000001=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));
#9000002=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));
#9000003=(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT());
#9000004=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-4),#9000001,'DISTANCE_ACCURACY_VALUE','');
#9000005=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#9000004))GLOBAL_UNIT_ASSIGNED_CONTEXT((#9000001,#9000002,#9000003))REPRESENTATION_CONTEXT('',''));
";
/// DIRECTION records of the washer, which every copy after the first reuses.
const SHARED: usize = 16;

fn parse(text: &str) -> tessstep_model::Document {
    tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap()
}

/// `#id` references of `line` moved by `offset`, except the `shared` records.
fn renumber(line: &str, offset: u64, shared: &BTreeSet<u64>) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(i) = rest.find('#') {
        out.push_str(&rest[..=i]);
        rest = &rest[i + 1..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let id: u64 = rest[..end].parse().unwrap();
        let _ = write!(
            out,
            "{}",
            if shared.contains(&id) {
                id
            } else {
                id + offset
            }
        );
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `copies` washers in one document, each in its own ADVANCED_BREP_SHAPE_REPRESENTATION
/// of one shared context. Copies after the first reuse the first copy's DIRECTION
/// records, so the roots' closures share records. The `open` copy's shell is a
/// CONNECTED_FACE_SET, outside the curved B-rep profile.
fn washers(copies: u64, open: Option<u64>) -> String {
    let (header, rest) = WASHER.split_once("DATA;\n").unwrap();
    let body = rest.split_once("ENDSEC;").unwrap().0;
    let id = |line: &str| -> u64 { line[1..line.find('=').unwrap()].parse().unwrap() };
    let shared: BTreeSet<u64> = body
        .lines()
        .filter(|l| l.contains("=DIRECTION("))
        .map(id)
        .collect();
    assert_eq!(shared.len(), SHARED);
    let mut data = String::new();
    for k in 0..copies {
        for line in body.lines() {
            if k > 0 && shared.contains(&id(line)) {
                continue;
            }
            let mut line = renumber(line, k * 10_000, &shared);
            if open == Some(k) {
                line = line.replace("CLOSED_SHELL", "CONNECTED_FACE_SET");
            }
            data.push_str(&line);
            data.push('\n');
        }
        let _ = writeln!(
            data,
            "#{}=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{}),#9000005);",
            9_100_000 + k,
            1000 + k * 10_000
        );
    }
    format!("{header}DATA;\n{CONTEXT}{data}ENDSEC;\nEND-ISO-10303-21;\n")
}

fn model(units: &ContextUnits) -> ModelTolerance {
    let distance = units.distance_uncertainty.unwrap().max(1e-7);
    ModelTolerance::new(
        Length::metres(distance).unwrap(),
        Angle::radians(1e-8).unwrap(),
    )
    .unwrap()
}

fn chord() -> TessellationTolerance {
    TessellationTolerance::new(Length::metres(1e-4).unwrap(), Angle::radians(0.1).unwrap()).unwrap()
}

/// Free-function and session results compared through their complete Debug output,
/// which prints every float exactly.
fn same<T: std::fmt::Debug>(a: &T, b: &T) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

#[test]
fn a_shared_decode_serves_every_root_with_the_free_function_results() {
    let doc = parse(&washers(4, None));
    let options = ImportOptions::default();
    // The decoded closure of one root alone.
    let single = ImportSession::new(&doc, options);
    let units = single.solids().unwrap()[0].units.clone().unwrap();
    single
        .import_brep_solid(
            EntityId::new(1000).unwrap(),
            units.length,
            units.plane_angle,
            model(&units),
        )
        .unwrap();
    let closure = single.statistics().decoded_entities;
    assert!(closure > SHARED);

    let session = ImportSession::new(&doc, options);
    let roots: Vec<EntityId> = session.solids().unwrap().iter().map(|r| r.entity).collect();
    assert_eq!(roots.len(), 4);
    assert_eq!(session.prepare(ImportProfile::Brep, &roots), 4);
    // Already prepared roots keep their decode.
    assert_eq!(session.prepare(ImportProfile::Brep, &roots), 0);
    for root in session.solids().unwrap() {
        let units = root.units.clone().unwrap();
        let shared =
            session.import_brep_solid(root.entity, units.length, units.plane_angle, model(&units));
        let alone = import_brep_solid(
            &doc,
            root.entity,
            units.length,
            units.plane_angle,
            model(&units),
            options,
        );
        assert!(same(&shared, &alone));
        let (shared, alone) = (shared.unwrap(), alone.unwrap());
        let mesh = session.tessellate(&shared, chord(), TessellationOptions::default());
        let free = alone.tessellate(chord(), TessellationOptions::default());
        assert!(same(&mesh, &free));
        mesh.unwrap().require_solid().unwrap();
    }
    let s = session.statistics();
    assert_eq!(
        (s.shared_decodes, s.shared_roots, s.root_decodes),
        (1, 4, 0)
    );
    assert_eq!(s.shared_fallbacks, 0);
    // The shared DIRECTION records were decoded once and adapted by every root.
    assert_eq!(s.decoded_entities, 4 * closure - 3 * SHARED);
    assert_eq!(s.adapted_entities, 4 * closure);
    assert_eq!((s.imports, s.discoveries), (4, 1));
    assert!(s.triangles > 0);
    // The shared decode was released with its last root: a second import decodes alone.
    let first = &session.solids().unwrap()[0];
    let units = first.units.clone().unwrap();
    let again =
        session.import_brep_solid(first.entity, units.length, units.plane_angle, model(&units));
    let alone = import_brep_solid(
        &doc,
        first.entity,
        units.length,
        units.plane_angle,
        model(&units),
        options,
    );
    assert!(same(&again, &alone));
    assert_eq!(session.statistics().root_decodes, 1);
}

#[test]
fn a_root_outside_the_profile_makes_its_group_decode_alone_with_identical_errors() {
    let doc = parse(&washers(3, Some(1)));
    let options = ImportOptions::default();
    let session = ImportSession::new(&doc, options);
    let roots: Vec<EntityId> = session.solids().unwrap().iter().map(|r| r.entity).collect();
    assert_eq!(session.prepare(ImportProfile::Brep, &roots), 0);
    let mut failures = 0;
    for root in session.solids().unwrap() {
        let units = root.units.clone().unwrap();
        let shared =
            session.import_brep_solid(root.entity, units.length, units.plane_angle, model(&units));
        let alone = import_brep_solid(
            &doc,
            root.entity,
            units.length,
            units.plane_angle,
            model(&units),
            options,
        );
        assert!(same(&shared, &alone));
        if let Err(e) = shared {
            assert_eq!((e.stage, e.kind), (Stage::Profile, ErrorKind::Unsupported));
            assert!(
                e.message
                    .contains("CONNECTED_FACE_SET is outside the tessstep_brep")
            );
            failures += 1;
        }
    }
    assert_eq!(failures, 1);
    let s = session.statistics();
    assert_eq!(
        (s.shared_decodes, s.shared_fallbacks, s.root_decodes),
        (0, 1, 3)
    );
}

#[test]
fn discovery_and_selection_run_once_per_session() {
    let doc = parse(SELECTION);
    let options = ImportOptions::default();
    let session = ImportSession::new(&doc, options);
    for _ in 0..2 {
        assert_eq!(
            session.solids().unwrap(),
            discover_solids(&doc, options).unwrap()
        );
        assert_eq!(
            session.tessellations().unwrap(),
            discover_tessellations(&doc, options).unwrap()
        );
        assert_eq!(
            session.presentations().unwrap(),
            discover_presentations(&doc, options).unwrap()
        );
        let solids = discover_solids(&doc, options).unwrap();
        let tessellations = discover_tessellations(&doc, options).unwrap();
        for preference in [
            RepresentationPreference::Exact,
            RepresentationPreference::Tessellated,
        ] {
            assert_eq!(
                session.representation_choices(preference).unwrap(),
                select_representations(&doc, &solids, &tessellations, preference, options).unwrap()
            );
        }
    }
    assert_eq!(session.statistics().discoveries, 5);
}

#[test]
fn tessellations_and_presentations_share_the_tessellated_profile_decode() {
    let options = ImportOptions::default();
    let doc = parse(SELECTION);
    let session = ImportSession::new(&doc, options);
    let roots: Vec<EntityId> = session
        .tessellations()
        .unwrap()
        .iter()
        .map(|r| r.entity)
        .collect();
    assert!(roots.len() >= 2);
    assert_eq!(
        session.prepare(ImportProfile::Tessellated, &roots),
        roots.len()
    );
    for &root in &roots {
        let shared = session.import_tessellated(
            root,
            tessstep_math::LengthUnit::MILLIMETRE,
            Default::default(),
        );
        let alone = import_tessellated(
            &doc,
            root,
            tessstep_math::LengthUnit::MILLIMETRE,
            options,
            Default::default(),
        );
        assert!(same(&shared, &alone));
    }
    assert_eq!(session.statistics().shared_decodes, 1);

    let doc = parse(PRESENTATION);
    let session = ImportSession::new(&doc, options);
    let roots: Vec<EntityId> = session
        .presentations()
        .unwrap()
        .iter()
        .map(|r| r.entity)
        .collect();
    assert_eq!(session.prepare(ImportProfile::Tessellated, &roots), 2);
    for &root in &roots {
        let shared = session.import_presentation(
            root,
            tessstep_math::LengthUnit::MILLIMETRE,
            PresentationLimits::default(),
        );
        let alone = import_presentation(
            &doc,
            root,
            tessstep_math::LengthUnit::MILLIMETRE,
            options,
            PresentationLimits::default(),
        );
        assert_eq!(shared, alone);
    }
    let s = session.statistics();
    assert_eq!((s.shared_decodes, s.root_decodes), (1, 0));
}

#[test]
fn assembly_import_decodes_the_selected_roots_together() {
    let doc = parse(&washers(3, None));
    let options = AssemblyImportOptions::default();
    let session = ImportSession::new(&doc, options.assembly.import);
    let imported = session.import_assembly(options).unwrap();
    let s = session.statistics();
    assert_eq!(
        (s.shared_decodes, s.shared_roots, s.root_decodes),
        (1, 3, 0)
    );
    // Solids, tessellations and the exact selection.
    assert_eq!(s.discoveries, 3);
    assert_eq!(imported.roots.len(), 3);
    // The free function uses a one-shot session with the same outcome.
    let free = import_assembly(&doc, options).unwrap();
    assert!(same(&imported.roots, &free.roots));
    assert_eq!(imported.assembly.unplaced(), free.assembly.unplaced());
    let meshes = |a: &LinkedAssembly| {
        a.scene()
            .assets()
            .iter()
            .map(|asset| format!("{:?}", asset.mesh.data()))
            .collect::<Vec<_>>()
    };
    assert_eq!(meshes(&imported.assembly), meshes(&free.assembly));
    // Each mesh is the free single-root import's.
    for (asset, root) in imported
        .assembly
        .scene()
        .assets()
        .iter()
        .zip(session.solids().unwrap())
    {
        let units = root.units.clone().unwrap();
        let solid = import_brep_solid(
            &doc,
            root.entity,
            units.length,
            units.plane_angle,
            model(&units),
            options.assembly.import,
        )
        .unwrap();
        let mesh = solid
            .tessellate(options.tolerance, options.tessellation)
            .unwrap();
        assert_eq!(
            format!("{:?}", asset.mesh.data()),
            format!("{:?}", mesh.data())
        );
    }
}

#[test]
fn session_budgets_are_named_in_diagnostics() {
    let doc = parse(&washers(2, None));
    let options = ImportOptions::default();
    let budgets = SessionBudgets::for_document(&doc);
    assert_eq!(
        budgets,
        SessionBudgets::default(),
        "a small document keeps the defaults"
    );
    let roots: Vec<(EntityId, ContextUnits)> = discover_solids(&doc, options)
        .unwrap()
        .into_iter()
        .map(|r| (r.entity, r.units.unwrap()))
        .collect();
    let import = |session: &ImportSession<'_>, (root, units): &(EntityId, ContextUnits)| {
        session.import_brep_solid(*root, units.length, units.plane_angle, model(units))
    };
    // A stage is admitted while its running total is below the budget.
    for (budget, stage, message) in [
        (
            SessionBudgets {
                max_decoded_entities: 1,
                ..budgets
            },
            Stage::Profile,
            "session decode budget exceeded (max_decoded_entities 1)",
        ),
        (
            SessionBudgets {
                max_adapted_entities: 1,
                ..budgets
            },
            Stage::Geometry,
            "session adapter budget exceeded (max_adapted_entities 1)",
        ),
    ] {
        let session = ImportSession::with_budgets(&doc, options, budget);
        import(&session, &roots[0]).unwrap();
        let e = import(&session, &roots[1]).unwrap_err();
        assert_eq!(
            (e.kind, e.stage, e.entity),
            (ErrorKind::ResourceLimit, stage, Some(roots[1].0))
        );
        assert_eq!(e.message, message);
    }
    // A prepared group is not decoded once the decode budget is spent.
    let session = ImportSession::with_budgets(
        &doc,
        options,
        SessionBudgets {
            max_decoded_entities: 1,
            ..budgets
        },
    );
    import(&session, &roots[0]).unwrap();
    assert_eq!(
        session.prepare(ImportProfile::Brep, &[roots[0].0, roots[1].0]),
        0
    );

    let session = ImportSession::with_budgets(
        &doc,
        options,
        SessionBudgets {
            max_triangles: 1,
            ..budgets
        },
    );
    let first = import(&session, &roots[0]).unwrap();
    session
        .tessellate(&first, chord(), TessellationOptions::default())
        .unwrap();
    let second = import(&session, &roots[1]).unwrap();
    let e = session
        .tessellate(&second, chord(), TessellationOptions::default())
        .unwrap_err();
    assert_eq!(
        (e.kind, e.stage, e.entity),
        (
            ErrorKind::ResourceLimit,
            Stage::Tessellation,
            Some(roots[1].0)
        )
    );
    assert_eq!(
        e.message,
        "session triangle budget exceeded (max_triangles 1)"
    );
}

#[test]
fn session_budgets_scale_with_the_document() {
    let doc = parse(&washers(4, None));
    let entities = doc.entities().len();
    let budgets = SessionBudgets::for_document(&doc);
    assert!(budgets.max_decoded_entities >= entities * SessionBudgets::DECODED_PER_ENTITY);
    assert!(budgets.max_adapted_entities >= entities * SessionBudgets::ADAPTED_PER_ENTITY);
    assert!(budgets.max_triangles >= entities * SessionBudgets::TRIANGLES_PER_ENTITY);
    let large = ImportSession::new(&doc, ImportOptions::default());
    assert_eq!(large.budgets(), budgets);
}

#[test]
fn a_session_moves_between_threads_and_prints_a_summary() {
    fn send<T: Send>(_: &T) {}
    let doc = parse(&washers(2, None));
    let session = ImportSession::new(&doc, ImportOptions::default());
    send(&session);
    session.solids().unwrap();
    let summary = format!("{session:?}");
    assert!(summary.starts_with("ImportSession { entities: "));
    assert!(summary.contains("discoveries: 1"));
}

#[allow(dead_code)]
#[path = "../benches/session.rs"]
mod bench;

/// The nightly session benchmark's stage report, session and independent imports with
/// product linking and styles, on an authored styled assembly and on three washers.
#[test]
fn the_session_benchmark_reports_authored_documents() {
    let styled = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/geometry/assembly-styled.step"
    );
    let washers_path =
        std::env::temp_dir().join(format!("tessstep-session-{}.step", std::process::id()));
    std::fs::write(&washers_path, washers(3, None)).unwrap();
    bench::run(vec![
        styled.into(),
        washers_path.to_string_lossy().into_owned(),
    ]);
    std::fs::remove_file(&washers_path).unwrap();
}
