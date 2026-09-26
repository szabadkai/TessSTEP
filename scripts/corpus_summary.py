#!/usr/bin/env python3
"""Publish stage-aware corpus Markdown, CSV and regression/expectation JUnit."""
import argparse
from collections import Counter
import csv
import html
import io
import json
import os
from pathlib import Path
import xml.etree.ElementTree as ET

from corpus_report import aggregate, stages_of, source_of, verdict, FAULTS, STAGES


def markdown(report):
    summary = report["summary"]
    counts = summary["unique_statuses"]
    changes = report["baseline_changes"]
    stats = aggregate(report["cases"], report.get("baseline_present", True))
    lines = [f'## {report.get("title", "External STEP corpus")}', "",
             f'**{summary["unique_inputs"]:,} unique inputs** across {summary["files"]:,} paths; {summary["seconds"]:.1f}s.', "",
             "| Physical observation | Unique inputs |", "| --- | ---: |"]
    lines += [f"| {status} | {counts.get(status, 0):,} |" for status in ("clean", "reference_errors", "rejected", "crash", "timeout", "runner_error", "nondeterministic")]
    lines += ["", "| Check verdict | Inputs |", "| --- | ---: |"]
    lines += [f"| {status} | {count} |" for status,count in stats["verdicts"].items()]
    lines += ["", "| Stage | Executed / total | Outcomes |", "| --- | ---: | --- |"]
    lines += [f'| {stage} | {s["tested"]} / {s["total"]} | ' + ', '.join(f'{k}: {v}' for k,v in s['statuses'].items()) + ' |' for stage,s in stats['stages'].items()]
    g = stats["geometry"]
    if g["files_surveyed"]:
        cell = lambda text: html.escape(str(text)).replace("|", "&#124;").replace("\n", " ")
        lines += ["", f'**Solid import:** {g["root_count"]:,} roots in {g["files_with_roots"]:,} files; '
                  f'{g["geometry_roots"]:,} passed geometry/topology; {g["accepted_roots"]:,} produced solid meshes.', "",
                  "| First failing stage : kind | Roots |", "| --- | ---: |"]
        lines += [f"| {cell(k)} | {v:,} |" for k, v in g["outcomes"].items()]
        lines += ["", "| Top failure categories | Roots |", "| --- | ---: |"]
        lines += [f"| {cell(k)} | {v:,} |" for k, v in list(g["categories"].items())[:10]]
    lines += ["", "| Source | Unique | Clean | Reference errors | Rejected | Failed checks |", "| --- | ---: | ---: | ---: | ---: | ---: |"]
    lines += [f'| {html.escape(name).replace(chr(124), "&#124;").replace(chr(10), " ")} | {s["unique_inputs"]} | {s["statuses"].get("clean",0)} | {s["statuses"].get("reference_errors",0)} | {s["statuses"].get("rejected",0)} | {s["verdicts"].get("failed",0)} |' for name,s in stats['sources'].items()]
    lines += ["", f'Baseline changes: `{dict(Counter(c["kind"] for c in changes))}`.', "",
              "Physical acceptance is not CAD conformance; a reviewed adversarial rejection can pass.",
              "Unconfigured/skipped stages are not passes. Geometry and tessellation import every solid root with bounded selected-root profiles at an assumed unit scale; profile rejection is not an AP validity verdict.",
              "See the separate kernel test report for constructed geometry, mesh and C ABI test evidence.", "",
              "Artifacts contain searchable HTML, JSON, Markdown, JUnit and CSV; no external CAD inputs.",
              f'Executable SHA-256: `{report["binary_sha256"]}`', ""]
    if changes:
        lines += ["### Changed inputs (first 30)", ""]
        for change in changes[:30]:
            name = html.escape(change["path"]).replace("`", "&#96;").replace("\n", " ")
            lines.append(f'- **{change["kind"]}**: `{name}`; fields: {", ".join(change.get("details", {})) or change["kind"]}')
    failed = [c for c in report['cases'] if c.get('expectation_failures')]
    if failed:
        lines += ["", "### Expectation failures (first 20)"]
        for case in failed[:20]:
            lines.append('- ' + html.escape('; '.join(case['expectation_failures'])).replace('\n', ' '))
    return "\n".join(lines) + "\n"


def xml_safe(text):
    # XML 1.0 forbids control bytes that may occur in STEP diagnostics/filenames.
    return ''.join(c if c in '\t\n\r' or 0x20 <= ord(c) <= 0xD7FF or 0xE000 <= ord(c) <= 0xFFFD or 0x10000 <= ord(c) <= 0x10FFFF else '\ufffd' for c in text)


def junit(report):
    regressions = {c["sha256"] for c in report["baseline_changes"] if c["kind"] == "regression"}
    suite = ET.Element("testsuite", name=report.get("title", "Corpus compatibility"), time=str(report["summary"]["seconds"]))
    failures = skipped = 0
    for case in report["cases"]:
        test = ET.SubElement(suite, "testcase", name=xml_safe(case["paths"][0]), classname="corpus." + xml_safe(source_of(case["paths"][0])), time=str(case["seconds"]))
        if case["sha256"] in regressions or case["status"] in FAULTS or case.get("expectation_failures"):
            ET.SubElement(test, "failure", message="Compatibility or expectation failure").text = xml_safe(json.dumps({k:case.get(k) for k in ('status','diagnostics','schema_result','product_result','expectation_failures','determinism')},ensure_ascii=True))
            failures += 1
        elif verdict(case, report.get("baseline_present", True)) == "unreviewed":
            ET.SubElement(test, "skipped", message="Observed only: no reviewed expectation or baseline")
            skipped += 1
        ET.SubElement(test, "system-out").text = xml_safe(f'Observed status: {case["status"]}; SHA-256: {case["sha256"]}; stages: {stages_of(case)}')
    for change in report["baseline_changes"]:
        if change["kind"] == "removed":
            test = ET.SubElement(suite, "testcase", name=xml_safe(change["path"]), classname="corpus.missing")
            ET.SubElement(test, "failure", message="Baseline input missing")
            failures += 1
    suite.set("tests", str(len(suite)))
    suite.set("failures", str(failures))
    suite.set("skipped", str(skipped))
    return ET.tostring(suite, encoding="unicode", xml_declaration=True) + "\n"


def csv_text(report):
    out = io.StringIO(newline='')
    writer = csv.writer(out)
    writer.writerow(['path','sha256','source','verdict','physical_outcome',*STAGES,'entities','seconds','diagnostic_codes'])
    for case in report['cases']:
        for path in case['paths']:
            row = [path,case['sha256'],source_of(path),verdict(case, report.get('baseline_present',False)),case['status'],
                   *(stages_of(case)[s] for s in STAGES),case.get('entity_count',''),case['seconds'],','.join(case.get('diagnostic_counts',{}))]
            writer.writerow(["'"+str(v) if str(v).startswith(('=','+','-','@','\t','\r')) else v for v in row])
    return out.getvalue()


def write_outputs(report, directory):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    for name,text in [('junit.xml',junit(report)),('summary.md',markdown(report)),('cases.csv',csv_text(report))]:
        (directory/name).write_text(text, encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, action="append", help="Repeat for multiple corpus suites")
    args = parser.parse_args()
    for path in args.report or [Path("reports/corpus/latest.json")]:
        if not path.exists():
            message = f"## {path.parent.name} corpus report\n\n**Run incomplete.** Inspect logs; no complete result is available.\n"
        else:
            report = json.loads(path.read_text(encoding="utf-8"))
            message = markdown(report)
            write_outputs(report, path.parent)
        print(message)
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as stream:
                stream.write(message)


if __name__ == "__main__":
    main()
