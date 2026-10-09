"""Opt-in topology repair evidence for the corpus geometry survey (Milestone 22.5).

The survey reports per-root repair counts only when it runs with `--repair`
(`corpus.py --geometry-repair`); the default corpus run is strict and its reports do
not change. Repair-on runs belong in a separate output directory.
"""
from collections import Counter

from corpus_report import source_of

COUNTS = ("merged_vertices", "merged_edges", "disconnected_shells", "flipped_faces", "reversed_loops", "reoriented_shells", "undecided_shells")
# Counts that change the B-rep; undecided shells and disconnected shells only describe it.
EDITS = ("merged_vertices", "merged_edges", "flipped_faces", "reversed_loops", "reoriented_shells")


def repaired(root):
    repairs = root.get("repairs") or {}
    return any(repairs.get(key) for key in EDITS)


def summarize_repairs(roots):
    """Per-file totals over the roots whose B-rep a repair changed."""
    edited = [r for r in roots if repaired(r)]
    totals = {key: sum((r.get("repairs") or {}).get(key, 0) for r in roots) for key in COUNTS}
    return {"repaired_roots": len(edited), "repaired_meshed": sum(r["status"] == "accepted" for r in edited),
            **totals, "max_merge_distance_m": max([(r.get("repairs") or {}).get("max_merge_distance_m", 0.0) for r in roots] or [0.0])}


def markdown_lines(cases):
    """Summary lines for a repair-on run; empty for strict runs."""
    results = [c for c in cases if (c.get("geometry_result") or {}).get("repairs") is not None]
    if not results:
        return []
    totals = Counter()
    distance = 0.0
    sources = {}
    for case in results:
        repairs = case["geometry_result"]["repairs"]
        totals.update({k: repairs[k] for k in ("repaired_roots", "repaired_meshed", *COUNTS)})
        distance = max(distance, repairs["max_merge_distance_m"])
        if repairs["repaired_roots"]:
            row = sources.setdefault(source_of(case["paths"][0]), Counter())
            row.update({"files": 1, "roots": repairs["repaired_roots"], "meshed": repairs["repaired_meshed"]})
    lines = ["", f'**Opt-in repair** (sewing and orientation at ten times the model tolerance): {totals["repaired_roots"]:,} roots '
             f'repaired in {sum(r["files"] for r in sources.values()):,} files, {totals["repaired_meshed"]:,} of them meshed; '
             f'{totals["merged_vertices"]:,} merged vertices and {totals["merged_edges"]:,} merged edges (largest distance '
             f'{distance:.3e} m, {totals["disconnected_shells"]:,} disconnected closed shells), {totals["flipped_faces"]:,} flipped '
             f'faces, {totals["reversed_loops"]:,} faces with reversed loops, {totals["reoriented_shells"]:,} reoriented and {totals["undecided_shells"]:,} undecided shells.', "",
             "| Source | Files | Repaired roots | Meshed |", "| --- | ---: | ---: | ---: |"]
    lines += [f'| {name} | {row["files"]:,} | {row["roots"]:,} | {row["meshed"]:,} |' for name, row in sorted(sources.items())]
    return lines
