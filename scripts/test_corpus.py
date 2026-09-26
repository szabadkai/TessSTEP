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
        result = self.inspect_payload({"format_version": 1, "scope": "physical-syntax", "diagnostics": [{"severity":"error", "code":"TS1103"}],
                                       "document": {"entity_count": 1, "headers": {}, "unresolved_references": ["#2"]}}, 1)
        self.assertEqual(result["status"], "reference_errors")
        self.assertEqual(result["stages"]["physical_parse"], "accepted")
        self.assertEqual(result["stages"]["tessellation"], "not_integrated")

    def test_rejection_is_distinct_from_broken_protocol_or_crash(self):
        self.assertEqual(self.inspect_payload({"format_version": 1, "scope": "physical-syntax", "diagnostics": [{"severity":"error", "code":"TS1002"}], "document": None}, 1)["status"], "rejected")
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

    def test_schema_stage_is_independent_and_protocol_checked(self):
        for status, code in [("accepted", 0), ("rejected", 1), ("unsupported", 1), ("not_configured", 1), ("resource_limit", 1)]:
            result = case()
            result.update(seconds=0, entity_count=2)
            payload = {"format_version": 1, "scope": "schema-structure", "status": status, "entity_count": 2}
            def run(*args, **kwargs):
                kwargs["stdout"].write(json.dumps(payload).encode())
                return subprocess.CompletedProcess(args, code)
            with patch.object(corpus.subprocess, "run", side_effect=run):
                corpus.inspect_schema(Path("validator"), "TEST", Path("file.step"), 1, result)
            self.assertEqual(result["status"], "clean")
            self.assertEqual(result["stages"]["schema"], status)
        payload["status"] = "accepted"  # nonzero exit must not claim acceptance
        with patch.object(corpus.subprocess, "run", side_effect=run):
            corpus.inspect_schema(Path("validator"), "TEST", Path("file.step"), 1, result)
        self.assertEqual(result["status"], "runner_error")

    def test_schema_stage_skips_rejected_inputs_and_reports_timeout(self):
        result = case("rejected")
        with patch.object(corpus.subprocess, "run") as run:
            corpus.inspect_schema(Path("validator"), "TEST", Path("file.step"), 1, result)
            run.assert_not_called()
        self.assertEqual(result["stages"]["schema"], "not_run")
        result = case()
        result["seconds"] = 0
        with patch.object(corpus.subprocess, "run", side_effect=subprocess.TimeoutExpired("validator", 1)):
            corpus.inspect_schema(Path("validator"), "TEST", Path("file.step"), 1, result)
        self.assertEqual(result["status"], "timeout")

    def test_schema_regression_does_not_require_physical_regression(self):
        old = case()
        old["stages"]["schema"] = "accepted"
        current = case()
        current["stages"]["schema"] = "rejected"
        self.assertEqual(corpus.compare([current], {"cases": [old]})[0]["kind"], "regression")

    def test_product_stage_requires_schema_success_and_checks_protocol(self):
        for schema in ("not_implemented", "rejected", "unsupported"):
            result = case()
            result["stages"]["schema"] = schema
            with patch.object(corpus.subprocess, "run") as run:
                corpus.inspect_schema(Path("checker"), "TEST", Path("file.step"), 1, result, stage="product")
                run.assert_not_called()
            self.assertEqual(result["stages"]["product"], "not_run")
        for scope, count, code, expected in [("product-structure", 2, 0, "accepted"),
                ("schema-structure", 2, 0, "runner_error"), ("product-structure", 3, 0, "runner_error"),
                ("product-structure", 2, 1, "runner_error")]:
            result = case()
            result.update(seconds=0, entity_count=2)
            result["stages"]["schema"] = "accepted"
            def run(*args, **kwargs):
                kwargs["stdout"].write(json.dumps({"format_version": 1, "scope": scope,
                    "status": "accepted", "entity_count": count}).encode())
                return subprocess.CompletedProcess(args, code)
            with patch.object(corpus.subprocess, "run", side_effect=run):
                corpus.inspect_schema(Path("checker"), "TEST", Path("file.step"), 1, result, stage="product")
            self.assertEqual(result["stages"]["product"], expected)

    def test_product_stage_baseline_migration_and_regressions(self):
        old = case()
        current = case()
        current["stages"]["product"] = "not_implemented"
        self.assertEqual(corpus.compare([current], {"cases": [old]}), [])
        self.assertNotIn("product", old["stages"])
        old["stages"]["product"] = "accepted"
        current["stages"]["product"] = "rejected"
        self.assertEqual(corpus.compare([current], {"cases": [old]})[0]["kind"], "regression")

    def survey(self, roots, code=0, **overrides):
        payload = {"format_version": 1, "scope": "solid-survey", "metres_per_unit": 0.001, "parse": "accepted",
                   "root_count": len(roots), "truncated": False, "roots": roots, **overrides}
        result = case()
        result["seconds"] = 0
        result["stages"].update(geometry="not_integrated", tessellation="not_integrated")
        def run(*args, **kwargs):
            kwargs["stdout"].write(json.dumps(payload).encode())
            return subprocess.CompletedProcess(args, code)
        with patch.object(corpus.subprocess, "run", side_effect=run):
            corpus.inspect_geometry(Path("survey"), Path("file.step"), 1, result, 0.001)
        return result

    @staticmethod
    def root(status="accepted", stage=None, kind=None, message="", entity_type=""):
        stages = {"profile": "accepted", "geometry": "accepted", "tessellation": "accepted"}
        if status != "accepted":
            order = ["profile", "geometry", "tessellation"]
            failed = "geometry" if stage == "topology" else stage
            for s in order[order.index(failed):]:
                stages[s] = status if s == failed else "not_run"
            return {"id": 1, "status": status, "stages": stages, "failed_stage": stage, "kind": kind,
                    "entity": 7, "entity_type": entity_type, "message": message, "seconds": 0.1}
        return {"id": 1, "status": "accepted", "stages": stages, "vertices": 8, "triangles": 12, "seconds": 0.1}

    def test_geometry_stage_aggregates_every_root(self):
        circle = self.root("unsupported", "profile", "unsupported", "CIRCLE is outside the tessstep_planar import profile", "CIRCLE")
        open_shell = self.root("rejected", "topology", "invalid_geometry", "OpenShell at shell 12")
        result = self.survey([self.root(), circle, circle, open_shell])
        self.assertEqual(result["status"], "clean")
        self.assertEqual((result["stages"]["geometry"], result["stages"]["tessellation"]), ("partial", "partial"))
        summary = result["geometry_result"]
        self.assertEqual((summary["root_count"], summary["geometry_roots"], summary["accepted_roots"]), (4, 1, 1))
        self.assertEqual(summary["outcomes"], {"accepted": 1, "profile:unsupported": 2, "topology:invalid_geometry": 1})
        self.assertEqual(summary["categories"]["OpenShell at shell N"], 1)
        self.assertEqual(summary["unsupported_entity_types"], {"CIRCLE": 2})
        self.assertNotIn("seconds", summary["roots"][0])
        self.assertEqual(self.survey([circle, circle, open_shell])["stages"]["geometry"], "unsupported")
        self.assertEqual(self.survey([circle])["stages"]["tessellation"], "not_run")
        self.assertEqual(self.survey([self.root()])["stages"]["geometry"], "accepted")
        empty = self.survey([])
        self.assertEqual((empty["stages"]["geometry"], empty["stages"]["tessellation"]), ("no_solid_roots", "not_run"))

    def test_geometry_protocol_disagreements_are_runner_errors(self):
        for overrides, code in [({"scope": "planar-solid"}, 0), ({"parse": "rejected"}, 0),
                                ({"root_count": 5}, 0), ({"truncated": True}, 0), ({}, 2)]:
            self.assertEqual(self.survey([self.root()], code, **overrides)["status"], "runner_error", overrides)
        self.assertEqual(self.survey([self.root()], 101)["status"], "crash")
        self.assertEqual(self.survey([dict(self.root(), status="healed")])["status"], "runner_error")
        truncated = self.survey([self.root()], root_count=3, truncated=True)
        self.assertEqual((truncated["status"], truncated["geometry_result"]["surveyed_roots"]), ("clean", 1))

    def test_geometry_stage_skips_physical_rejection_and_records_timeouts(self):
        result = case("rejected")
        with patch.object(corpus.subprocess, "run") as run:
            corpus.inspect_geometry(Path("survey"), Path("file.step"), 1, result, 0.001)
            run.assert_not_called()
        self.assertEqual(result["stages"]["geometry"], "not_run")
        result = case()
        result["seconds"] = 0
        with patch.object(corpus.subprocess, "run", side_effect=subprocess.TimeoutExpired("survey", 1)):
            corpus.inspect_geometry(Path("survey"), Path("file.step"), 1, result, 0.001)
        self.assertEqual((result["status"], result["stages"]["geometry"]), ("timeout", "timeout"))

    def test_geometry_baseline_migration_and_root_regressions(self):
        old = case()
        old["stages"].update(geometry="not_integrated", tessellation="not_integrated")
        current = self.survey([self.root()])
        self.assertEqual(corpus.compare([current], {"cases": [old]})[0]["kind"], "changed")
        unmeasured = case()
        unmeasured["stages"].update(geometry="not_integrated", tessellation="not_integrated")
        self.assertEqual(corpus.compare([unmeasured], {"cases": [old]}), [])
        # A saved baseline keeps the accepted-root count without the full summary.
        saved = {"sha256": "abc", "paths": ["sample.step"], **corpus.signature(self.survey([self.root(), self.root()]))}
        worse = self.survey([self.root(), self.root("rejected", "tessellation", "invalid_geometry", "bad")])
        self.assertEqual(corpus.compare([worse], {"cases": [saved]})[0]["kind"], "regression")
        better = self.survey([self.root(), self.root(), self.root()])
        self.assertEqual(corpus.compare([better], {"cases": [saved]})[0]["kind"], "changed")


if __name__ == "__main__":
    unittest.main()
