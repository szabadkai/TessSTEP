#!/usr/bin/env python3
"""Publish corpus observations as Actions Markdown and regression JUnit XML."""
import argparse
from collections import Counter
import html
import json
import os
from pathlib import Path
import xml.etree.ElementTree as ET


def markdown(report):
    summary = report["summary"]
    counts = summary["unique_statuses"]
    changes = report["baseline_changes"]
    lines = ["## External STEP corpus", "",
             f'**{summary["unique_inputs"]:,} unique inputs** across {summary["files"]:,} source paths; {summary["seconds"]:.1f} seconds.', "",
             "| Observed outcome | Unique inputs |", "| --- | ---: |"]
    lines += [f"| {status} | {counts.get(status, 0):,} |" for status in ("clean", "reference_errors", "rejected", "crash", "timeout", "runner_error")]
    lines += ["", f'Baseline changes: `{dict(Counter(c["kind"] for c in changes))}`.', "",
              "Parsing acceptance is not CAD conformance. Rejection can be correct for adversarial inputs.",
              "Schema structure is checked only with a configured validator. Geometry and tessellation: not implemented.",
              f"Schema stages: `{dict(Counter(c.get('stages', {}).get('schema', 'not_implemented') for c in report['cases']))}`.", "",
              "Download the `corpus-report` artifact for searchable `index.html`, complete JSON and JUnit results.",
              "Raw third-party CAD files are not included in artifacts or release packages.", "",
              f'Executable SHA-256: `{report["binary_sha256"]}`', ""]
    if changes:
        lines += ["### Changed inputs (first 50)", ""]
        for change in changes[:50]:
            name = html.escape(change["path"]).replace("`", "&#96;").replace("\n", " ")
            lines.append(f'- **{change["kind"]}**: `{name}` ({change.get("before", "—")} → {change.get("after", "—")})')
    return "\n".join(lines) + "\n"


def junit(report):
    regressions = {c["sha256"] for c in report["baseline_changes"] if c["kind"] == "regression"}
    suite = ET.Element("testsuite", name="External corpus compatibility", time=str(report["summary"]["seconds"]))
    failures = 0
    for case in report["cases"]:
        test = ET.SubElement(suite, "testcase", name=case["paths"][0], classname="corpus.physical", time=str(case["seconds"]))
        if case["sha256"] in regressions or case["status"] in {"crash", "timeout", "runner_error"}:
            ET.SubElement(test, "failure", message=case["status"]).text = json.dumps({"physical": case["diagnostics"], "schema": case.get("schema_result")})
            failures += 1
        ET.SubElement(test, "system-out").text = f'Observed status: {case["status"]}; SHA-256: {case["sha256"]}'
    for change in report["baseline_changes"]:
        if change["kind"] == "removed":
            test = ET.SubElement(suite, "testcase", name=change["path"], classname="corpus.missing")
            ET.SubElement(test, "failure", message="Baseline input missing")
            failures += 1
    suite.set("tests", str(len(suite)))
    suite.set("failures", str(failures))
    return ET.tostring(suite, encoding="unicode", xml_declaration=True) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, default=Path("reports/corpus/latest.json"))
    args = parser.parse_args()
    if not args.report.exists():
        message = "## External STEP corpus\n\n**Corpus run did not finish.** Inspect acquisition/build/test logs; no complete result is available.\n"
    else:
        report = json.loads(args.report.read_text(encoding="utf-8"))
        message = markdown(report)
        args.report.with_name("junit.xml").write_text(junit(report), encoding="utf-8")
        args.report.with_name("summary.md").write_text(message, encoding="utf-8")
    print(message)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as stream:
            stream.write(message)


if __name__ == "__main__":
    main()
