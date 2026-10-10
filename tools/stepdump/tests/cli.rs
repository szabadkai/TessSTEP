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
#[test]
fn slips_are_warnings_unless_strict_and_budgets_follow_the_file() {
    let slips = fixture("tolerated/exporter-slips.step");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_stepdump"))
            .args(args)
            .output()
            .unwrap()
    };
    let tolerant = run(&["--json", &slips]);
    assert_eq!(tolerant.status.code(), Some(0));
    let text = String::from_utf8(tolerant.stdout).unwrap();
    for expected in [
        "\"deviations\":{\"literal_backslashes\":4,\"null_header_strings\":1}",
        "\"code\":\"TS1006\",\"severity\":\"warning\"",
        "\"code\":\"TS1011\",\"severity\":\"warning\"",
        "D:\\\\exports\\\\bracket.stp",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    let strict = run(&["--json", "--strict", &slips]);
    assert_eq!(strict.status.code(), Some(1));
    let text = String::from_utf8(strict.stdout).unwrap();
    assert!(
        text.contains("\"document\":null") && text.contains("TS1006"),
        "{text}"
    );
    // Budgets are sized from the file; an explicit budget still applies after sizing.
    let valid = fixture("valid/complex.step");
    for args in [
        vec!["--max-total-values=1", valid.as_str()],
        vec!["--default-budgets", "--max-total-values=1", valid.as_str()],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains("total value budget exceeded (limit 1)"),
            "{text}"
        );
    }
    assert_eq!(run(&["--default-budgets", &valid]).status.code(), Some(0));
    let help = String::from_utf8(run(&["--help"]).stdout).unwrap();
    assert!(help.contains("--strict") && help.contains("ParseLimits::for_input"));
}
