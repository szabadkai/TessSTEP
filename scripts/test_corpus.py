"""Tests for corpus discovery, failure handling and regression semantics."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import corpus


def case(status="clean", key="abc"):
    accepted = status in {"clean", "reference_errors"}
    return {"sha256": key, "paths": ["sample.step"], "status": status,
            "stages": {"physical_parse": "accepted" if accepted else "rejected",
                       "references": "resolved" if status == "clean" else "not_run"},
            "diagnostic_counts": {}}


class CorpusTests(unittest.TestCase):
    def test_discovers_nested_vendor_duplicates_and_uppercase_extensions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "vendor/nested").mkdir(parents=True)
            (root / "vendor/nested/a.STP").write_bytes(b"identical")
            (root / "b.step").write_bytes(b"identical")
            (root / "c.p21").write_bytes(b"different")
            (root / "ignore.zip").write_bytes(b"archive")
            cases = corpus.discover(root)
            self.assertEqual(len(cases), 2)
            self.assertEqual(cases[0]["paths"], ["b.step", "vendor/nested/a.STP"])
            self.assertEqual(corpus.discover(root), cases)

    def test_baseline_detects_regression_and_removed_inputs(self):
        old = {"cases": [case(), case(key="removed")]}
        changes = corpus.compare([case("rejected"), case(key="new")], old)
        self.assertEqual([c["kind"] for c in changes], ["regression", "added", "removed"])

    def test_accepting_adversarial_input_is_only_an_observation(self):
        self.assertEqual(corpus.compare([case()], {"cases": [case("rejected") ]})[0]["kind"], "changed")
        self.assertEqual(corpus.compare([case()], {"cases": [case()]}), [])
        self.assertEqual(corpus.compare([case("timeout")], {"cases": [case("rejected") ]})[0]["kind"], "regression")

    def inspect_payload(self, payload, code):
        def run(*args, **kwargs):
            kwargs["stdout"].write(json.dumps(payload).encode())
            return subprocess.CompletedProcess(args, code)
        with patch.object(corpus.subprocess, "run", side_effect=run):
            return corpus.inspect(Path("stepdump"), Path("file.step"), 1)

    def test_reference_errors_do_not_erase_parse_success(self):
        result = self.inspect_payload({"format_version": 1, "scope": "physical-syntax", "diagnostics": [],
                                       "document": {"entity_count": 1, "headers": {}, "unresolved_references": ["#2"]}}, 1)
        self.assertEqual(result["status"], "reference_errors")
        self.assertEqual(result["stages"]["physical_parse"], "accepted")
        self.assertEqual(result["stages"]["tessellation"], "not_implemented")

    def test_rejection_is_distinct_from_broken_protocol_or_crash(self):
        self.assertEqual(self.inspect_payload({"format_version": 1, "scope": "physical-syntax", "diagnostics": [], "document": None}, 1)["status"], "rejected")
        self.assertEqual(self.inspect_payload({}, 0)["status"], "runner_error")
        self.assertEqual(self.inspect_payload({}, 101)["status"], "crash")
        self.assertEqual(self.inspect_payload({}, -9)["status"], "crash")

    def test_header_records_can_preserve_duplicate_names(self):
        result = self.inspect_payload({"format_version": 1, "scope": "physical-syntax", "diagnostics": [],
                                       "document": {"entity_count": 1, "headers": [{"name": "FILE_SCHEMA", "parameters": [["AP242"]]}], "unresolved_references": []}}, 0)
        self.assertEqual(result["status"], "clean")
        self.assertEqual(result["schemas"], [[["AP242"]]])

    def test_timeout_is_recorded_without_ending_suite(self):
        with patch.object(corpus.subprocess, "run", side_effect=subprocess.TimeoutExpired("stepdump", 1)):
            result = corpus.inspect(Path("stepdump"), Path("file.step"), 1)
        self.assertEqual(result["status"], "timeout")
        self.assertEqual(result["stages"]["physical_parse"], "not_run")


if __name__ == "__main__":
    unittest.main()
