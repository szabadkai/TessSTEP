//! Replays every committed minimized fuzz finding (`fuzz/regressions/<target>/<file>`)
//! through the exercise function its libFuzzer target calls, on stable Rust without
//! instrumentation. See fuzz/README.md for turning a finding into a regression.
// Each module is compiled exactly as its fuzz target compiles it, so the shared
// fuzz/limits.rs budgets are loaded once per module that uses them.
#![allow(clippy::duplicate_mod)]
use std::{
    fs,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
};

#[path = "../../tessstep-curves/tests/support/exercise.rs"]
mod analytic_curves;
#[path = "../../../fuzz/support/brep.rs"]
mod brep_import;
#[path = "../../tessstep-tessellate/tests/support/pipeline_exercise.rs"]
mod brep_pipeline;
#[path = "../../../fuzz/harness.rs"]
mod entity_database;
#[path = "../../../fuzz/support/express.rs"]
mod express_frontend;
#[path = "../../tessstep-math/tests/support/exercise.rs"]
mod math_primitives;
#[path = "../../tessstep-curves/tests/support/spline_exercise.rs"]
mod nurbs_curves;
#[path = "../../../fuzz/support/lexer.rs"]
mod part21_lexer;
#[path = "../../../fuzz/support/parser.rs"]
mod part21_parser;
#[path = "../../../fuzz/support/presentation.rs"]
mod presentation_import;
#[path = "../../../fuzz/support/product.rs"]
mod product_adapter;
#[path = "../../../fuzz/support/codegen.rs"]
mod schema_codegen;
#[path = "../../../fuzz/support/decoder.rs"]
mod schema_decoder;
#[path = "../../tessstep-surfaces/tests/support/exercise.rs"]
mod surface_evaluators;
#[path = "../../../fuzz/support/tessellated.rs"]
mod tessellated_import;

type Exercise = fn(&[u8]);

/// Every `[[bin]]` in fuzz/Cargo.toml and the function its `fuzz_target!` calls.
const TARGETS: &[(&str, Exercise)] = &[
    ("analytic_curves", analytic_curves::exercise),
    ("brep_import", brep_import::exercise),
    ("brep_pipeline", brep_pipeline::exercise),
    ("entity_database", entity_database::exercise),
    ("express_frontend", express_frontend::exercise),
    ("math_primitives", math_primitives::exercise),
    ("nurbs_curves", nurbs_curves::exercise),
    ("part21_lexer", part21_lexer::exercise),
    ("part21_parser", part21_parser::exercise),
    ("presentation_import", presentation_import::exercise),
    ("product_adapter", product_adapter::exercise),
    ("schema_codegen", schema_codegen::exercise),
    ("schema_decoder", schema_decoder::exercise),
    ("surface_evaluators", surface_evaluators::exercise),
    ("tessellated_import", tessellated_import::exercise),
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cargo_fuzz_targets() -> Vec<String> {
    let manifest = fs::read_to_string(repository().join("fuzz/Cargo.toml")).unwrap();
    let mut names = Vec::new();
    let mut in_bin = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_bin = line == "[[bin]]";
            continue;
        }
        match line.split_once('=') {
            Some((key, value)) if in_bin && key.trim() == "name" => {
                names.push(value.trim().trim_matches('"').to_owned())
            }
            _ => {}
        }
    }
    names.sort();
    names
}

/// Committed findings in path order. A directory that names no target, or a nested
/// directory, is an error so that no committed artifact is silently skipped; dot
/// files (editor and OS metadata) are ignored.
fn findings(root: &Path, targets: &[(&str, Exercise)]) -> Result<Vec<(Exercise, PathBuf)>, String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(out);
    };
    let mut directories: Vec<_> = entries.map(|e| e.unwrap().path()).collect();
    directories.sort();
    for directory in directories {
        let name = directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Some(&(_, exercise)) = targets.iter().find(|(target, _)| *target == name) else {
            return Err(format!(
                "{} names no fuzz target in fuzz/Cargo.toml",
                directory.display()
            ));
        };
        let mut files: Vec<_> = fs::read_dir(&directory)
            .map_err(|e| format!("{}: {e}", directory.display()))?
            .map(|e| e.unwrap().path())
            .filter(|p| !p.file_name().unwrap().to_string_lossy().starts_with('.'))
            .collect();
        files.sort();
        for file in files {
            if !file.is_file() {
                return Err(format!("{} is not a finding file", file.display()));
            }
            out.push((exercise, file));
        }
    }
    Ok(out)
}

/// Replays every finding, continuing past failures, and names each one that panics.
fn replay(root: &Path, targets: &[(&str, Exercise)]) -> Result<usize, String> {
    let findings = findings(root, targets)?;
    let failed: Vec<_> = findings
        .iter()
        .filter(|(exercise, file)| {
            let data = fs::read(file).unwrap();
            panic::catch_unwind(AssertUnwindSafe(|| exercise(&data))).is_err()
        })
        .map(|(_, file)| file.display().to_string())
        .collect();
    if failed.is_empty() {
        Ok(findings.len())
    } else {
        Err(format!(
            "{} of {} committed fuzz findings fail: {}",
            failed.len(),
            findings.len(),
            failed.join(", ")
        ))
    }
}

#[test]
fn every_fuzz_target_has_a_stable_replay() {
    let mut names: Vec<_> = TARGETS.iter().map(|(name, _)| name.to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        cargo_fuzz_targets(),
        "add each fuzz target's exercise function to TARGETS in this file"
    );
    for (_, exercise) in TARGETS {
        exercise(&[]);
        exercise(&[0xff]);
    }
}

#[test]
fn committed_fuzz_findings_replay_without_failure() {
    let replayed = replay(&repository().join("fuzz/regressions"), TARGETS).unwrap();
    println!("fuzz regressions: replayed {replayed} committed findings");
}

#[test]
fn regression_replay_names_every_failing_finding_and_unknown_target() {
    fn quiet(_: &[u8]) {}
    fn fails_on_bad(data: &[u8]) {
        assert_ne!(data, b"bad", "replayed failure");
    }
    let targets: &[(&str, Exercise)] = &[("quiet", quiet), ("strict", fails_on_bad)];
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fuzz-regression-replay");
    let _ = fs::remove_dir_all(&root);
    assert_eq!(replay(&root, targets), Ok(0));
    for (path, data) in [
        ("quiet/bad", &b"bad"[..]),
        ("strict/good", b"good"),
        ("strict/bad", b"bad"),
        ("strict/.DS_Store", b"bad"),
    ] {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::write(root.join(path), data).unwrap();
    }
    let error = replay(&root, targets).unwrap_err();
    assert!(
        error.starts_with("1 of 3 committed fuzz findings fail"),
        "{error}"
    );
    assert!(error.ends_with("bad"), "{error}");
    fs::remove_file(root.join("strict/bad")).unwrap();
    assert_eq!(replay(&root, targets), Ok(2));
    fs::create_dir_all(root.join("removed_target")).unwrap();
    assert!(
        replay(&root, targets)
            .unwrap_err()
            .contains("names no fuzz target")
    );
    fs::remove_dir_all(&root).unwrap();
}
