#!/usr/bin/env python3
"""Instrumented libFuzzer runs, stable driver smoke and one report that tells them apart.

run      one prebuilt cargo-fuzz binary for a fixed time budget with the target's limits
         and seeds from fuzz_seeds.py; every failing input is minimized with libFuzzer's
         -minimize_crash loop (what `cargo fuzz tmin` runs) and replayed once.
smoke    the stable deterministic fuzz smoke tests and the committed-finding replay.
summary  summary.json and Markdown over both; a run is `instrumented` only when its log
         shows coverage counters and its binary contains the AddressSanitizer runtime.
"""
import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

import fuzz_seeds

ROOT = fuzz_seeds.ROOT
FAILURES = ("crash", "leak", "timeout", "oom")
ASAN_MARKERS = (b"__asan_init", b"heap-buffer-overflow")
STATUS = re.compile(r"^#(\d+)\s+(INITED|NEW|REDUCE|pulse|DONE|RELOAD)\s+(.*)$")
COUNTERS = re.compile(r"^INFO: Loaded \d+ modules?\s+\((\d+) inline 8-bit counters\)")
PANIC = re.compile(r"^thread '[^']*'(?: \(\d+\))? panicked at (.+?):?$")
ERROR = re.compile(r"^==\d+==\s*ERROR: (.+)$")
FIELDS = (("cov", r"\bcov: (\d+)"), ("ft", r"\bft: (\d+)"), ("corpus_units", r"\bcorp: (\d+)/"),
          ("corpus_size", r"\bcorp: \d+/(\S+)"), ("exec_per_sec", r"\bexec/s: (\d+)"), ("rss_mb", r"\brss: (\d+)Mb"))


def failure(lines):
    """The first Rust panic (location and message) and the libFuzzer or sanitizer error."""
    parts = []
    for i, line in enumerate(lines):
        match = PANIC.match(line)
        if match:
            message = lines[i + 1].strip() if i + 1 < len(lines) else ""
            parts.append(f"panicked at {match[1]}: {message}")
            break
    for line in lines:
        match = ERROR.match(line)
        if match:
            parts.append(match[1].strip()[:200])
            break
    return "; ".join(parts) or None


def parse_log(text):
    """Seed, coverage counters, first and last status lines and final stats of one libFuzzer log."""
    lines = text.splitlines()
    result = {"seed": None, "coverage_counters": 0, "seed_corpus_files": None, "initial": None, "final": None,
              "stats": {}, "artifacts": [], "done_seconds": None, "failure": failure(lines)}
    for line in lines:
        if match := re.match(r"^INFO: Seed: (\d+)", line):
            result["seed"] = int(match[1])
        elif match := COUNTERS.match(line):
            result["coverage_counters"] += int(match[1])
        elif match := re.match(r"^INFO: seed corpus: files: (\d+)", line):
            result["seed_corpus_files"] = int(match[1])
        elif match := STATUS.match(line):
            row = {"executions": int(match[1]), "event": match[2]}
            for key, pattern in FIELDS:
                if value := re.search(pattern, match[3]):
                    row[key] = value[1] if key == "corpus_size" else int(value[1])
            if match[2] == "INITED":
                result["initial"] = row
            result["final"] = row
        elif match := re.match(r"^stat::(\w+):\s+(\d+)", line):
            result["stats"][match[1]] = int(match[2])
        elif match := re.search(r"Test unit written to (.+)$", line):
            result["artifacts"].append(match[1].strip())
        elif match := re.match(r"^Done \d+ runs in (\d+) second", line):
            result["done_seconds"] = int(match[1])
    return result


def sanitizer_of(binary):
    data = Path(binary).read_bytes()
    return "address" if any(marker in data for marker in ASAN_MARKERS) else None


def libfuzzer_flags(config, seconds=None):
    """Per-input limits for every run; -max_len and the time budget only when fuzzing,
    because libFuzzer's -minimize_crash step asserts that -max_len is unset."""
    flags = [f"-timeout={config['timeout']}", f"-rss_limit_mb={config['rss_limit_mb']}",
             f"-malloc_limit_mb={config['malloc_limit_mb']}"]
    if seconds:
        flags = [f"-max_len={config['max_len']}", *flags, f"-max_total_time={seconds}", "-print_final_stats=1"]
    return flags


def execute(command, log_path, timeout, env=None, cwd=None):
    """Run with output to log_path; kill the process group after timeout. Returns (exit code or None, seconds)."""
    started = time.monotonic()
    with open(log_path, "wb") as log:
        process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=env, cwd=cwd,
                                   start_new_session=os.name == "posix")
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
            process.wait()
            code = None
    return code, time.monotonic() - started


def read(path):
    return Path(path).read_text(encoding="utf-8", errors="replace")


def run_target(target, binary, seconds, output, work, command=None, require_instrumented=False,
               minimize_seconds=180, build=None):
    """Fuzz one target, minimize and replay its findings, and write output/<target>/report.json."""
    config = fuzz_seeds.TARGETS[target]
    command = list(command or [str(Path(binary).resolve())])
    out, work = Path(output) / target, Path(work)
    findings, corpus, scratch = out / "findings", work / "corpus" / target, work / "scratch" / target
    for directory in (out, corpus, scratch):
        shutil.rmtree(directory, ignore_errors=True)
        directory.mkdir(parents=True)
    findings.mkdir()
    manifest = fuzz_seeds.assemble(target, work / "seeds")
    shutil.copy(work / "seeds" / f"{target}.json", out / "seeds.json")
    env = {**os.environ, "RUST_BACKTRACE": "1"}
    asan = env.get("ASAN_OPTIONS", "")
    if "detect_odr_violation" not in asan:  # as cargo fuzz run sets it
        env["ASAN_OPTIONS"] = ":".join(filter(None, (asan, "detect_odr_violation=0")))
    fuzz = [*command, *libfuzzer_flags(config, seconds), f"-artifact_prefix={findings.resolve()}{os.sep}",
            str(corpus.resolve()), str((work / "seeds" / target).resolve())]
    code, duration = execute(fuzz, out / "run.log", seconds + 300, env, scratch)
    log = parse_log(read(out / "run.log"))

    def reproduces(path, log_path):
        replay = [*command, *libfuzzer_flags(config), f"-artifact_prefix={scratch.resolve()}{os.sep}", str(path.resolve())]
        return execute(replay, log_path, 2 * config["timeout"] + 30, env, scratch)[0] not in (0, None)

    results, slow = [], []
    for path in sorted(findings.iterdir()):
        kind = path.name.split("-", 1)[0]
        if kind == "slow":
            slow.append({"artifact": path.name, "bytes": path.stat().st_size})
        if kind not in FAILURES:
            continue
        minimized, replay_log = findings / f"minimized-{path.name}", out / f"reproduce-{path.name}.log"
        _, spent = execute([*command, *libfuzzer_flags(config), "-minimize_crash=1", "-runs=255",
                            f"-exact_artifact_path={minimized.resolve()}", str(path.resolve())],
                           out / f"minimize-{path.name}.log", minimize_seconds, env, scratch)
        if minimized.is_file() and not reproduces(minimized, replay_log):
            minimized.unlink()  # keep only a minimized input that fails again
        failing = minimized.is_file() or reproduces(path, replay_log)
        data = path.read_bytes()
        results.append({"kind": kind, "artifact": path.name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                        "minimized": minimized.name if minimized.is_file() else None,
                        "minimized_bytes": minimized.stat().st_size if minimized.is_file() else None,
                        "minimize_seconds": round(spent, 1), "reproduces": failing,
                        "summary": failure(read(replay_log).splitlines()) or log["failure"]})
    sanitizer = sanitizer_of(binary)
    instrumented = log["coverage_counters"] > 0 and sanitizer is not None
    error = None
    if not results:
        if code is None:
            error = f"libFuzzer did not stop within {seconds + 300} s"
        elif code != 0:
            error = f"libFuzzer exited with {code} and wrote no failure artifact: {log['failure'] or 'see run.log'}"
        elif log["final"] is None:
            error = "run.log has no libFuzzer status line"
        elif require_instrumented and not instrumented:
            error = (f"not an instrumented run: {log['coverage_counters']} coverage counters, "
                     f"sanitizer runtime {'found' if sanitizer else 'missing'}")
    report = {"format_version": 1, "target": target, "instrumented": instrumented, "engine": "libFuzzer",
              "sanitizer": sanitizer, "coverage_counters": log["coverage_counters"], "build": build,
              "status": "finding" if results else "error" if error else "clean", "error": error,
              "budget_seconds": seconds, "duration_seconds": round(duration, 1), "exit_code": code,
              "libfuzzer_seed": log["seed"], "options": {key: config[key] for key in ("max_len", "timeout", "rss_limit_mb", "malloc_limit_mb")},
              "seeds": {"files": len(manifest["seeds"]), "bytes": sum(s["bytes"] for s in manifest["seeds"]),
                        "skipped": len(manifest["skipped"])},
              "initial": log["initial"], "final": log["final"], "stats": log["stats"],
              "new_corpus_files": sum(1 for _ in corpus.iterdir()), "findings": results, "slow_units": slow,
              "failure": log["failure"], "command": fuzz}
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(describe(report))
    if report["status"] != "clean":
        print(f"--- {out / 'run.log'} from the failure ---")
        print(excerpt(read(out / "run.log")))
    publish(markdown_runs([report], heading=f"### Fuzz target `{target}`"), annotations(report))
    return 0 if report["status"] == "clean" else 1


def excerpt(text, limit=120):
    """The log from its first panic or error line (else its last 60 lines), at most limit lines."""
    lines = text.splitlines()
    start = next((i for i, line in enumerate(lines) if PANIC.match(line) or ERROR.match(line)), max(0, len(lines) - 60))
    return "\n".join(lines[start:start + limit])


def number(value):
    return "-" if value is None else f"{value:,}"


def span(report, key):
    first, last = (report.get("initial") or {}).get(key), (report.get("final") or {}).get(key)
    return "-" if last is None else f"{number(first)} -> {number(last)}"


def describe(report):
    final = report.get("final") or {}
    text = (f"{report['target']}: {report['status']}; instrumented: {str(report['instrumented']).lower()} "
            f"(sanitizer {report['sanitizer'] or 'none'}, {number(report['coverage_counters'])} coverage counters); "
            f"{number(final.get('executions'))} executions in {report['duration_seconds']} s; "
            f"coverage {span(report, 'cov')}, features {span(report, 'ft')}; corpus {number(final.get('corpus_units'))} units")
    for f in report["findings"]:
        minimized = "no" if f["minimized"] is None else f"{f['minimized_bytes']} bytes"
        text += (f"\nFINDING {f['kind']} {f['artifact']} ({f['bytes']} bytes, minimized: {minimized}, "
                 f"reproduces: {str(f['reproduces']).lower()}): {f['summary']}")
    if report["error"]:
        text += f"\nERROR {report['error']}"
    return text


def escape(text, property=False):
    """GitHub workflow-command escaping; property values also escape ':' and ','."""
    text = str(text).replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    return text.replace(":", "%3A").replace(",", "%2C") if property else text


def annotations(report):
    """One `::error` workflow command per finding and for a run error."""
    lines, target = [], report["target"]
    for f in report["findings"]:
        minimized = f"{f['minimized']} ({f['minimized_bytes']} bytes)" if f["minimized"] else "not reduced"
        title = escape(f"fuzz {target}: {f['kind']}", property=True)
        lines.append(f"::error title={title}::" + escape(
            f"{f['summary']}. Artifact fuzz-report-{target}: {target}/findings/{f['artifact']} "
            f"({f['bytes']} bytes), minimized {minimized}; reproduces: {str(f['reproduces']).lower()}. "
            f"Commit it under fuzz/regressions/{target}/ with the fix (fuzz/README.md)."))
    if report["error"]:
        title = escape(f"fuzz {target}: run error", property=True)
        lines.append(f"::error title={title}::" + escape(report["error"]))
    return lines


def publish(markdown, notes=()):
    if os.environ.get("GITHUB_ACTIONS") == "true":
        for line in notes:
            print(line)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as stream:
            stream.write(markdown)


def cell(text):
    return html.escape(str(text)).replace("|", "&#124;").replace("\n", " ")


def markdown_runs(reports, heading):
    lines = [heading, "", "| Target | Instrumented | Status | Seconds | Executions | exec/s | Coverage | Features "
             "| Corpus | Seeds | Peak RSS MB | Findings |", "| --- | --- | --- | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: | --- |"]
    for r in reports:
        final = r.get("final") or {}
        instrumented = f"yes ({r['sanitizer']})" if r["instrumented"] else "**no**"
        found = ", ".join(f"{f['kind']} `{f['artifact'][:24]}`" for f in r["findings"]) or "-"
        lines.append(f"| `{r['target']}` | {instrumented} | {r['status']} | {r['duration_seconds']} | "
                     f"{number(final.get('executions'))} | {number(r['stats'].get('average_exec_per_sec'))} | "
                     f"{span(r, 'cov')} | {span(r, 'ft')} | {number(final.get('corpus_units'))} | {r['seeds']['files']} | "
                     f"{number(r['stats'].get('peak_rss_mb'))} | {cell(found)} |")
    details = [f"- **`{r['target']}`** {f['kind']} `{f['artifact']}` ({f['bytes']} B, "
               f"{'minimized to ' + str(f['minimized_bytes']) + ' B' if f['minimized'] else 'not reduced'}, "
               f"{'reproduces' if f['reproduces'] else 'did not reproduce'}): {cell(f['summary'])}. "
               f"Artifact `fuzz-report-{r['target']}`, file `{r['target']}/findings/{f['minimized'] or f['artifact']}`."
               for r in reports for f in r["findings"]]
    errors = [f"- **`{r['target']}`** error: {cell(r['error'])}" for r in reports if r["error"]]
    if details or errors:
        lines += ["", *details, *errors]
    if details:
        lines += ["", "Commit a minimized finding as `fuzz/regressions/<target>/<name>` with its fix; "
                  "`cargo test -p tessstep --test fuzz_regressions` replays it on stable (see `fuzz/README.md`)."]
    return "\n".join(lines) + "\n\n"


def smoke_command(row):
    command = ["cargo", "test", "--locked", "-p", row["package"], "--test", row["test"], "--", "--nocapture"]
    return command + (["--exact", row["filter"]] if row["filter"] else [])


def smoke_counts(text):
    passed = sum(int(n) for n in re.findall(r"test result: \w+\. (\d+) passed", text))
    failed = sum(int(n) for n in re.findall(r"test result: \w+\. \d+ passed; (\d+) failed", text))
    cases = next((int(m[1].replace(",", "")) for line in text.splitlines()
                  if (m := re.search(r"(\d[\d,]*)\D*\bcases\b", line))), None)
    replayed = re.search(r"replayed (\d+) committed findings", text)
    return {"passed": passed, "failed": failed, "cases": cases,
            "findings_replayed": int(replayed[1]) if replayed else None}


def run_smoke(output, rows=None, runner=subprocess.run):
    """Run every stable smoke test and write output/smoke/report.json; instrumented is always false."""
    out = Path(output) / "smoke"
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    results = []
    with open(out / "smoke.log", "w", encoding="utf-8") as log:
        for row in fuzz_seeds.SMOKE if rows is None else rows:
            command = smoke_command(row)
            started = time.monotonic()
            done = runner(command, cwd=ROOT, capture_output=True, text=True, env={**os.environ, "CARGO_TERM_COLOR": "never"})
            text = (done.stdout or "") + (done.stderr or "")
            log.write(f"$ {' '.join(command)}\n{text}\n")
            results.append({"name": row["name"], "package": row["package"], "test": row["test"], "targets": row["targets"],
                            "exercise": row["exercise"], "instrumented": False,
                            "engine": "cargo test (stable; no coverage or sanitizer instrumentation)",
                            "status": "passed" if done.returncode == 0 else "failed", "exit_code": done.returncode,
                            "seconds": round(time.monotonic() - started, 1), **smoke_counts(text), "command": command})
    report = {"format_version": 1, "instrumented": False, "tests": results}
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    for r in results:
        print(f"{r['name']}: {r['status']} in {r['seconds']} s ({r['package']} --test {r['test']}; targets {', '.join(r['targets'])})")
    return int(any(r["status"] != "passed" for r in results))


def load(directory):
    runs, smoke = {}, None
    for path in sorted(Path(directory).glob("*/report.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        if path.parent.name == "smoke":
            smoke = data
        else:
            runs[data["target"]] = data
    return runs, smoke


def summarize(directory, output, expected=None, build_result=None):
    """Aggregate run and smoke reports; returns 1 unless every expected target ran clean and instrumented."""
    runs, smoke = load(directory)
    expected = sorted(expected or fuzz_seeds.cargo_targets())
    reports = [runs[name] for name in sorted(runs)]
    missing = [name for name in expected if name not in runs]
    tests = smoke["tests"] if smoke else []
    totals = {"targets": len(expected), "reported": len(reports), "instrumented": sum(r["instrumented"] for r in reports),
              "clean": sum(r["status"] == "clean" for r in reports), "with_findings": sum(r["status"] == "finding" for r in reports),
              "findings": sum(len(r["findings"]) for r in reports), "errors": sum(r["status"] == "error" for r in reports),
              "missing": len(missing), "executions": sum((r.get("final") or {}).get("executions", 0) for r in reports),
              "smoke_tests": len(tests), "smoke_passed": sum(t["status"] == "passed" for t in tests),
              "smoke_reported": smoke is not None}
    summary = {"format_version": 1, "build_result": build_result, "totals": totals, "missing": missing,
               "instrumented_runs": reports, "driver_smoke": tests}
    out = Path(output)
    out.mkdir(parents=True, exist_ok=True)
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    markdown = summary_markdown(summary)
    (out / "summary.md").write_text(markdown, encoding="utf-8")
    publish(markdown)
    print(markdown)
    ok = (not missing and all(r["status"] == "clean" and r["instrumented"] for r in reports)
          and smoke is not None and totals["smoke_passed"] == len(tests))
    return 0 if ok else 1


def summary_markdown(summary):
    t = summary["totals"]
    budgets = sorted({r["budget_seconds"] for r in summary["instrumented_runs"]})
    lines = ["## Fuzzing: instrumented runs and stable driver smoke", "",
             f"**Instrumented (libFuzzer + AddressSanitizer, `instrumented: true`):** {t['instrumented']} of {t['targets']} "
             f"targets ran instrumented for {', '.join(map(str, budgets)) or '-'} s each: {t['clean']} clean, "
             f"{t['with_findings']} with findings ({t['findings']} inputs), {t['errors']} errors, {t['missing']} missing; "
             f"{t['executions']:,} executions.", "",
             f"**Driver smoke (stable `cargo test`, `instrumented: false`):** "
             + (f"{t['smoke_passed']} of {t['smoke_tests']} tests passed." if t["smoke_reported"] else "no report."), ""]
    if summary["missing"]:
        lines += [f"No run report for {', '.join(f'`{m}`' for m in summary['missing'])} "
                  f"(instrumented build: {summary['build_result'] or 'unknown'}; a missing report means the build or "
                  "the target's job failed or was cancelled).", ""]
    lines.append(markdown_runs(summary["instrumented_runs"], "### libFuzzer runs").rstrip("\n"))
    lines += ["", "### Stable driver smoke (not instrumented)", "",
              "| Test | Package | Targets | Exercise | Status | Cases | Seconds |", "| --- | --- | --- | --- | --- | ---: | ---: |"]
    for r in summary["driver_smoke"]:
        cases = number(r["cases"]) if r["findings_replayed"] is None else f"{r['findings_replayed']} findings"
        lines.append(f"| `{r['name']}` | {r['package']} | {cell(', '.join(r['targets']))} | {cell(r['exercise'])} | "
                     f"{r['status']} | {cases} | {r['seconds']} |")
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run", help="fuzz one target with a prebuilt instrumented binary")
    run.add_argument("--target", required=True, choices=sorted(fuzz_seeds.TARGETS))
    run.add_argument("--binary", required=True, type=Path)
    run.add_argument("--seconds", type=int, default=fuzz_seeds.DEFAULT_BUDGET)
    run.add_argument("--output", type=Path, default=ROOT / "reports/fuzz")
    run.add_argument("--work", type=Path, default=ROOT / "target/fuzz-work")
    run.add_argument("--minimize-seconds", type=int, default=180)
    run.add_argument("--build", help="how the binary was built, recorded in the report")
    run.add_argument("--require-instrumented", action="store_true",
                     help="report an error unless the run shows coverage counters and AddressSanitizer")
    smoke = commands.add_parser("smoke", help="run the stable fuzz smoke tests")
    smoke.add_argument("--output", type=Path, default=ROOT / "reports/fuzz")
    summary = commands.add_parser("summary", help="aggregate run and smoke reports")
    summary.add_argument("--input", type=Path, default=ROOT / "reports/fuzz")
    summary.add_argument("--output", type=Path, default=ROOT / "reports/fuzz-summary")
    summary.add_argument("--expected", default="", help="JSON list or comma separated targets (default: fuzz/Cargo.toml)")
    summary.add_argument("--build-result", default=None)
    args = parser.parse_args(argv)
    if args.command == "run":
        if not 1 <= args.seconds <= fuzz_seeds.MAX_BUDGET:
            parser.error(f"--seconds must be between 1 and {fuzz_seeds.MAX_BUDGET}")
        return run_target(args.target, args.binary, args.seconds, args.output, args.work,
                          require_instrumented=args.require_instrumented, minimize_seconds=args.minimize_seconds,
                          build=args.build)
    if args.command == "smoke":
        return run_smoke(args.output)
    expected = args.expected.strip()
    expected = json.loads(expected) if expected.startswith("[") else [e for e in re.split(r"[,\s]+", expected) if e]
    return summarize(args.input, args.output, expected, args.build_result)


if __name__ == "__main__":
    sys.exit(main())
