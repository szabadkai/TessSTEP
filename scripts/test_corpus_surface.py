"""Tests for the surface-model root class of the corpus shape survey."""
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import corpus
import corpus_report
import corpus_summary
from test_corpus import case


def shell(closed=False, status="accepted", faces=3, boundary_edges=None):
    out = {"id": 5, "type": "CLOSED_SHELL" if closed else "OPEN_SHELL", "closed": closed, "faces": faces,
           "boundary_edges": 0 if closed else (8 if boundary_edges is None else boundary_edges), "status": status}
    if status == "accepted":
        out.update(vertices=10, triangles=12, mesh_boundary_edges=0 if closed else 10, watertight=closed)
    return out


def model(status="accepted", stage=None, kind=None, message="", entity_type="", shells=None):
    stages = {"profile": "accepted", "geometry": "accepted", "tessellation": "accepted"}
    common = {"id": 1000, "type": "SHELL_BASED_SURFACE_MODEL", "profile": "surface_model", "units": "context",
              "seconds": 0.1, "closure": {"OPEN_SHELL": 1, "ADVANCED_FACE": 3}}
    if status == "accepted":
        return {**common, "status": "accepted", "stages": stages, "vertices": 10, "triangles": 12,
                "shells": [shell()] if shells is None else shells}
    order = ["profile", "geometry", "tessellation"]
    failed = "geometry" if stage == "topology" else stage
    for s in order[order.index(failed):]:
        stages[s] = status if s == failed else "not_run"
    if shells is None:
        shells = [shell(status=status)] if stage == "tessellation" else []
    return {**common, "status": status, "stages": stages, "failed_stage": stage, "kind": kind, "entity": 7,
            "entity_type": entity_type, "message": message, "shells": shells}


def survey(models, code=0, solids=(), **section):
    payload = {"format_version": 2, "scope": "shape-survey", "metres_per_unit": 0.001, "parse": "accepted",
               "root_count": len(solids), "truncated": False, "roots": list(solids),
               "presentation": {"occurrence_count": 0, "surveyed": 0, "truncated": False, "with_context_units": 0, "accepted": 0,
                                "polylines": 0, "triangles": 0, "points": 0, "zero_area_triangles": 0, "outcomes": {}, "failures": []}}
    if models is not None:
        payload["surface_models"] = {"root_count": len(models), "truncated": False, "roots": models, **section}
    result = case()
    result["seconds"] = 0
    result["stages"].update(geometry="not_integrated", tessellation="not_integrated")

    def run(*args, **kwargs):
        kwargs["stdout"].write(json.dumps(payload).encode())
        return subprocess.CompletedProcess(args, code)
    with patch.object(corpus.subprocess, "run", side_effect=run):
        corpus.inspect_geometry(Path("survey"), Path("file.step"), 1, result, 0.001)
    return result


class SurfaceModelTests(unittest.TestCase):
    def test_surface_models_are_a_separate_root_class(self):
        open_shell = model("rejected", "topology", "invalid_geometry", "OpenShell at shell 0")
        vertex = model("unsupported", "profile", "unsupported", "VERTEX_SHELL is outside the tessstep_brep import profile", "VERTEX_SHELL")
        trim = model("rejected", "tessellation", "invalid_geometry", "tessellation Some(FaceId(2)): UnresolvedTolerance",
                     shells=[shell(), shell(True, "rejected")])
        result = survey([model(), model(shells=[shell(True), shell()]), open_shell, vertex, trim])
        self.assertEqual(result["status"], "clean")
        # The solid stages keep their meaning: this file has no solid or tessellation roots.
        self.assertEqual((result["stages"]["geometry"], result["stages"]["tessellation"]), ("no_shape_roots", "not_run"))
        self.assertEqual(result["stages"]["surface_model"], "partial")
        summary = result["surface_model_result"]
        self.assertEqual((summary["root_count"], summary["geometry_roots"], summary["accepted_roots"]), (5, 3, 2))
        self.assertEqual((summary["shells"], summary["open_shells"], summary["meshed_open_shells"]), (5, 3, 3))
        self.assertEqual((summary["closed_shells"], summary["watertight_closed_shells"], summary["open_boundary_edges"]), (2, 1, 24))
        self.assertEqual(summary["outcomes"], {"accepted": 2, "profile:unsupported": 1, "tessellation:invalid_geometry": 1,
                                               "topology:invalid_geometry": 1})
        self.assertEqual(summary["categories"]["OpenShell at shell N"], 1)
        self.assertEqual(summary["unsupported_entity_types"], {"VERTEX_SHELL": 1})
        self.assertEqual(summary["closure_failing_roots"], {"ADVANCED_FACE": 3, "OPEN_SHELL": 3})
        self.assertNotIn("closure", summary["roots"][0])
        self.assertEqual(survey([vertex, vertex, open_shell])["stages"]["surface_model"], "unsupported")
        self.assertEqual(survey([model()])["stages"]["surface_model"], "accepted")
        self.assertEqual(survey([])["stages"]["surface_model"], "no_surface_models")
        self.assertEqual(survey(None)["stages"]["surface_model"], "not_run")
        self.assertNotIn("surface_model_result", survey(None))

    def test_surface_model_protocol_disagreements_are_runner_errors(self):
        accepted_with_failed_shell = dict(model(), shells=[shell(status="rejected")])
        closed_with_boundary = dict(model(), shells=[dict(shell(True), boundary_edges=2)])
        open_closed = dict(model(), shells=[dict(shell(True), watertight=False)])
        early_failure_with_shells = dict(model("rejected", "geometry", "invalid_geometry", "bad"), shells=[shell(status="rejected")])
        for models, section in [([dict(model(), profile="brep")], {}), ([dict(model(), status="healed")], {}),
                                ([accepted_with_failed_shell], {}), ([closed_with_boundary], {}), ([open_closed], {}),
                                ([early_failure_with_shells], {}), ([dict(model(), shells=[])], {}),
                                ([model()], {"root_count": 3}), ([model()], {"truncated": True}), ([model()], {"extra": 1})]:
            result = survey(models, **section)
            self.assertEqual(result["status"], "runner_error", (models, section))
            self.assertEqual(result["stages"]["surface_model"], "runner_error")
        truncated = survey([model()], root_count=4, truncated=True)
        self.assertEqual((truncated["status"], truncated["surface_model_result"]["surveyed_roots"]), ("clean", 1))

    def test_baseline_gains_the_class_without_changing_other_files(self):
        old = case()
        old["stages"].update(geometry="no_shape_roots", tessellation="not_run")
        old.update(geometry_accepted_roots=0, geometry_closure_types={}, presentation_accepted=None)
        # A file without surface models compares equal to a baseline saved before the class.
        self.assertEqual(corpus.compare([survey([])], {"cases": [old]}), [])
        self.assertEqual(corpus.compare([survey(None)], {"cases": [old]}), [])
        # A file with them changes only by gaining the class.
        changed = corpus.compare([survey([model()])], {"cases": [old]})
        self.assertEqual(changed[0]["kind"], "changed")
        self.assertEqual(set(changed[0]["details"]), {"stages", "surface_model_accepted_roots", "surface_model_closure_types"})
        # A saved baseline keeps the accepted-root count; fewer meshed roots or a lost
        # accepted stage is a regression, more is a change.
        current = survey([model(), model()])
        saved = {"sha256": "abc", "paths": ["sample.step"], **corpus.signature(current), "stages": current["stages"]}
        self.assertEqual(saved["surface_model_accepted_roots"], 2)
        self.assertEqual(corpus.compare([survey([model(), model()])], {"cases": [saved]}), [])
        worse = survey([model(), model("rejected", "tessellation", "invalid_geometry", "bad")])
        self.assertEqual(corpus.compare([worse], {"cases": [saved]})[0]["kind"], "regression")
        better = survey([model(), model(), model()])
        self.assertEqual(corpus.compare([better], {"cases": [saved]})[0]["kind"], "changed")

    def test_summary_and_report_show_the_class(self):
        a = survey([model(), model("rejected", "topology", "invalid_geometry", "OpenShell at shell 0")])
        a["paths"] = ["vendor/opencascade-dataset/step/a.stp"]
        b = survey([])
        b.update(sha256="def", paths=["vendor/other/b.stp"])
        stats = corpus_report.aggregate([a, b])
        totals = stats["surface_models"]
        self.assertEqual((totals["files_with_roots"], totals["root_count"], totals["accepted_roots"]), (1, 2, 1))
        self.assertEqual(totals["sources"], {"opencascade-dataset": {"files": 1, "roots": 2, "accepted_roots": 1}})
        self.assertEqual(stats["stages"]["surface_model"]["statuses"], {"no_surface_models": 1, "partial": 1})
        report = {"summary": {"unique_inputs": 2, "files": 2, "seconds": 1, "unique_statuses": {"clean": 2}}, "cases": [a, b],
                  "baseline_changes": [], "baseline_present": False, "binary_sha256": "x", "run_id": "r", "source_fingerprint": "f" * 16,
                  "corpus_root": "/c"}
        text = corpus_summary.markdown(report)
        self.assertIn("**Surface models (shells, never solids):** 2 SHELL_BASED_SURFACE_MODEL roots in 1 files", text)
        self.assertIn("| topology:invalid_geometry | 1 |", text)
        self.assertIn("surface_model", corpus_summary.csv_text(report).splitlines()[0])
        self.assertIn("Surface model outcomes", corpus_report.render(report))


if __name__ == "__main__":
    unittest.main()
