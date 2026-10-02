import contextlib
import io
import json
import os
from pathlib import Path
import re
import struct
import sys
import tempfile
import unittest
from unittest import mock

import fuzz_report
import fuzz_seeds

ROOT = fuzz_seeds.ROOT


def quiet(function, *args, **kwargs):
    with contextlib.redirect_stdout(io.StringIO()):
        return function(*args, **kwargs)


def tree(root, files):
    for name, data in files.items():
        path = Path(root) / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


CARGO = b'[package]\nname = "fuzz"\n\n[[bin]]\nname = "alpha"\npath = "a.rs"\n\n[[bin]]\nname = "beta"\npath = "b.rs"\n'


class SeedTests(unittest.TestCase):
    def test_configuration_matches_cargo_targets_and_committed_corpora(self):
        self.assertEqual(fuzz_seeds.check(), sorted(fuzz_seeds.TARGETS))
        self.assertEqual(len(fuzz_seeds.TARGETS), 15)

    def test_unconfigured_targets_stale_patterns_and_missing_smoke_are_rejected(self):
        config = fuzz_seeds.text(("corpus/*.step",), 64)
        smoke = [{"name": "s", "targets": ["alpha", "beta"]}]
        with tempfile.TemporaryDirectory() as directory:
            tree(directory, {"fuzz/Cargo.toml": CARGO, "corpus/a.step": b"a"})
            root = Path(directory)
            self.assertEqual(fuzz_seeds.check(root, {"alpha": config, "beta": config}, smoke), ["alpha", "beta"])
            with self.assertRaisesRegex(ValueError, r"unconfigured \['beta'\]"):
                fuzz_seeds.check(root, {"alpha": config}, smoke)
            with self.assertRaisesRegex(ValueError, "matches no committed file"):
                fuzz_seeds.check(root, {"alpha": config, "beta": fuzz_seeds.text(("missing/*.step",), 64)}, smoke)
            with self.assertRaisesRegex(ValueError, r"no stable smoke test in SMOKE for \['beta'\]"):
                fuzz_seeds.check(root, {"alpha": config, "beta": config}, [{"name": "s", "targets": ["alpha"]}])
            replay_only = [{"name": "fuzz_regressions", "targets": ["alpha", "beta"]}]
            with self.assertRaisesRegex(ValueError, "no stable smoke test"):
                fuzz_seeds.check(root, {"alpha": config, "beta": config}, replay_only)

    def test_every_target_assembles_deterministically_within_its_length(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            for name, config in fuzz_seeds.TARGETS.items():
                a, b = fuzz_seeds.assemble(name, first), fuzz_seeds.assemble(name, second)
                self.assertEqual(a, b)
                self.assertTrue(a["seeds"], name)
                for seed in a["seeds"]:
                    data = (Path(first) / name / seed["name"]).read_bytes()
                    self.assertEqual(data, (Path(second) / name / seed["name"]).read_bytes())
                    self.assertLessEqual(len(data), config["max_len"], seed["name"])
                self.assertEqual(sorted(p.name for p in (Path(first) / name).iterdir()),
                                 sorted(s["name"] for s in a["seeds"]))
                self.assertEqual(json.loads((Path(first) / f"{name}.json").read_text(encoding="utf-8")), a)

    def test_duplicates_oversized_inputs_dot_files_and_regressions(self):
        config = {**fuzz_seeds.text(("corpus/*.step",), 8), "generated": lambda: {"made": b"g"}}
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as output:
            tree(directory, {"corpus/a.step": b"same", "corpus/b.step": b"same", "corpus/big.step": bytes(9),
                             "corpus/.hidden.step": b"h", "fuzz/regressions/alpha/crash-1": b"boom",
                             "fuzz/regressions/alpha/.DS_Store": b"x", "fuzz/regressions/beta/crash-2": b"other"})
            manifest = fuzz_seeds.assemble("alpha", output, Path(directory), config)
            self.assertEqual([s["name"] for s in manifest["seeds"]],
                             ["corpus__a.step", "generated__made.bin", "regression__crash-1"])
            self.assertEqual([s["source"] for s in manifest["seeds"]],
                             ["corpus/a.step", "generated:made", "fuzz/regressions/alpha/crash-1"])
            self.assertEqual(manifest["skipped"], [
                {"source": "corpus/b.step", "reason": "duplicate of corpus/a.step"},
                {"source": "corpus/big.step", "reason": "9 bytes exceeds -max_len=8"}])
            self.assertEqual((Path(output) / "alpha/regression__crash-1").read_bytes(), b"boom")
            # A rerun replaces the directory rather than accumulating stale seeds.
            (Path(output) / "alpha/stale").write_bytes(b"s")
            fuzz_seeds.assemble("alpha", output, Path(directory), config)
            self.assertFalse((Path(output) / "alpha/stale").exists())

    def test_generated_seeds_fit_their_harness_layouts(self):
        sizes = {"math_primitives": {144}, "analytic_curves": {128}, "nurbs_curves": {128},
                 "surface_evaluators": {128}}
        for name, expected in sizes.items():
            seeds = fuzz_seeds.TARGETS[name]["generated"]()
            self.assertEqual({len(v) for v in seeds.values()}, expected, name)
        identity = struct.unpack("<18d", fuzz_seeds.math_seeds()["identity"])
        self.assertEqual(identity[:9], (1, 0, 0, 0, 1, 0, 0, 0, 1))
        nurbs = fuzz_seeds.nurbs_seeds()
        self.assertEqual(struct.unpack("<16d", nurbs["quadratic"])[12:], (1, 0.7, 1, 0.3))
        self.assertTrue(nurbs["supplied-knots"][1] & 1 and not nurbs["supplied-knots"][0] & 1)
        self.assertTrue(nurbs["odd-degree"][0] & 1)
        pipeline = fuzz_seeds.pipeline_seeds()
        self.assertTrue(all(len(v) <= 96 and v[0] < 6 for v in pipeline.values()))
        self.assertEqual(sorted({v[2] % 8 for k, v in pipeline.items() if "mutation" in k}), list(range(8)))
        self.assertEqual(fuzz_seeds.boundary(16)["repeat-all-ones"], b"\xff" * 16)

    def test_plan_selects_targets_and_validates_the_budget(self):
        everything = fuzz_seeds.plan()
        self.assertEqual(json.loads(everything["targets"]), sorted(fuzz_seeds.TARGETS))
        self.assertEqual(everything["budget"], str(fuzz_seeds.DEFAULT_BUDGET))
        chosen = fuzz_seeds.plan(" part21_lexer,brep_import part21_lexer", "300")
        self.assertEqual(chosen, {"targets": '["brep_import", "part21_lexer"]', "budget": "300"})
        with self.assertRaisesRegex(ValueError, "unknown fuzz targets"):
            fuzz_seeds.plan("part21_lexer,nope")
        for budget in ("0", "901", "1.5", "abc", "-5"):
            with self.assertRaisesRegex(ValueError, "budget must be"):
                fuzz_seeds.plan("", budget)
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(fuzz_seeds.main(["--plan", "--only", "math_primitives"]), 0)
        self.assertEqual(output.getvalue().splitlines(), ['targets=["math_primitives"]', "budget=240"])
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(fuzz_seeds.main(["--plan", "--budget", "0"]), 1)

    def test_every_smoke_row_names_an_existing_test(self):
        for row in fuzz_seeds.SMOKE:
            path = ROOT / "crates" / row["package"] / "tests" / f"{row['test']}.rs"
            self.assertTrue(path.is_file(), path)
            function = row["filter"] or (row["name"] if row["name"] != row["test"] else None)
            if function:
                self.assertIn(f"fn {function}(", path.read_text(encoding="utf-8"), row["name"])
            self.assertLessEqual(set(row["targets"]), set(fuzz_seeds.TARGETS))


INSTRUMENTED = """INFO: Running with entropic power schedule (0xFF, 100).
INFO: Seed: 4242
INFO: Loaded 1 modules   (52345 inline 8-bit counters): 52345 [0x55d0c0a0, 0x55d0c0b0),
INFO: Loaded 1 PC tables (52345 PCs): 52345 [0x55d0c0b0,0x55d0c0c0),
INFO:        0 files found in /w/corpus/part21_parser
INFO:       98 files found in /w/seeds/part21_parser
INFO: seed corpus: files: 98 min: 56b max: 10654b total: 238727b rss: 45Mb
#99\tINITED cov: 2345 ft: 6789 corp: 80/200Kb exec/s: 0 rss: 60Mb
#1024\tpulse  cov: 2350 ft: 6800 corp: 81/201Kb lim: 10654 exec/s: 512 rss: 70Mb
#1100\tNEW    cov: 2351 ft: 6801 corp: 82/202Kb lim: 10654 exec/s: 550 rss: 70Mb L: 300/10654 MS: 2 ChangeByte-CopyPart-
#987654\tDONE   cov: 2400 ft: 7000 corp: 120/300Kb lim: 65536 exec/s: 4098 rss: 300Mb
Done 987654 runs in 241 second(s)
stat::number_of_executed_units: 987654
stat::average_exec_per_sec:     4098
stat::new_units_added:          40
stat::slowest_unit_time_sec:    0
stat::peak_rss_mb:              300
"""
# Condensed from a stable-built libFuzzer driver (libfuzzer-sys 0.4.13) and current Rust.
CRASH = """INFO: Seed: 1688036440
INFO: seed corpus: files: 2 min: 5b max: 55b total: 60b rss: 25Mb

thread '<unnamed>' (15434266) panicked at fuzz/support/brep.rs:46:13:
assertion `left == right` failed
  left: 1
stack backtrace:
thread '<unnamed>' (15434266) panicked at /rustc/abc/library/core/src/panicking.rs:225:5:
panic in a function that cannot unwind
thread caused non-unwinding panic. aborting.
==21310== ERROR: libFuzzer: deadly signal
SUMMARY: libFuzzer: deadly signal
MS: 0 ; base unit: 0000000000000000000000000000000000000000
artifact_prefix='/w/findings/'; Test unit written to /w/findings/crash-3313117234aa0ede5c013c5d4d6b25df5f8413e8
Base64: Qnh4
stat::number_of_executed_units: 3
stat::peak_rss_mb:              26
"""
TIMEOUT = """ALARM: working on the last Unit for 2 seconds
       and the timeout value is 2 (use -timeout=N to change)
artifact_prefix='/w/'; Test unit written to /w/timeout-76995f08e24bf651f936812c0af7e228fdd2dfd2
==21864== ERROR: libFuzzer: timeout after 2 seconds
SUMMARY: libFuzzer: timeout
"""
UNINSTRUMENTED = """WARNING: Failed to find function "__sanitizer_acquire_crash_state".
INFO: Seed: 1674962809
#99\tINITED exec/s: 0 rss: 26Mb
WARNING: no interesting inputs were found so far. Is the code instrumented for coverage?
#13636994\tDONE   corp: 1/1b lim: 65536 exec/s: 3409248 rss: 26Mb
Done 13636994 runs in 4 second(s)
"""


class LogTests(unittest.TestCase):
    def test_instrumented_log_yields_coverage_corpus_and_final_stats(self):
        log = fuzz_report.parse_log(INSTRUMENTED)
        self.assertEqual((log["seed"], log["coverage_counters"], log["seed_corpus_files"]), (4242, 52345, 98))
        self.assertEqual(log["initial"], {"executions": 99, "event": "INITED", "cov": 2345, "ft": 6789,
                                          "corpus_units": 80, "corpus_size": "200Kb", "exec_per_sec": 0, "rss_mb": 60})
        self.assertEqual((log["final"]["executions"], log["final"]["cov"], log["final"]["ft"], log["final"]["corpus_units"]),
                         (987654, 2400, 7000, 120))
        self.assertEqual(log["stats"]["new_units_added"], 40)
        self.assertEqual((log["done_seconds"], log["artifacts"], log["failure"]), (241, [], None))

    def test_the_first_panic_and_the_libfuzzer_error_name_the_failure(self):
        log = fuzz_report.parse_log(CRASH)
        self.assertEqual(log["failure"], "panicked at fuzz/support/brep.rs:46:13: assertion `left == right` failed; "
                                         "libFuzzer: deadly signal")
        self.assertEqual(log["artifacts"], ["/w/findings/crash-3313117234aa0ede5c013c5d4d6b25df5f8413e8"])
        self.assertIsNone(log["initial"])
        self.assertEqual(fuzz_report.parse_log(TIMEOUT)["failure"], "libFuzzer: timeout after 2 seconds")
        asan = "==7==ERROR: AddressSanitizer: heap-buffer-overflow on address 0x602000000011 at pc 0x55"
        self.assertEqual(fuzz_report.parse_log(asan)["failure"], asan.split("ERROR: ")[1])

    def test_an_uninstrumented_driver_shows_no_coverage(self):
        log = fuzz_report.parse_log(UNINSTRUMENTED)
        self.assertEqual(log["coverage_counters"], 0)
        self.assertNotIn("cov", log["final"])
        self.assertEqual(log["final"]["executions"], 13636994)


# A stand-in for a cargo-fuzz binary: libFuzzer's flags, output lines and artifact files.
FAKE = r'''# stand-in libFuzzer driver for test_fuzz.py; sanitizer marker: MARKER
import hashlib, os, sys
args = [a for a in sys.argv[1:] if not a.startswith("-")]
flags = dict(a[1:].split("=", 1) for a in sys.argv[1:] if a.startswith("-") and "=" in a)
env = os.environ.get
print("INFO: Seed: 7")
print("flags: " + " ".join(sorted(flags)))
if env("FAKE_COUNTERS"):
    print("INFO: Loaded 1 modules   (500 inline 8-bit counters): 500 [0x1, 0x2),")
if flags.get("minimize_crash") == "1":
    with open(flags["exact_artifact_path"], "wb") as stream:
        stream.write(b"" if env("FAKE_BAD_MINIMIZE") else b"BOOM")
    print("CRASH_MIN: failed to minimize beyond it, exiting")
    sys.exit(0)
if len(args) == 1 and os.path.isfile(args[0]):
    print("Running: " + args[0])
    if b"BOOM" in open(args[0], "rb").read():
        print("thread '<unnamed>' (1) panicked at fuzz/support/fake.rs:1:1:\nfake failure")
        print("==9== ERROR: libFuzzer: deadly signal")
        sys.exit(77)
    sys.exit(0)
seeds = sum(len(os.listdir(d)) for d in args)
cov = " cov: 10 ft: 20" if env("FAKE_COUNTERS") else ""
print(f"INFO: seed corpus: files: {seeds} min: 1b max: 9b total: 9b rss: 20Mb")
print(f"#{seeds}\tINITED{cov} corp: {seeds}/90b exec/s: 0 rss: 21Mb")
if env("FAKE_EXIT"):
    sys.exit(int(env("FAKE_EXIT")))
if env("FAKE_CRASH"):
    data = b"xxBOOMxx"
    path = flags["artifact_prefix"] + "crash-" + hashlib.sha1(data).hexdigest()
    open(path, "wb").write(data)
    print("thread '<unnamed>' (1) panicked at fuzz/support/fake.rs:1:1:\nfake failure")
    print("==9== ERROR: libFuzzer: deadly signal")
    print(f"artifact_prefix='{flags['artifact_prefix']}'; Test unit written to {path}")
    print("stat::number_of_executed_units: 5")
    sys.exit(77)
open(os.path.join(args[0], "new-unit"), "wb").write(b"n")
cov = " cov: 12 ft: 30" if env("FAKE_COUNTERS") else ""
print(f"#1000\tDONE  {cov} corp: {seeds + 1}/99b lim: 4096 exec/s: 500 rss: 30Mb")
print("Done 1000 runs in 2 second(s)")
for key, value in (("number_of_executed_units", 1000), ("average_exec_per_sec", 500), ("new_units_added", 1),
                   ("slowest_unit_time_sec", 0), ("peak_rss_mb", 30)):
    print(f"stat::{key}: {value}")
'''


class RunTests(unittest.TestCase):
    def run_fake(self, environment, sanitizer=True, require=True):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        fake = root / "fake_driver.py"
        fake.write_text(FAKE.replace("MARKER", "__asan_init" if sanitizer else "none"), encoding="utf-8")
        with mock.patch.dict(os.environ, environment), mock.patch.dict(os.environ, {"GITHUB_ACTIONS": "", "GITHUB_STEP_SUMMARY": ""}):
            code = quiet(fuzz_report.run_target, "math_primitives", fake, 5, root / "out", root / "work",
                         command=[sys.executable, str(fake)], require_instrumented=require, minimize_seconds=60)
        report = json.loads((root / "out/math_primitives/report.json").read_text(encoding="utf-8"))
        return code, report, root / "out/math_primitives"

    def test_a_clean_instrumented_run_records_coverage_corpus_and_seeds(self):
        code, report, out = self.run_fake({"FAKE_COUNTERS": "1"})
        self.assertEqual((code, report["status"], report["instrumented"], report["sanitizer"]), (0, "clean", True, "address"))
        self.assertEqual((report["initial"]["cov"], report["final"]["cov"], report["final"]["ft"]), (10, 12, 30))
        self.assertEqual((report["new_corpus_files"], report["coverage_counters"], report["stats"]["peak_rss_mb"]), (1, 500, 30))
        seeds = len(fuzz_seeds.math_seeds())
        self.assertEqual((report["seeds"]["files"], report["initial"]["executions"]), (seeds, seeds))
        self.assertEqual(report["options"]["max_len"], 144)
        self.assertIn("flags: artifact_prefix malloc_limit_mb max_len max_total_time print_final_stats rss_limit_mb timeout",
                      (out / "run.log").read_text(encoding="utf-8"))
        self.assertTrue((out / "seeds.json").is_file())

    def test_a_finding_is_minimized_replayed_and_annotated(self):
        code, report, out = self.run_fake({"FAKE_COUNTERS": "1", "FAKE_CRASH": "1"})
        self.assertEqual((code, report["status"]), (1, "finding"))
        [finding] = report["findings"]
        self.assertEqual((finding["kind"], finding["bytes"], finding["minimized_bytes"], finding["reproduces"]),
                         ("crash", 8, 4, True))
        self.assertEqual((out / "findings" / finding["minimized"]).read_bytes(), b"BOOM")
        self.assertEqual(finding["summary"], "panicked at fuzz/support/fake.rs:1:1: fake failure; libFuzzer: deadly signal")
        # Minimization runs without -max_len, which libFuzzer's -minimize_crash step rejects.
        self.assertNotIn("max_len", (out / f"minimize-{finding['artifact']}.log").read_text(encoding="utf-8"))
        [note] = fuzz_report.annotations(report)
        self.assertTrue(note.startswith("::error title=fuzz math_primitives%3A crash::panicked at"), note)
        self.assertNotIn("\n", note)
        self.assertIn("fuzz/regressions/math_primitives/", note)
        self.assertEqual(fuzz_report.escape("a:b,c%\nd", property=True), "a%3Ab%2Cc%25%0Ad")
        log = "\n".join(["noise"] * 200 + ["thread '<unnamed>' panicked at a.rs:1:1:", "boom"] + ["frame"] * 300)
        self.assertTrue(fuzz_report.excerpt(log).startswith("thread '<unnamed>' panicked at a.rs:1:1:\nboom"))
        self.assertEqual(len(fuzz_report.excerpt(log).splitlines()), 120)
        self.assertEqual(fuzz_report.excerpt("\n".join(map(str, range(100)))).splitlines()[0], "40")

    def test_a_minimized_input_that_does_not_fail_again_is_dropped(self):
        _, report, out = self.run_fake({"FAKE_COUNTERS": "1", "FAKE_CRASH": "1", "FAKE_BAD_MINIMIZE": "1"})
        [finding] = report["findings"]
        self.assertEqual((finding["minimized"], finding["reproduces"]), (None, True))
        self.assertEqual(sorted(p.name for p in (out / "findings").iterdir()), [finding["artifact"]])

    def test_uninstrumented_runs_and_failed_runs_without_artifacts_are_errors(self):
        code, report, _ = self.run_fake({}, sanitizer=False)
        self.assertEqual((code, report["status"], report["instrumented"]), (1, "error", False))
        self.assertIn("not an instrumented run: 0 coverage counters, sanitizer runtime missing", report["error"])
        code, report, _ = self.run_fake({"FAKE_COUNTERS": "1"}, sanitizer=False, require=False)
        self.assertEqual((code, report["status"], report["instrumented"]), (0, "clean", False))
        code, report, _ = self.run_fake({"FAKE_COUNTERS": "1", "FAKE_EXIT": "3"})
        self.assertEqual((code, report["status"]), (1, "error"))
        self.assertTrue(report["error"].startswith("libFuzzer exited with 3 and wrote no failure artifact"))


def run_report(target, **changes):
    report = {"format_version": 1, "target": target, "instrumented": True, "engine": "libFuzzer", "sanitizer": "address",
              "coverage_counters": 100, "build": None, "status": "clean", "error": None, "budget_seconds": 240,
              "duration_seconds": 240.5, "exit_code": 0, "libfuzzer_seed": 1, "options": {},
              "seeds": {"files": 3, "bytes": 30, "skipped": 0},
              "initial": {"executions": 3, "cov": 10, "ft": 20, "corpus_units": 3},
              "final": {"executions": 1000, "cov": 12, "ft": 30, "corpus_units": 4},
              "stats": {"average_exec_per_sec": 4, "peak_rss_mb": 30}, "new_corpus_files": 1,
              "findings": [], "slow_units": [], "failure": None, "command": []}
    report.update(changes)
    return report


class SummaryTests(unittest.TestCase):
    def summarize(self, reports, smoke=None, expected=("alpha", "beta")):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for report in reports:
                tree(root / "in", {f"{report['target']}/report.json": json.dumps(report).encode()})
            if smoke is not None:
                tree(root / "in", {"smoke/report.json": json.dumps({"instrumented": False, "tests": smoke}).encode()})
            with mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}):
                code = quiet(fuzz_report.summarize, root / "in", root / "out", list(expected), "success")
            summary = json.loads((root / "out/summary.json").read_text(encoding="utf-8"))
            return code, summary, (root / "out/summary.md").read_text(encoding="utf-8")

    SMOKE = [{"name": "fuzz_regressions", "package": "tessstep", "test": "fuzz_regressions", "targets": ["alpha", "beta"],
              "exercise": "shared", "instrumented": False, "status": "passed", "seconds": 1.0, "cases": None,
              "findings_replayed": 0}]

    def test_clean_instrumented_runs_and_passing_smoke_succeed(self):
        code, summary, markdown = self.summarize([run_report("alpha"), run_report("beta")], self.SMOKE)
        self.assertEqual(code, 0)
        self.assertEqual((summary["totals"]["instrumented"], summary["totals"]["executions"], summary["missing"]), (2, 2000, []))
        self.assertTrue(all(r["instrumented"] for r in summary["instrumented_runs"]))
        self.assertTrue(all(not t["instrumented"] for t in summary["driver_smoke"]))
        self.assertIn("**Instrumented (libFuzzer + AddressSanitizer, `instrumented: true`):** 2 of 2 targets", markdown)
        self.assertIn("**Driver smoke (stable `cargo test`, `instrumented: false`):** 1 of 1 tests passed.", markdown)
        self.assertIn("| `alpha` | yes (address) | clean | 240.5 | 1,000 | 4 | 10 -> 12 | 20 -> 30 | 4 | 3 | 30 | - |", markdown)
        self.assertNotIn("fuzz/regressions/<target>", markdown)

    def test_findings_errors_missing_targets_and_failed_smoke_fail(self):
        finding = {"kind": "crash", "artifact": "crash-ab", "bytes": 9, "sha256": "x", "minimized": "minimized-crash-ab",
                   "minimized_bytes": 2, "minimize_seconds": 1.0, "reproduces": True, "summary": "panicked at a.rs:1:1: x | y"}
        code, summary, markdown = self.summarize([run_report("alpha", status="finding", findings=[finding])], self.SMOKE)
        self.assertEqual((code, summary["missing"], summary["totals"]["findings"]), (1, ["beta"], 1))
        self.assertIn("No run report for `beta` (instrumented build: success", markdown)
        self.assertIn("minimized to 2 B, reproduces): panicked at a.rs:1:1: x &#124; y", markdown)
        self.assertIn("`alpha/findings/minimized-crash-ab`", markdown)
        self.assertIn("fuzz/regressions/<target>/<name>", markdown)
        uninstrumented = run_report("beta", instrumented=False, sanitizer=None)
        self.assertEqual(self.summarize([run_report("alpha"), uninstrumented], self.SMOKE)[0], 1)
        self.assertEqual(self.summarize([run_report("alpha"), run_report("beta")], None)[0], 1)
        failed = [{**self.SMOKE[0], "status": "failed"}]
        self.assertEqual(self.summarize([run_report("alpha"), run_report("beta")], failed)[0], 1)
        error = run_report("beta", status="error", error="libFuzzer did not stop within 540 s")
        code, _, markdown = self.summarize([run_report("alpha"), error], self.SMOKE)
        self.assertEqual(code, 1)
        self.assertIn("- **`beta`** error: libFuzzer did not stop within 540 s", markdown)

    def test_cli_accepts_the_build_job_matrix_as_expected_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree(root / "in", {"alpha/report.json": json.dumps(run_report("alpha")).encode(),
                               "smoke/report.json": json.dumps({"tests": self.SMOKE}).encode()})
            with mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}):
                code = quiet(fuzz_report.main, ["summary", "--input", str(root / "in"), "--output", str(root / "out"),
                                                "--expected", '["alpha"]'])
            self.assertEqual(code, 0)
            summary = json.loads((root / "out/summary.json").read_text(encoding="utf-8"))
            self.assertEqual(summary["totals"]["targets"], 1)


class SmokeTests(unittest.TestCase):
    def test_smoke_commands_counts_and_report(self):
        row = {"name": "codegen_fuzz_smoke", "package": "tessstep-codegen", "test": "generated",
               "filter": "codegen_fuzz_smoke", "targets": ["schema_codegen"], "exercise": "shared"}
        self.assertEqual(fuzz_report.smoke_command(row), ["cargo", "test", "--locked", "-p", "tessstep-codegen", "--test",
                                                          "generated", "--", "--nocapture", "--exact", "codegen_fuzz_smoke"])
        output = ("running 1 test\ncodegen fuzz smoke: 9,395 bounded cases\ntest codegen_fuzz_smoke ... ok\n"
                  "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out\n")
        replay = "fuzz regressions: replayed 4 committed findings\ntest result: FAILED. 2 passed; 1 failed; 0 ignored\n"
        results = iter([(0, output), (101, replay)])

        def runner(command, **_):
            code, text = next(results)
            return mock.Mock(returncode=code, stdout=text, stderr="")

        replay_row = {**row, "name": "fuzz_regressions", "package": "tessstep", "test": "fuzz_regressions", "filter": None}
        with tempfile.TemporaryDirectory() as directory:
            code = quiet(fuzz_report.run_smoke, directory, [row, replay_row], runner)
            report = json.loads((Path(directory) / "smoke/report.json").read_text(encoding="utf-8"))
        self.assertEqual(code, 1)
        self.assertFalse(report["instrumented"])
        first, second = report["tests"]
        self.assertEqual((first["status"], first["cases"], first["passed"], first["instrumented"]), ("passed", 9395, 1, False))
        self.assertEqual((second["status"], second["findings_replayed"], second["failed"]), ("failed", 4, 1))


class WorkflowTests(unittest.TestCase):
    TEXT = (ROOT / ".github/workflows/nightly.yml").read_text(encoding="utf-8")

    def test_the_matrix_is_derived_from_cargo_toml_not_listed(self):
        self.assertIn("python scripts/fuzz_seeds.py --plan", self.TEXT)
        self.assertIn("target: ${{ fromJSON(needs.fuzz-build.outputs.targets) }}", self.TEXT)
        for target in fuzz_seeds.TARGETS:
            self.assertNotIn(target, self.TEXT)

    def test_instrumented_build_runs_report_and_manual_dispatch(self):
        for text in ("cargo fuzz build --sanitizer address --debug-assertions", "--require-instrumented",
                     "python scripts/fuzz_report.py smoke", "python scripts/fuzz_report.py summary", "workflow_dispatch:",
                     "fail-fast: false", "pattern: fuzz-report-*", "cargo metadata --manifest-path fuzz/Cargo.toml --locked"):
            self.assertIn(text, self.TEXT)

    def test_actions_are_pinned_by_commit_and_benchmarks_unchanged(self):
        uses = re.findall(r"uses: (\S+)", self.TEXT)
        self.assertTrue(uses)
        for action in uses:
            self.assertRegex(action, r"@[0-9a-f]{40}$")
        self.assertIn("run: cargo bench --workspace --locked 2>&1 | tee benchmarks.txt", self.TEXT)

    def test_the_workflow_structure_when_pyyaml_is_available(self):
        try:
            import yaml
        except ImportError:
            self.skipTest("PyYAML is not installed")
        jobs = yaml.safe_load(self.TEXT)["jobs"]
        self.assertEqual(jobs["fuzz"]["needs"], "fuzz-build")
        self.assertEqual((jobs["fuzz-report"]["needs"], jobs["fuzz-report"]["if"]), (["fuzz-build", "fuzz", "fuzz-smoke"], "always()"))


if __name__ == "__main__":
    unittest.main()
