"""Tests for the opt-in repair evidence of the corpus geometry survey."""
import unittest

import corpus
from corpus_repair import markdown_lines, summarize_repairs


def root(status="accepted", **repairs):
    counts = dict.fromkeys(("merged_vertices", "merged_edges", "disconnected_shells", "flipped_faces",
                            "reversed_loops", "reoriented_shells", "undecided_shells"), 0)
    counts.update(repairs)
    counts.setdefault("max_merge_distance_m", 0.0)
    stages = {"profile": "accepted", "geometry": "accepted", "tessellation": "accepted" if status == "accepted" else "rejected"}
    common = {"id": 1, "type": "MANIFOLD_SOLID_BREP", "profile": "brep", "selection": "selected", "paired_with": [],
              "status": status, "stages": stages, "repairs": counts}
    if status != "accepted":
        common.update({"failed_stage": "tessellation", "kind": "invalid_geometry", "entity": 1, "entity_type": "", "message": "x"})
    return common


def payload(roots, repair=True):
    presentation = {"occurrence_count": 0, "surveyed": 0, "truncated": False, "with_context_units": 0, "accepted": 0,
                    "polylines": 0, "triangles": 0, "points": 0, "zero_area_triangles": 0, "outcomes": {}, "failures": []}
    out = {"format_version": 2, "scope": "shape-survey", "metres_per_unit": 0.001, "parse": "accepted",
           "root_count": len(roots), "truncated": False, "roots": roots, "presentation": presentation}
    if repair:
        out["repair"] = True
    return out


class RepairSummaryTests(unittest.TestCase):
    def test_counts_only_roots_whose_brep_a_repair_changed(self):
        roots = [root(merged_vertices=16, merged_edges=12, disconnected_shells=1, max_merge_distance_m=5e-8),
                 root("rejected", flipped_faces=1), root(undecided_shells=1), root()]
        summary = summarize_repairs(roots)
        self.assertEqual((summary["repaired_roots"], summary["repaired_meshed"]), (2, 1))
        self.assertEqual((summary["merged_vertices"], summary["merged_edges"], summary["flipped_faces"]), (16, 12, 1))
        self.assertEqual((summary["disconnected_shells"], summary["undecided_shells"]), (1, 1))
        self.assertEqual(summary["max_merge_distance_m"], 5e-8)

    def test_strict_surveys_keep_their_report_shape(self):
        strict = [{k: v for k, v in r.items() if k != "repairs"} for r in [root()]]
        self.assertNotIn("repairs", corpus.summarize_survey(payload(strict, repair=False)))
        self.assertIn("repairs", corpus.summarize_survey(payload([root(reversed_loops=1)])))
        self.assertEqual(markdown_lines([{"paths": ["a.step"], "geometry_result": corpus.summarize_survey(payload(strict, repair=False))}]), [])

    def test_markdown_lists_repaired_roots_per_source(self):
        cases = [{"paths": ["vendor/exporter/a.step"], "geometry_result": corpus.summarize_survey(payload([root(reoriented_shells=1)]))},
                 {"paths": ["vendor/exporter/b.step"], "geometry_result": corpus.summarize_survey(payload([root()]))}]
        text = "\n".join(markdown_lines(cases))
        self.assertIn("1 roots repaired in 1 files, 1 of them meshed", text)
        self.assertIn("| exporter | 1 | 1 | 1 |", text)


if __name__ == "__main__":
    unittest.main()
