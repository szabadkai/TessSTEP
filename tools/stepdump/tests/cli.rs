use std::{path::Path, process::Command};
fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/part21")
        .join(name)
        .to_string_lossy()
        .into_owned()
}
#[test]
fn reports_headers_counts_and_is_deterministic() {
    let args = ["--json", &fixture("valid/complex.step")];
    let first = Command::new(env!("CARGO_BIN_EXE_stepdump"))
        .args(args)
        .output()
        .unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_stepdump"))
        .args(args)
        .output()
        .unwrap();
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let text = String::from_utf8(first.stdout).unwrap();
    for expected in [
        "FILE_DESCRIPTION",
        "FILE_NAME",
        "FILE_SCHEMA",
        "\"entity_count\":2",
        "\"A\":2",
        "\"diagnostics\":[]",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}
#[test]
fn diagnostics_and_exit_statuses_are_useful() {
    for (file, expected) in [
        ("valid/missing.step", "TS1103"),
        ("invalid/duplicate.step", "TS1101"),
        ("invalid/truncated.step", "TS1002"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_stepdump"))
            .args(["--json", &fixture(file)])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8(output.stdout).unwrap().contains(expected));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_stepdump"))
        .arg("--unknown")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
#[test]
fn stdin_and_help_work() {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_stepdump"))
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(include_bytes!("../../../corpus/part21/valid/empty.step"))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Entities: 0")
    );
    assert!(
        Command::new(env!("CARGO_BIN_EXE_stepdump"))
            .arg("--help")
            .output()
            .unwrap()
            .status
            .success()
    );
}
