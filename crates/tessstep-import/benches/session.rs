//! Document import sessions against independent per-root imports.
//!
//! A generated document of K renumbered washers (one shared context, one shape
//! representation each) shows how importing every root scales with K: a session
//! discovers once and decodes the roots together, while an independent import runs
//! discovery (a document scan) for each root to find its units. The largest external
//! corpus inputs, resolved from `TESSSTEP_CORPUS` (absolute, or relative to the
//! workspace root) or `~/step-corpus`, then record time and peak memory per stage:
//! parse, discovery, decode, adapt, tessellate and assembly (product linking and
//! styles). Inputs that are absent are skipped with a notice. Peak memory is the
//! process's peak resident set size during the stage, read from Linux `/proc` after
//! resetting it; elsewhere it is reported as unavailable.
//!
//! `cargo run --release -p tessstep-import --example session -- FILE...` runs the
//! same stage report on chosen files.
use std::{
    fmt::Write,
    fs::File,
    hint::black_box,
    io::BufReader,
    path::{Path, PathBuf},
    time::Instant,
};
use tessstep_import::*;
use tessstep_math::{Angle, Length, ModelTolerance};
use tessstep_mesh::Mesh;
use tessstep_model::Document;
use tessstep_part21::{EntityId, ParseLimits};

const WASHER: &str = include_str!("../../../corpus/geometry/brep-washer.step");
/// The slowest inputs to import: 24 B-rep roots in 77,956 entities, and 8 roots of
/// which three exhaust their tessellation budgets.
const CORPUS_INPUTS: [&str; 2] = [
    "vendor/opencascade-dataset/step/bug33261.stp",
    "vendor/opencascade-dataset/step/trj3_s1-id-203.stp",
];
const CONTEXT: &str = "#9000001=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));
#9000002=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));
#9000003=(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT());
#9000004=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-4),#9000001,'DISTANCE_ACCURACY_VALUE','');
#9000005=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#9000004))GLOBAL_UNIT_ASSIGNED_CONTEXT((#9000001,#9000002,#9000003))REPRESENTATION_CONTEXT('',''));
";

fn main() {
    // `cargo bench` passes `--bench`; other arguments are files to report.
    run(std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect());
}

/// The scaling series and the corpus inputs, or the stage report of `files`.
pub fn run(files: Vec<String>) {
    if !files.is_empty() {
        for file in files {
            report(Path::new(&file));
        }
        return;
    }
    scaling();
    match corpus() {
        Ok(root) => {
            for input in CORPUS_INPUTS {
                let path = root.join(input);
                if path.is_file() {
                    report(&path);
                } else {
                    println!("corpus input skipped: {} not found", path.display());
                }
            }
        }
        Err(reason) => println!("corpus inputs skipped: {reason}"),
    }
}

fn corpus() -> Result<PathBuf, String> {
    let root = match std::env::var_os("TESSSTEP_CORPUS") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_relative() {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(path)
            } else {
                path
            }
        }
        None => std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| PathBuf::from(home).join("step-corpus"))
            .ok_or("neither TESSSTEP_CORPUS nor a home directory is set")?,
    };
    if root.is_dir() {
        Ok(root)
    } else {
        Err(format!(
            "{} is not a directory (set TESSSTEP_CORPUS)",
            root.display()
        ))
    }
}

/// `#id` references moved by `offset`.
fn renumber(data: &str, offset: u64) -> String {
    let mut out = String::with_capacity(data.len() + data.len() / 8);
    let mut rest = data;
    while let Some(i) = rest.find('#') {
        out.push_str(&rest[..=i]);
        rest = &rest[i + 1..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let id: u64 = rest[..end].parse().expect("entity id");
        let _ = write!(out, "{}", id + offset);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `copies` washers, each in its own shape representation of one shared context.
fn washers(copies: u64) -> String {
    let (header, rest) = WASHER.split_once("DATA;\n").expect("DATA section");
    let body = rest.split_once("ENDSEC;").expect("ENDSEC").0;
    let mut data = String::from(CONTEXT);
    for k in 0..copies {
        data.push_str(&renumber(body, k * 10_000));
        let _ = writeln!(
            data,
            "#{}=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{}),#9000005);",
            9_100_000 + k,
            1000 + k * 10_000
        );
    }
    format!("{header}DATA;\n{data}ENDSEC;\nEND-ISO-10303-21;\n")
}

/// Mean seconds per call of `f` over at least one second.
fn time(f: &mut dyn FnMut()) -> f64 {
    let start = Instant::now();
    let mut n = 0;
    while n == 0 || start.elapsed().as_secs_f64() < 1. {
        f();
        n += 1;
    }
    start.elapsed().as_secs_f64() / n as f64
}

fn model_tolerance(units: &ContextUnits) -> ModelTolerance {
    let distance = units.distance_uncertainty.unwrap_or(1e-7).max(1e-7);
    ModelTolerance::new(
        Length::metres(distance).expect("finite tolerance"),
        Angle::radians(1e-8).expect("finite angle"),
    )
    .expect("valid tolerance")
}

/// Per-root decode and adapt cost as the document grows, without tessellation.
fn scaling() {
    let options = ImportOptions::default();
    for copies in [1u64, 8, 64, 256] {
        let doc =
            tessstep_model::parse(washers(copies).as_bytes(), ParseLimits::default()).unwrap();
        let session_s = time(&mut || {
            let session = ImportSession::new(&doc, options);
            let roots = session.solids().unwrap();
            let ids: Vec<EntityId> = roots.iter().map(|r| r.entity).collect();
            session.prepare(ImportProfile::Brep, &ids);
            for root in roots {
                let units = root.units.as_ref().unwrap();
                black_box(
                    session
                        .import_brep_solid(
                            root.entity,
                            units.length,
                            units.plane_angle,
                            model_tolerance(units),
                        )
                        .unwrap(),
                );
            }
        });
        let independent_s = time(&mut || {
            for k in 0..copies {
                let entity = EntityId::new(1000 + k * 10_000).unwrap();
                let roots = discover_solids(&doc, options).unwrap();
                let root = roots.iter().find(|r| r.entity == entity).unwrap();
                let units = root.units.as_ref().unwrap();
                black_box(
                    import_brep_solid(
                        &doc,
                        entity,
                        units.length,
                        units.plane_angle,
                        model_tolerance(units),
                        options,
                    )
                    .unwrap(),
                );
            }
        });
        println!(
            "{copies:>4} washers ({} entities): session {:.1} us per root, independent {:.1} us per root ({:.1}x)",
            doc.entities().len(),
            session_s / copies as f64 * 1e6,
            independent_s / copies as f64 * 1e6,
            independent_s / session_s
        );
    }
}

/// Reset the peak resident set size; false when it cannot be reset.
fn reset_peak() -> bool {
    std::fs::write("/proc/self/clear_refs", "5").is_ok()
}

/// Peak resident set size since the last reset, in MiB.
fn peak_mib() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kib: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib / 1024.)
}

/// Seconds and peak memory of each stage, summed and maximized over repeated runs,
/// and the stages that failed.
#[derive(Default)]
struct Stages {
    stages: Vec<(&'static str, f64, Option<f64>)>,
    failures: Vec<String>,
}
impl Stages {
    fn run<T>(&mut self, name: &'static str, f: impl FnOnce() -> T) -> T {
        let per_stage = reset_peak();
        let start = Instant::now();
        let value = f();
        let seconds = start.elapsed().as_secs_f64();
        let peak = peak_mib().filter(|_| per_stage);
        match self.stages.iter_mut().find(|(n, ..)| *n == name) {
            Some(stage) => {
                stage.1 += seconds;
                stage.2 = match (stage.2, peak) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
            }
            None => self.stages.push((name, seconds, peak)),
        }
        value
    }
    /// `run` for a stage whose failure is reported, not fatal.
    fn attempt(&mut self, name: &'static str, f: impl FnOnce() -> Result<(), Error>) {
        if let Err(e) = self.run(name, f) {
            self.failures.push(format!("{name} failed: {e}"));
        }
    }
    fn print(&self, path: &str) {
        let mut line = format!("  {path:<11}");
        let mut total = 0.;
        for (name, seconds, peak) in &self.stages {
            total += seconds;
            let peak = peak.map_or("n/a".to_string(), |p| format!("{p:.1}"));
            let _ = write!(line, " | {name} {seconds:.3} s {peak} MiB");
        }
        println!("{line} | total {total:.3} s");
        for failure in &self.failures {
            println!("  {path:<11} {failure}");
        }
    }
}

/// A selected shape root with its context units, and its import profile.
struct Root {
    entity: EntityId,
    units: ContextUnits,
    solid: Option<SolidKind>,
}

/// Roots `import_assembly` imports first: each shape's selected representation, when
/// its context declares units.
fn selected(
    solids: &[SolidRoot],
    tessellations: &[TessellatedRoot],
    choices: &[RepresentationChoice],
) -> Vec<Root> {
    choices
        .iter()
        .filter_map(|choice| match choice.selected {
            RootRef::Solid(i) => solids[i].units.clone().ok().map(|units| Root {
                entity: solids[i].entity,
                units,
                solid: Some(solids[i].kind),
            }),
            RootRef::Tessellation(i) => tessellations[i].units.clone().ok().map(|units| Root {
                entity: tessellations[i].entity,
                units,
                solid: None,
            }),
        })
        .collect()
}

/// Stage times and peak memory of the session and of independent per-root imports of
/// every selected root, at the assembly import tolerances.
fn report(path: &Path) {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mut parse = Stages::default();
    let doc = parse.run("parse", || {
        File::open(path)
            .map_err(|e| e.to_string())
            .and_then(|file| {
                tessstep_model::parse(BufReader::new(file), ParseLimits::default())
                    .map_err(|e| e.to_string())
            })
    });
    let doc = match doc {
        Ok(doc) => doc,
        Err(e) => {
            println!("{name}: not parsed: {e}");
            return;
        }
    };
    let mut options = AssemblyImportOptions::default();
    options.assembly.import = ImportOptions::for_document(&doc);
    let (session, statistics) = session_import(&doc, options);
    let independent = independent_import(&doc, options);
    match (session, independent) {
        (Ok((session, roots, accepted)), Ok(independent)) => {
            println!(
                "{name}: {} entities, {roots} selected roots ({accepted} meshed)",
                doc.entities().len()
            );
            parse.print("parse");
            session.print("session");
            independent.print("independent");
            println!("  {statistics:?}");
        }
        (Err(e), _) | (_, Err(e)) => println!("{name}: not imported: {e}"),
    }
}

fn session_import(
    doc: &Document,
    options: AssemblyImportOptions,
) -> (Result<(Stages, usize, usize), Error>, SessionStatistics) {
    let session = ImportSession::new(doc, options.assembly.import);
    let mut stages = Stages::default();
    let result = session_stages(&session, options, &mut stages);
    (
        result.map(|(roots, accepted)| (stages, roots, accepted)),
        session.statistics(),
    )
}

/// The session stages; returns the selected and meshed root counts.
fn session_stages(
    session: &ImportSession<'_>,
    options: AssemblyImportOptions,
    stages: &mut Stages,
) -> Result<(usize, usize), Error> {
    let roots = stages.run("discover", || -> Result<Vec<Root>, Error> {
        let choices = session.representation_choices(options.preference)?;
        Ok(selected(
            session.solids()?,
            session.tessellations()?,
            choices,
        ))
    })?;
    stages.run("decode", || {
        for (profile, solid) in [
            (ImportProfile::Faceted, Some(true)),
            (ImportProfile::Brep, Some(false)),
            (ImportProfile::Tessellated, None),
        ] {
            let ids: Vec<EntityId> = roots
                .iter()
                .filter(|r| r.solid.map(|k| k == SolidKind::FacetedBrep) == solid)
                .map(|r| r.entity)
                .collect();
            session.prepare(profile, &ids);
        }
    });
    let mut shapes = Vec::new();
    let imported = stages.run("adapt", || {
        let mut solids = Vec::new();
        for root in &roots {
            let tolerance = model_tolerance(&root.units);
            match root.solid {
                Some(SolidKind::FacetedBrep) => solids.push((
                    root.entity,
                    session.import_faceted_solid(root.entity, root.units.length, tolerance),
                )),
                Some(_) => solids.push((
                    root.entity,
                    session.import_brep_solid(
                        root.entity,
                        root.units.length,
                        root.units.plane_angle,
                        tolerance,
                    ),
                )),
                None => {
                    if let Ok(t) =
                        session.import_tessellated(root.entity, root.units.length, options.mesh)
                    {
                        shapes.push(asset(root.entity, t.into_mesh()));
                    }
                }
            }
        }
        solids
    });
    stages.run("tessellate", || {
        for (entity, solid) in &imported {
            let Ok(solid) = solid else { continue };
            if let Ok(mesh) = session.tessellate(solid, options.tolerance, options.tessellation) {
                shapes.push(asset(*entity, mesh));
            }
        }
    });
    stages.attempt("assemble", || {
        let linked = session.link_assembly(&shapes, options.assembly)?;
        black_box(session.import_appearance(&linked, StyleOptions::default())?);
        Ok(())
    });
    Ok((roots.len(), shapes.len()))
}

fn independent_import(doc: &Document, options: AssemblyImportOptions) -> Result<Stages, Error> {
    let import = options.assembly.import;
    let mut stages = Stages::default();
    // Which roots to import; each root then rediscovers its units on its own.
    let solids = discover_solids(doc, import)?;
    let tessellations = discover_tessellations(doc, import)?;
    let choices = select_representations(doc, &solids, &tessellations, options.preference, import)?;
    let mut shapes = Vec::new();
    for root in selected(&solids, &tessellations, &choices) {
        let units = stages.run("discover", || -> Result<ContextUnits, Error> {
            let solids = discover_solids(doc, import)?;
            let tessellations = discover_tessellations(doc, import)?;
            let found = solids
                .iter()
                .find(|r| r.entity == root.entity)
                .map(|r| &r.units)
                .or_else(|| {
                    tessellations
                        .iter()
                        .find(|r| r.entity == root.entity)
                        .map(|r| &r.units)
                })
                .expect("selected roots are discovered");
            found.clone()
        })?;
        let solid = stages.run("import", || match root.solid {
            Some(SolidKind::FacetedBrep) => Some(import_faceted_solid(
                doc,
                root.entity,
                units.length,
                model_tolerance(&units),
                import,
            )),
            Some(_) => Some(import_brep_solid(
                doc,
                root.entity,
                units.length,
                units.plane_angle,
                model_tolerance(&units),
                import,
            )),
            None => {
                if let Ok(t) =
                    import_tessellated(doc, root.entity, units.length, import, options.mesh)
                {
                    shapes.push(asset(root.entity, t.into_mesh()));
                }
                None
            }
        });
        if let Some(Ok(solid)) = solid {
            stages.run("tessellate", || {
                if let Ok(mesh) = solid.tessellate(options.tolerance, options.tessellation) {
                    shapes.push(asset(root.entity, mesh));
                }
            });
        }
    }
    stages.attempt("assemble", || {
        let linked = link_assembly(doc, &shapes, options.assembly)?;
        black_box(import_appearance(doc, &linked, StyleOptions::default())?);
        Ok(())
    });
    Ok(stages)
}

fn asset(root: EntityId, mesh: Mesh) -> ShapeAsset {
    ShapeAsset {
        root,
        mesh: std::sync::Arc::new(mesh),
    }
}
