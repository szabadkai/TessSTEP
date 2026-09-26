#!/usr/bin/env python3
"""Run local STEP inputs through stepdump and retain comparable progress reports."""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import math
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from corpus_report import aggregate, render, FAULTS, stages_of

ROOT = Path(__file__).resolve().parents[1]
EXTENSIONS = {".step", ".stp", ".p21"}
STAGES = ("physical_parse", "references", "schema", "product", "geometry", "tessellation")


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def discover(root):
    """Hash all inputs; run identical bytes once, retain every relative filename."""
    cases = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = sorted(d for d in dirs if d not in {".git", "target", "__pycache__"})
        for name in sorted(files):
            path = Path(directory) / name
            if path.suffix.lower() not in EXTENSIONS:
                continue
            key = digest(path)
            case = cases.setdefault(key, {"sha256": key, "bytes": path.stat().st_size, "paths": []})
            case["paths"].append(path.relative_to(root).as_posix())
    return sorted(cases.values(), key=lambda case: case["paths"][0])


def inspect(binary, path, timeout):
    started = time.monotonic()
    result = {"status": "runner_error", "stages": dict.fromkeys(STAGES, "not_run"), "diagnostics": []}
    for stage in ("schema", "product"):
        result["stages"][stage] = "not_configured"
    for stage in ("geometry", "tessellation"):
        result["stages"][stage] = "not_integrated"
    # Disk-backed output avoids unbounded capture of adversarial diagnostics.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            process = subprocess.run([str(binary), "--json", str(path)], stdout=stdout, stderr=stderr, timeout=timeout)
            result["exit_code"] = process.returncode
            stderr.seek(0)
            result["stderr"] = stderr.read(4096).decode("utf-8", errors="replace")
            if process.returncode not in (0, 1):
                result["status"] = "crash" if process.returncode < 0 or process.returncode == 101 else "runner_error"
            elif stdout.tell() > 16 * 1024 * 1024:
                result["stderr"] = "stepdump JSON exceeds the runner's 16 MiB output budget"
            else:
                stdout.seek(0)
                raw_output = stdout.read()
                result["output_sha256"] = hashlib.sha256(raw_output).hexdigest()
                payload = json.loads(raw_output)
                if payload.get("format_version") != 1 or payload.get("scope") != "physical-syntax":
                    raise ValueError("unsupported stepdump output format/scope")
                diagnostics = payload["diagnostics"]
                document = payload["document"]
                if not isinstance(diagnostics, list) or any(not isinstance(d, dict) or d.get("severity") not in {"error", "warning"} for d in diagnostics):
                    raise ValueError("invalid physical diagnostics")
                has_errors = any(d["severity"] == "error" for d in diagnostics)
                if process.returncode != int(has_errors) or (document is None and not has_errors):
                    raise ValueError("physical parser exit status disagrees with payload")
                result["diagnostic_counts"] = dict(Counter(d["code"] for d in diagnostics))
                result["diagnostics"] = diagnostics[:20]
                result["diagnostics_truncated"] = len(diagnostics) > 20
                if document is None:
                    result["status"] = "rejected"
                    result["stages"]["physical_parse"] = "rejected"
                else:
                    result["entity_count"] = document["entity_count"]
                    result["entity_counts"] = document.get("entity_counts", {})
                    headers = document["headers"]
                    if isinstance(headers, dict):
                        result["schemas"] = headers.get("FILE_SCHEMA", [])
                    else:
                        result["schemas"] = [h["parameters"] for h in headers if h["name"] == "FILE_SCHEMA"]
                    result["missing_references"] = len(document["unresolved_references"])
                    result["stages"]["physical_parse"] = "accepted"
                    result["stages"]["references"] = "missing" if result["missing_references"] else "resolved"
                    result["status"] = "reference_errors" if result["missing_references"] else "clean"
        except subprocess.TimeoutExpired:
            result["status"] = "timeout"
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
            result["status"] = "runner_error"
            result["stderr"] = str(error)
    result["seconds"] = round(time.monotonic() - started, 4)
    return result


def inspect_schema(binary, schema_name, path, timeout, result, *, stage="schema"):
    """Run an explicit stage validator or product checker after its prerequisite."""
    prerequisite = "schema" if stage == "product" else "physical_parse"
    scope = "product-structure" if stage == "product" else "schema-structure"
    if result["stages"][prerequisite] != "accepted":
        result["stages"][stage] = "not_run"
        return
    started = time.monotonic()
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            process = subprocess.run([str(binary), schema_name, str(path)], stdout=stdout, stderr=stderr, timeout=timeout)
            stderr.seek(0)
            result[stage + "_stderr"] = stderr.read(4096).decode("utf-8", errors="replace")
            if process.returncode not in (0, 1):
                result["status"] = "crash" if process.returncode < 0 or process.returncode == 101 else "runner_error"
                result["stages"][stage] = result["status"]
            else:
                if stdout.tell() > 64 * 1024:
                    raise ValueError("stage validator JSON exceeds 64 KiB")
                stdout.seek(0)
                payload = json.load(stdout)
                status = payload["status"]
                if payload.get("format_version") != 1 or payload.get("scope") != scope:
                    raise ValueError("unsupported stage validator protocol")
                if status not in {"accepted", "rejected", "unsupported", "not_configured", "resource_limit"}:
                    raise ValueError("invalid schema status after physical acceptance")
                if (status == "accepted") != (process.returncode == 0):
                    raise ValueError("stage validator exit status disagrees with payload")
                if status == "accepted" and payload.get("entity_count") != result["entity_count"]:
                    raise ValueError("stage validator entity count differs from physical parser")
                result["stages"][stage] = status
                result[stage + "_result"] = payload
        except subprocess.TimeoutExpired:
            result["status"] = "timeout"
            result["stages"][stage] = "timeout"
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
            result["status"] = "runner_error"
            result["stages"][stage] = "runner_error"
            result[stage + "_stderr"] = str(error)
    result["seconds"] = round(result["seconds"] + time.monotonic() - started, 4)


ROOT_FAILURES = ("unsupported", "rejected", "resource_limit")


def summarize_survey(payload):
    """Bound per-file evidence: counts and categories for every root, details for the first 20."""
    roots = payload["roots"]
    failures = [r for r in roots if r["status"] != "accepted"]
    reached = lambda stage: [r for r in roots if r["stages"][stage] == "accepted"]
    return {"metres_per_unit": payload["metres_per_unit"], "root_count": payload["root_count"],
            "surveyed_roots": len(roots), "truncated": payload["truncated"],
            "geometry_roots": len(reached("geometry")), "accepted_roots": len(reached("tessellation")),
            "outcomes": dict(sorted(Counter("accepted" if r["status"] == "accepted" else f'{r["failed_stage"]}:{r["kind"]}' for r in roots).items())),
            # Entity IDs vary per file; digits are removed so categories aggregate.
            "categories": dict(Counter(re.sub(r"\d+", "N", r["message"]) for r in failures).most_common(10)),
            "unsupported_entity_types": dict(Counter(r["entity_type"] for r in failures if r["failed_stage"] == "profile" and r["kind"] == "unsupported").most_common(10)),
            "roots": [{k: v for k, v in r.items() if k != "seconds"} for r in roots[:20]]}


def stage_outcome(roots, stage):
    """All roots accepted, some accepted (partial), none reached, or the dominant failure.

    The file geometry stage covers profile, geometry and topology failures; the
    tessellation stage counts only roots that reached tessellation.
    """
    accepted = sum(r["stages"][stage] == "accepted" for r in roots)
    if roots and accepted == len(roots):
        return "accepted"
    if accepted:
        return "partial"
    failed = Counter(r["status"] for r in roots if stage == "geometry" or r["stages"][stage] in ROOT_FAILURES)
    return max(sorted(failed), key=failed.get) if failed else "not_run"


def inspect_geometry(binary, path, timeout, result, metres_per_unit):
    """Import every solid root with the selected-root profiles after physical acceptance."""
    if result["stages"]["physical_parse"] != "accepted":
        result["stages"]["geometry"] = result["stages"]["tessellation"] = "not_run"
        return
    started = time.monotonic()
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            process = subprocess.run([str(binary), str(path), repr(metres_per_unit)], stdout=stdout, stderr=stderr, timeout=timeout)
            stderr.seek(0)
            result["geometry_stderr"] = stderr.read(4096).decode("utf-8", errors="replace")
            if process.returncode != 0:
                result["status"] = "crash" if process.returncode < 0 or process.returncode == 101 else "runner_error"
                result["stages"]["geometry"] = result["stages"]["tessellation"] = result["status"]
            else:
                if stdout.tell() > 16 * 1024 * 1024:
                    raise ValueError("solid survey JSON exceeds 16 MiB")
                stdout.seek(0)
                payload = json.load(stdout)
                if payload.get("format_version") != 1 or payload.get("scope") != "solid-survey":
                    raise ValueError("unsupported solid survey protocol")
                if payload.get("parse") != "accepted":
                    raise ValueError("solid survey rejected a physically accepted input")
                roots = payload["roots"]
                if not isinstance(roots, list) or len(roots) != min(payload["root_count"], len(roots)) or payload["truncated"] != (payload["root_count"] > len(roots)):
                    raise ValueError("solid survey root counts disagree")
                for root in roots:
                    if root["status"] not in ("accepted", *ROOT_FAILURES) or set(root["stages"]) != {"profile", "geometry", "tessellation"}:
                        raise ValueError("invalid solid survey root outcome")
                result["geometry_result"] = summarize_survey(payload)
                result["stages"]["geometry"] = stage_outcome(roots, "geometry") if roots else "no_solid_roots"
                result["stages"]["tessellation"] = stage_outcome(roots, "tessellation") if roots else "not_run"
        except subprocess.TimeoutExpired:
            result["status"] = "timeout"
            result["stages"]["geometry"] = result["stages"]["tessellation"] = "timeout"
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
            result["status"] = "runner_error"
            result["stages"]["geometry"] = result["stages"]["tessellation"] = "runner_error"
            result["geometry_stderr"] = str(error)
    result["seconds"] = round(result["seconds"] + time.monotonic() - started, 4)


def signature(case):
    result = {key: case.get(key) for key in ("status", "stages", "diagnostic_counts", "entity_count", "missing_references", "schema_result", "product_result")}
    result["stages"] = stages_of(case)
    # Old baselines predate configured stages; reporting vocabulary is not a regression.
    for stage in ("schema", "product"):
        if result["stages"][stage] in {"not_configured", "not_run"} and not case.get(stage + "_result"):
            result["stages"][stage] = "unmeasured"
    if not case.get("geometry_result"):
        for stage in ("geometry", "tessellation"):
            if result["stages"][stage] == "not_integrated":
                result["stages"][stage] = "unmeasured"
    # Saved baselines keep only this count, not the full per-file survey summary.
    geometry = case.get("geometry_result")
    result["geometry_accepted_roots"] = geometry["accepted_roots"] if geometry else case.get("geometry_accepted_roots")
    return result


def compare(cases, previous):
    old = {case["sha256"]: case for case in previous.get("cases", [])}
    current = {case["sha256"]: case for case in cases}
    changes = []
    for key, case in current.items():
        before = old.get(key)
        if before is None:
            changes.append({"sha256": key, "path": case["paths"][0], "kind": "added"})
        elif signature(before) != signature(case):
            # This is a compatibility regression, not a standards-conformance verdict.
            regression = (before["status"] == "clean" and case["status"] != "clean") or (
                before["stages"]["physical_parse"] == "accepted" and case["stages"]["physical_parse"] != "accepted")
            regression |= (case.get("missing_references") or 0) > (before.get("missing_references") or 0)
            regression |= before["stages"].get("schema") == "accepted" and case["stages"].get("schema") != "accepted"
            regression |= before["stages"].get("product") == "accepted" and case["stages"].get("product") != "accepted"
            regression |= any(before["stages"].get(s) == "accepted" and case["stages"].get(s) != "accepted" for s in ("geometry", "tessellation"))
            accepted_before, accepted_after = signature(before)["geometry_accepted_roots"], signature(case)["geometry_accepted_roots"]
            regression |= accepted_before is not None and (accepted_after or 0) < accepted_before
            regression |= case["status"] in FAULTS and before["status"] not in FAULTS
            changes.append({"sha256": key, "path": case["paths"][0], "kind": "regression" if regression else "changed",
                            "before": before["status"], "after": case["status"],
                            "details": {k: {"before": signature(before).get(k), "after": signature(case).get(k)} for k in signature(case) if signature(before).get(k) != signature(case).get(k)}})
    changes.extend({"sha256": key, "path": case["paths"][0], "kind": "removed"} for key, case in old.items() if key not in current)
    return changes


def atomic_write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent, delete=False) as stream:
        temporary = Path(stream.name)
        stream.write(text)
    temporary.replace(path)


def read_report(path):
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))



def repeat_inspection(binary, path, timeout, case, repeat):
    if repeat == 1 or case["status"] in FAULTS:
        return
    case["determinism"] = {"runs": 1, "status": "matched"}
    for _ in range(repeat - 1):
        again = inspect(binary, path, timeout)
        case["seconds"] = round(case["seconds"] + again["seconds"], 4)
        case["determinism"]["runs"] += 1
        if again["status"] in FAULTS:
            case["status"] = again["status"]
            case["determinism"]["status"] = again["status"]
            break
        if again.get("output_sha256") != case.get("output_sha256"):
            case["status"] = "nondeterministic"
            case["determinism"]["status"] = "mismatch"
            break


def contains_expected(actual, expected):
    """Subset object matching with strict arrays and numerical oracle tolerance."""
    if isinstance(expected, dict):
        return isinstance(actual, dict) and all(k in actual and contains_expected(actual[k], v) for k, v in expected.items())
    if isinstance(expected, list):
        return isinstance(actual, list) and len(actual) == len(expected) and all(contains_expected(a, b) for a, b in zip(actual, expected))
    if isinstance(expected, float):
        return isinstance(actual, (int, float)) and math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12)
    return actual == expected


def check_expectations(case, expectations):
    case["expectations"] = {p: expectations[p] for p in case["paths"] if p in expectations}
    failures = []
    for path, expected in case["expectations"].items():
        if not isinstance(expected, dict) or not expected:
            raise ValueError(f"empty or invalid expectation for {path}")
        for key, value in expected.items():
            if key == "stages":
                if not isinstance(value, dict) or not value or any(s not in STAGES or not isinstance(v, str) for s, v in value.items()):
                    raise ValueError(f"invalid stage expectations for {path}")
                for stage, status in value.items():
                    if case["stages"].get(stage) != status:
                        failures.append(f"{path}: {stage}: expected {status}, got {case['stages'].get(stage)}")
            elif key == "diagnostic_codes":
                if sorted(case.get("diagnostic_counts", {})) != sorted(value):
                    failures.append(f"{path}: expected diagnostic codes {value}, got {list(case.get('diagnostic_counts', {}))}")
            elif key == "product_result_contains":
                if not value or not contains_expected(case.get("product_result"), value):
                    failures.append(f"{path}: product measurements: expected {value}, got {case.get('product_result')}")
            elif key == "first_relationship_matrix":
                matrices = case.get("product_result", {}).get("relationship_matrices", [])
                if not matrices or not contains_expected(matrices[0], value):
                    failures.append(f"{path}: first relationship matrix: expected {value}, got {matrices[:1]}")
            elif key not in {"status", "entity_count", "entity_counts", "missing_references", "schema_result", "product_result"}:
                raise ValueError(f"unknown expectation key: {key}")
            elif case.get(key) != value:
                failures.append(f"{path}: {key}: expected {value}, got {case.get(key)}")
    case["expectation_failures"] = failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=Path(os.environ.get("TESSSTEP_CORPUS", "~/step-corpus")))
    parser.add_argument("--output", type=Path, default=ROOT / "reports/corpus")
    parser.add_argument("--baseline", type=Path, default=ROOT / "corpus/baseline.json")
    parser.add_argument("--save-baseline", action="store_true", help="Explicitly replace the reviewed baseline after a complete run")
    parser.add_argument("--title", default="TessSTEP external STEP compatibility")
    parser.add_argument("--expectations", type=Path, help="Reviewed per-path expected observations JSON")
    parser.add_argument("--repeat", type=int, default=1, help="Repeat physical parsing to check byte-identical JSON (1–5)")
    parser.add_argument("--check", action="store_true", help="Exit 1 for compatibility regressions, removed inputs, or runner failures")
    parser.add_argument("--timeout", type=float, default=30, help="Seconds per unique input (default: 30)")
    parser.add_argument("--schema-validator", type=Path, help="Compiled expressc --validator executable")
    parser.add_argument("--schema-name", help="Explicit schema for the configured validator")
    parser.add_argument("--product-validator", type=Path, help="Product checker; requires the schema validator and uses its schema name")
    parser.add_argument("--geometry-metres-per-unit", type=float, default=0.001,
                        help="Assumed source length unit for the solid import survey (default 0.001: millimetres); files' own units are not read yet")
    args = parser.parse_args()
    if not (math.isfinite(args.geometry_metres_per_unit) and args.geometry_metres_per_unit > 0):
        parser.error("--geometry-metres-per-unit must be finite and positive")
    if args.product_validator and (not args.schema_validator or not args.product_validator.is_file()):
        parser.error("--product-validator requires an existing checker and a schema validator")
    if bool(args.schema_validator) != bool(args.schema_name):
        parser.error("--schema-validator and --schema-name must be supplied together")
    if args.schema_validator and not args.schema_validator.is_file():
        parser.error("schema validator must be an existing executable")
    root = args.corpus.expanduser().resolve()
    if not 1 <= args.repeat <= 5:
        parser.error("--repeat must be between 1 and 5")
    if not root.is_dir() or args.timeout <= 0:
        parser.error("corpus must be an existing directory and timeout must be positive")
    if args.check and not args.baseline.exists():
        parser.error("--check requires a saved baseline")
    if args.check and args.save_baseline:
        parser.error("--check and --save-baseline must be separate review steps")
    print(f"Discovering STEP inputs in {root} …", flush=True)
    cases = discover(root)
    if not cases:
        parser.error("no .step, .stp or .p21 files found (archives are not extracted)")
    expectations = read_report(args.expectations).get("cases", {}) if args.expectations else {}
    if args.expectations:
        paths = {path for case in cases for path in case["paths"]}
        if set(expectations) != paths:
            parser.error("expectations must cover every discovered path exactly")
    subprocess.run(["cargo", "build", "--release", "--locked", "-p", "stepdump"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--release", "--locked", "-p", "tessstep-import", "--example", "survey"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version=1"], cwd=ROOT))
    suffix = ".exe" if os.name == "nt" else ""
    binary = Path(metadata["target_directory"]) / "release" / ("stepdump" + suffix)
    survey = Path(metadata["target_directory"]) / "release" / "examples" / ("survey" + suffix)
    source = hashlib.sha256()
    for path in sorted([ROOT / "Cargo.toml", ROOT / "Cargo.lock", *sorted((ROOT / "scripts").glob("*.py")), *ROOT.glob("crates/**/*.rs"), *ROOT.glob("crates/**/*.rs.txt"), *ROOT.glob("crates/**/Cargo.toml"), *ROOT.glob("tools/**/*.rs"), *ROOT.glob("tools/**/Cargo.toml")]):
        source.update(path.relative_to(ROOT).as_posix().encode())
        source.update(path.read_bytes())
    previous = read_report(args.output / "latest.json")
    baseline = read_report(args.baseline)
    run_id = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H-%M-%S-%fZ")
    print(f'{sum(len(c["paths"]) for c in cases)} paths; {len(cases)} unique inputs; timeout {args.timeout:g}s', flush=True)
    started = time.monotonic()
    # Another implementation task may rebuild stepdump while the suite is running.
    # Execute a private snapshot so every input sees the same executable.
    with tempfile.TemporaryDirectory(prefix="tessstep-corpus-") as directory:
        snapshot = Path(directory) / binary.name
        shutil.copy2(binary, snapshot)
        binary_hash = digest(snapshot)
        schema_snapshot = None
        schema_config = None
        if args.schema_validator:
            schema_snapshot = Path(directory) / ("schema-validator.exe" if os.name == "nt" else "schema-validator")
            shutil.copy2(args.schema_validator, schema_snapshot)
            schema_config = {"schema": args.schema_name, "binary_sha256": digest(schema_snapshot)}
        survey_snapshot = Path(directory) / survey.name
        shutil.copy2(survey, survey_snapshot)
        geometry_config = {"scope": "every solid root; selected-root planar/faceted profiles", "policy": "tolerant default",
                           "metres_per_unit": args.geometry_metres_per_unit, "binary_sha256": digest(survey_snapshot)}
        product_snapshot = None
        product_config = None
        if args.product_validator:
            product_snapshot = Path(directory) / ("product-validator.exe" if os.name == "nt" else "product-validator")
            shutil.copy2(args.product_validator, product_snapshot)
            product_config = {"schema": args.schema_name, "binary_sha256": digest(product_snapshot)}
        for index, case in enumerate(cases, 1):
            path = root / case["paths"][0]
            case.update(inspect(snapshot, path, args.timeout))
            repeat_inspection(snapshot, path, args.timeout, case, args.repeat)
            if schema_snapshot:
                inspect_schema(schema_snapshot, args.schema_name, path, args.timeout, case)
            if product_snapshot:
                inspect_schema(product_snapshot, args.schema_name, path, args.timeout, case, stage="product")
            if case["status"] not in FAULTS:
                inspect_geometry(survey_snapshot, path, args.timeout, case, args.geometry_metres_per_unit)
            check_expectations(case, expectations)
            print(f'[{index}/{len(cases)}] {case["status"]:16} {case["seconds"]:7.3f}s {case["paths"][0]}', flush=True)
    summary = {"files": sum(len(c["paths"]) for c in cases), "unique_inputs": len(cases),
               "unique_statuses": dict(Counter(c["status"] for c in cases)),
               "file_statuses": dict(Counter(s for c in cases for s in [c["status"]] * len(c["paths"]))),
               "seconds": round(time.monotonic() - started, 3)}
    baseline_changes = compare(cases, baseline) if baseline else []
    changed = {c["sha256"]: c["kind"] for c in baseline_changes}
    for case in cases:
        case["baseline_change"] = changed.get(case["sha256"], "unchanged" if baseline else "unreviewed")
    summary.update(aggregate(cases, bool(baseline)))
    history = previous.get("history", []) + [{"run_id": run_id, **summary}]
    report = {"format_version": 2, "title": args.title, "repeat": args.repeat,
              "baseline_sha256": digest(args.baseline) if args.baseline.exists() else None,
              "expectations_sha256": digest(args.expectations) if args.expectations else None, "run_id": run_id, "corpus_root": str(root), "source_fingerprint": source.hexdigest(),
              "binary_sha256": binary_hash, "schema_validator": schema_config, "product_validator": product_config, "geometry_survey": geometry_config, "timeout_seconds": args.timeout, "summary": summary, "cases": cases,
              "changes": compare(cases, previous) if previous else [], "baseline_present": bool(baseline),
              "baseline_changes": baseline_changes, "history": history}
    encoded = json.dumps(report, ensure_ascii=True, indent=2) + "\n"
    atomic_write(args.output / "runs" / f"{run_id}.json", encoded)
    atomic_write(args.output / "latest.json", encoded)
    atomic_write(args.output / "index.html", render(report))
    from corpus_summary import write_outputs
    write_outputs(report, args.output)
    broken = any(c["status"] in FAULTS or c.get("expectation_failures") for c in cases)
    if args.save_baseline:
        if broken:
            print("Baseline not saved: resolve crashes, timeouts and runner errors first.", file=sys.stderr)
        else:
            saved = {"format_version": 1, "run_id": run_id, "source_fingerprint": source.hexdigest(),
                     "cases": [{"sha256": c["sha256"], "paths": c["paths"], **signature(c), "stages": c["stages"]} for c in cases]}
            atomic_write(args.baseline, json.dumps(saved, indent=2) + "\n")
    print(json.dumps({k: summary[k] for k in ("files", "unique_inputs", "unique_statuses", "verdicts", "seconds")}, indent=2))
    print(f'Report: {args.output / "index.html"}')
    regression = any(c["kind"] in {"regression", "removed"} for c in report["baseline_changes"])
    return int(broken or (args.check and regression))


if __name__ == "__main__":
    sys.exit(main())
