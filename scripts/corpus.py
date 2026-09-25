#!/usr/bin/env python3
"""Run local STEP inputs through stepdump and retain comparable progress reports."""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import html
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

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
    for stage in STAGES[2:]:
        result["stages"][stage] = "not_implemented"
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
                payload = json.load(stdout)
                if payload.get("format_version") != 1 or payload.get("scope") != "physical-syntax":
                    raise ValueError("unsupported stepdump output format/scope")
                diagnostics = payload["diagnostics"]
                document = payload["document"]
                result["diagnostic_counts"] = dict(Counter(d["code"] for d in diagnostics))
                result["diagnostics"] = diagnostics[:20]
                result["diagnostics_truncated"] = len(diagnostics) > 20
                if document is None:
                    result["status"] = "rejected"
                    result["stages"]["physical_parse"] = "rejected"
                else:
                    result["entity_count"] = document["entity_count"]
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


def signature(case):
    result = {key: case.get(key) for key in ("status", "stages", "diagnostic_counts", "entity_count", "missing_references", "schema_result", "product_result")}
    result["stages"] = {"product": "not_implemented", **case.get("stages", {})}
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
            regression |= before["stages"].get("schema") == "accepted" and case["stages"].get("schema") != "accepted"
            regression |= before["stages"].get("product") == "accepted" and case["stages"].get("product") != "accepted"
            regression |= case["status"] in {"crash", "timeout", "runner_error"} and before["status"] not in {"crash", "timeout", "runner_error"}
            changes.append({"sha256": key, "path": case["paths"][0], "kind": "regression" if regression else "changed",
                            "before": before["status"], "after": case["status"]})
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


def render(report):
    esc = lambda value: html.escape(str(value), quote=True)
    counts = report["summary"]["unique_statuses"]
    rows = []
    for case in report["cases"]:
        detail = "; ".join(f'{d["code"]}: {d["message"]} (line {d["span"]["line"]})' for d in case["diagnostics"])
        if case.get("stderr"):
            detail += " " + case["stderr"]
        if case.get("schema_result"):
            detail += " schema: " + str(case["schema_result"])
        if case.get("schema_stderr"):
            detail += " " + case["schema_stderr"]
        if case.get("product_result"):
            detail += " product: " + str(case["product_result"])
        if case.get("product_stderr"):
            detail += " " + case["product_stderr"]
        paths = "<br>".join(esc(p) for p in case["paths"])
        rows.append(f'<tr data-status="{esc(case["status"])}"><td>{paths}</td><td>{esc(case["status"])}</td>'
                    + "".join(f'<td>{esc(case["stages"].get(s, "not_implemented"))}</td>' for s in STAGES)
                    + f'<td>{case.get("entity_count", "—")}</td><td>{case["seconds"]:.3f}</td><td>{esc(detail)}</td></tr>')
    changes = "".join(f'<li>{esc(c["kind"])}: {esc(c["path"])} {esc(c.get("before", ""))} → {esc(c.get("after", ""))}</li>' for c in report["changes"])
    history = "".join(f'<tr><td>{esc(h["run_id"])}</td><td>{h["files"]}</td><td>{h["unique_inputs"]}</td><td>{h["unique_statuses"].get("clean", 0)}</td><td>{h["unique_statuses"].get("rejected", 0)}</td><td>{h["unique_statuses"].get("reference_errors", 0)}</td></tr>' for h in report["history"][-30:])
    return f'''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>tessSTEP corpus progress</title><style>
body{{font:15px system-ui;margin:32px;color:#182534;background:#f6f8fb}}h1{{margin-bottom:8px}}.cards{{display:flex;gap:12px;flex-wrap:wrap}}
.card{{background:white;border:1px solid #ccd6e0;padding:18px;border-radius:9px}}strong{{font-size:24px}}table{{border-collapse:collapse;width:100%;background:white;margin-top:16px}}
td,th{{text-align:left;border-bottom:1px solid #dde3eb;padding:9px;vertical-align:top}}td:first-child{{overflow-wrap:anywhere;max-width:520px}}th{{background:#e6edf6;position:sticky;top:0}}
input,select{{font:inherit;padding:10px;margin:8px 8px 0 0}}input{{width:50%}}td:last-child{{max-width:520px;overflow-wrap:anywhere}}small{{color:#536275}}
</style><h1>tessSTEP corpus progress</h1><p>{esc(report["run_id"])} · {esc(report["source_fingerprint"][:12])} · {esc(report["corpus_root"])}</p>
<div class="cards"><div class="card"><strong>{report["summary"]["files"]}</strong><br>file paths</div><div class="card"><strong>{len(report["cases"])}</strong><br>unique inputs</div>
{''.join(f'<div class="card"><strong>{n}</strong><br>{esc(s)}</div>' for s,n in sorted(counts.items()))}</div>
<p>Identical bytes run once; every discovered path appears below. “Clean” means physical syntax accepted and references resolved.</p>
<p><b>Schema and product results appear separately when validators are configured. Geometry and tessellation: not implemented.</b> Rejected adversarial files may be correct behavior. These observations are not conformance scores. External resources and signatures are not verified.</p>
<details><summary>Changes from previous run ({len(report["changes"])})</summary><ul>{changes or '<li>No changes</li>'}</ul></details>
<p>Baseline comparison: {esc(dict(Counter(c["kind"] for c in report["baseline_changes"]))) if report["baseline_present"] else 'No baseline saved yet'}.</p>
<details open><summary>Run history (latest 30)</summary><table><tr><th>Run (UTC)</th><th>Paths</th><th>Unique</th><th>Clean</th><th>Rejected</th><th>Reference errors</th></tr>{history}</table></details>
<h2>Files</h2><input id="query" aria-label="Search files and diagnostics" placeholder="Search a filename, source, or diagnostic"><select id="status" aria-label="Filter status"><option value="">All statuses</option>{''.join(f'<option>{esc(s)}</option>' for s in sorted(counts))}</select><span id="visible"></span>
<table id="files"><thead><tr><th>File paths</th><th>Observed status</th><th>Physical parse</th><th>References</th><th>Schema structure</th><th>Product structure</th><th>Geometry</th><th>Tessellation</th><th>Entities</th><th>Seconds</th><th>Diagnostics (first 20)</th></tr></thead><tbody>{''.join(rows)}</tbody></table>
<script>const rows=[...document.querySelectorAll('#files tbody tr')],q=document.querySelector('#query'),s=document.querySelector('#status');
function filter(){{let n=0;for(const r of rows){{r.hidden=!(r.textContent.toLowerCase().includes(q.value.toLowerCase())&&(!s.value||r.dataset.status===s.value));if(!r.hidden)n++;}}document.querySelector('#visible').textContent=n+' unique inputs shown';}}
q.addEventListener('input',filter);s.addEventListener('change',filter);filter();</script></html>'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=Path(os.environ.get("TESSSTEP_CORPUS", "~/step-corpus")))
    parser.add_argument("--output", type=Path, default=ROOT / "reports/corpus")
    parser.add_argument("--baseline", type=Path, default=ROOT / "corpus/baseline.json")
    parser.add_argument("--save-baseline", action="store_true", help="Explicitly replace the reviewed baseline after a complete run")
    parser.add_argument("--check", action="store_true", help="Exit 1 for compatibility regressions, removed inputs, or runner failures")
    parser.add_argument("--timeout", type=float, default=30, help="Seconds per unique input (default: 30)")
    parser.add_argument("--schema-validator", type=Path, help="Compiled expressc --validator executable")
    parser.add_argument("--schema-name", help="Explicit schema for the configured validator")
    parser.add_argument("--product-validator", type=Path, help="Product checker; requires the schema validator and uses its schema name")
    args = parser.parse_args()
    if args.product_validator and (not args.schema_validator or not args.product_validator.is_file()):
        parser.error("--product-validator requires an existing checker and a schema validator")
    if bool(args.schema_validator) != bool(args.schema_name):
        parser.error("--schema-validator and --schema-name must be supplied together")
    if args.schema_validator and not args.schema_validator.is_file():
        parser.error("schema validator must be an existing executable")
    root = args.corpus.expanduser().resolve()
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
    subprocess.run(["cargo", "build", "--release", "--locked", "-p", "stepdump"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version=1"], cwd=ROOT))
    binary = Path(metadata["target_directory"]) / "release" / ("stepdump.exe" if os.name == "nt" else "stepdump")
    source = hashlib.sha256()
    for path in sorted([ROOT / "Cargo.toml", ROOT / "Cargo.lock", Path(__file__), *ROOT.glob("crates/**/*.rs"), *ROOT.glob("crates/**/*.rs.txt"), *ROOT.glob("crates/**/Cargo.toml"), *ROOT.glob("tools/**/*.rs"), *ROOT.glob("tools/**/Cargo.toml")]):
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
        product_snapshot = None
        product_config = None
        if args.product_validator:
            product_snapshot = Path(directory) / ("product-validator.exe" if os.name == "nt" else "product-validator")
            shutil.copy2(args.product_validator, product_snapshot)
            product_config = {"schema": args.schema_name, "binary_sha256": digest(product_snapshot)}
        for index, case in enumerate(cases, 1):
            path = root / case["paths"][0]
            case.update(inspect(snapshot, path, args.timeout))
            if schema_snapshot:
                inspect_schema(schema_snapshot, args.schema_name, path, args.timeout, case)
            if product_snapshot:
                inspect_schema(product_snapshot, args.schema_name, path, args.timeout, case, stage="product")
            print(f'[{index}/{len(cases)}] {case["status"]:16} {case["seconds"]:7.3f}s {case["paths"][0]}', flush=True)
    summary = {"files": sum(len(c["paths"]) for c in cases), "unique_inputs": len(cases),
               "unique_statuses": dict(Counter(c["status"] for c in cases)),
               "file_statuses": dict(Counter(s for c in cases for s in [c["status"]] * len(c["paths"]))),
               "seconds": round(time.monotonic() - started, 3)}
    history = previous.get("history", []) + [{"run_id": run_id, **summary}]
    report = {"format_version": 1, "run_id": run_id, "corpus_root": str(root), "source_fingerprint": source.hexdigest(),
              "binary_sha256": binary_hash, "schema_validator": schema_config, "product_validator": product_config, "timeout_seconds": args.timeout, "summary": summary, "cases": cases,
              "changes": compare(cases, previous) if previous else [], "baseline_present": bool(baseline),
              "baseline_changes": compare(cases, baseline) if baseline else [], "history": history}
    encoded = json.dumps(report, ensure_ascii=True, indent=2) + "\n"
    atomic_write(args.output / "runs" / f"{run_id}.json", encoded)
    atomic_write(args.output / "latest.json", encoded)
    atomic_write(args.output / "index.html", render(report))
    broken = any(c["status"] in {"crash", "timeout", "runner_error"} for c in cases)
    if args.save_baseline:
        if broken:
            print("Baseline not saved: resolve crashes, timeouts and runner errors first.", file=sys.stderr)
        else:
            saved = {"format_version": 1, "run_id": run_id, "source_fingerprint": source.hexdigest(),
                     "cases": [{"sha256": c["sha256"], "paths": c["paths"], **signature(c)} for c in cases]}
            atomic_write(args.baseline, json.dumps(saved, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    print(f'Report: {args.output / "index.html"}')
    regression = any(c["kind"] in {"regression", "removed"} for c in report["baseline_changes"])
    return int(broken or (args.check and regression))


if __name__ == "__main__":
    sys.exit(main())
