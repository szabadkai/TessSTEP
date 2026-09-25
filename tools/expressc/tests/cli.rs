use std::process::Command;
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_expressc"))
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn expressc_exit_status_and_determinism() {
    assert!(run(&["--help"]).status.success());
    assert_eq!(run(&[]).status.code(), Some(2));
    assert_eq!(run(&["--missing"]).status.code(), Some(2));
    assert_eq!(run(&["--max-work", "no"]).status.code(), Some(2));
    assert_eq!(
        run(&["--max-bytes", "1", "corpus/express/valid/base.exp"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run(&["--max-work", "0", "corpus/express/valid/base.exp"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(run(&["does-not-exist.exp"]).status.code(), Some(2));
    let args = [
        "--json",
        "corpus/express/valid/base.exp",
        "corpus/express/valid/imports.exp",
    ];
    let a = run(&args);
    let b = run(&args);
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert_eq!(a.stdout, b.stdout);
    assert!(a.stderr.is_empty());
    assert!(
        String::from_utf8(a.stdout)
            .unwrap()
            .contains("\"structural_valid\":true")
    );
    assert_eq!(
        run(&["--strict", "corpus/express/valid/base.exp"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        run(&["corpus/express/valid/imports.exp"]).status.code(),
        Some(1)
    );
    assert_eq!(
        run(&["corpus/express/invalid/semantic.exp"]).status.code(),
        Some(1)
    );
    assert_eq!(
        run(&["corpus/express/invalid/truncated.exp"]).status.code(),
        Some(1)
    );
    let ast = run(&["--ast", "corpus/express/valid/opaque.exp"]);
    assert!(ast.status.success());
    assert!(
        String::from_utf8(ast.stdout)
            .unwrap()
            .contains("Unsupported")
    );
}

#[test]
fn expressc_rust_output_and_failure_policy() {
    let args = [
        "--rust",
        "corpus/express/valid/base.exp",
        "corpus/express/valid/imports.exp",
    ];
    let a = run(&args);
    assert!(a.status.success());
    assert_eq!(a.stdout, run(&args).stdout);
    assert!(String::from_utf8_lossy(&a.stdout).contains("pub static SCHEMA_SET"));
    assert!(!a.stderr.is_empty()); // opaque semantics remain explicit
    for args in [
        vec!["--rust", "--strict", "corpus/express/valid/base.exp"],
        vec![
            "--rust",
            "--max-output-bytes",
            "1",
            "corpus/express/valid/base.exp",
        ],
        vec!["--rust", "corpus/express/invalid/semantic.exp"],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
    assert_eq!(
        run(&["--rust", "--json", "corpus/express/valid/base.exp"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn expressc_validator_generation_policy() {
    let a = run(&["--validator", "corpus/express/valid/base.exp"]);
    assert!(a.status.success());
    assert_eq!(
        a.stdout,
        run(&["--validator", "corpus/express/valid/base.exp"]).stdout
    );
    assert!(String::from_utf8_lossy(&a.stdout).contains("schema-structure"));
    for args in [
        vec!["--validator", "--strict", "corpus/express/valid/base.exp"],
        vec![
            "--validator",
            "--max-output-bytes",
            "1",
            "corpus/express/valid/base.exp",
        ],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
    }
    assert_eq!(
        run(&["--validator", "--rust", "corpus/express/valid/base.exp"])
            .status
            .code(),
        Some(2)
    );
}
