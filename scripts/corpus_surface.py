"""Surface models as a separate root class of the corpus shape survey (Milestone 22.6).

The survey imports every SHELL_BASED_SURFACE_MODEL with the curved profile and meshes
each of its shells as a shell, never as a solid. These helpers check the survey's
`surface_models` section, bound it into a per-file `surface_model_result`, derive the
file-level `surface_model` stage and aggregate the class across the corpus. Solid and
tessellation roots, their stages and their baseline fields are unaffected.
"""
from collections import Counter
import re

ROOT_FAILURES = ("unsupported", "rejected", "resource_limit")
STATUSES = ("accepted", *ROOT_FAILURES)


def valid_shell(shell):
    return (isinstance(shell, dict) and isinstance(shell.get("closed"), bool) and shell.get("status") in STATUSES
            and isinstance(shell.get("faces"), int) and isinstance(shell.get("boundary_edges"), int)
            and (not shell["closed"] or shell["boundary_edges"] == 0)
            and (shell["status"] != "accepted" or (isinstance(shell.get("watertight"), bool)
                                                   and (not shell["closed"] or shell["watertight"]))))


def valid_root(root):
    """A root is accepted exactly when it has shells and every one of them meshed; a
    root that failed before tessellation reports no shells."""
    if not (isinstance(root, dict) and root.get("status") in STATUSES and root.get("profile") == "surface_model"
            and isinstance(root.get("stages"), dict) and set(root["stages"]) == {"profile", "geometry", "tessellation"}
            and isinstance(root.get("shells"), list) and all(valid_shell(s) for s in root["shells"])):
        return False
    meshed = bool(root["shells"]) and all(s["status"] == "accepted" for s in root["shells"])
    if root["status"] == "accepted":
        return meshed and all(v == "accepted" for v in root["stages"].values())
    return not meshed and (root["stages"]["tessellation"] in ROOT_FAILURES) == bool(root["shells"])


def valid(section):
    """The survey's `surface_models` section: consistent counts and valid roots."""
    if not isinstance(section, dict) or set(section) != {"root_count", "truncated", "roots"}:
        return False
    roots = section["roots"]
    return (isinstance(roots, list) and len(roots) == min(section["root_count"], len(roots))
            and section["truncated"] == (section["root_count"] > len(roots)) and all(valid_root(r) for r in roots))


def summarize(section):
    """Bound per-file evidence: counts and categories for every root and shell, details
    for the first 20 roots (and their first 10 shells)."""
    roots = section["roots"]
    failures = [r for r in roots if r["status"] != "accepted"]
    shells = [s for r in roots for s in r["shells"]]
    return {"root_count": section["root_count"], "surveyed_roots": len(roots), "truncated": section["truncated"],
            "geometry_roots": sum(r["stages"]["geometry"] == "accepted" for r in roots),
            "accepted_roots": sum(r["status"] == "accepted" for r in roots),
            "shells": len(shells), "open_shells": sum(not s["closed"] for s in shells),
            "meshed_open_shells": sum(not s["closed"] and s["status"] == "accepted" for s in shells),
            "closed_shells": sum(s["closed"] for s in shells),
            "watertight_closed_shells": sum(s["closed"] and s["status"] == "accepted" and s["watertight"] for s in shells),
            "faces": sum(s["faces"] for s in shells), "open_boundary_edges": sum(s["boundary_edges"] for s in shells),
            "triangles": sum(s.get("triangles", 0) for s in shells),
            "outcomes": dict(sorted(Counter("accepted" if r["status"] == "accepted" else f'{r["failed_stage"]}:{r["kind"]}' for r in roots).items())),
            # Entity IDs vary per file; digits are removed so categories aggregate.
            "categories": dict(Counter(re.sub(r"\d+", "N", r["message"]) for r in failures).most_common(10)),
            "unsupported_entity_types": dict(Counter(r["entity_type"] for r in failures if r["failed_stage"] == "profile" and r["kind"] == "unsupported").most_common(10)),
            "closure_types": dict(sorted(sum((Counter(r.get("closure", {})) for r in roots), Counter()).items())),
            "closure_roots": dict(sorted(Counter(t for r in roots for t in r.get("closure", {})).items())),
            "closure_failing_roots": dict(sorted(Counter(t for r in failures for t in r.get("closure", {})).items())),
            "closure_truncated": sum(bool(r.get("closure_truncated")) for r in roots),
            "roots": [{**{k: v for k, v in r.items() if k not in ("seconds", "closure", "shells")}, "shells": r["shells"][:10]}
                      for r in roots[:20]]}


def stage(roots):
    """All roots meshed, some meshed (partial), none present, or the dominant failure."""
    if not roots:
        return "no_surface_models"
    accepted = sum(r["status"] == "accepted" for r in roots)
    if accepted == len(roots):
        return "accepted"
    if accepted:
        return "partial"
    failed = Counter(r["status"] for r in roots)
    return max(sorted(failed), key=failed.get)


def totals(cases, source_of):
    """Root- and shell-level surface model outcomes across files; categories count roots."""
    results = [(c["paths"][0], c["surface_model_result"]) for c in cases if c.get("surface_model_result")]
    total = lambda key: sum(r.get(key, 0) for _, r in results)
    merged = lambda key: dict(sum((Counter(r.get(key, {})) for _, r in results), Counter()).most_common(15))
    sources = {}
    for path, r in results:
        if r["root_count"]:
            row = sources.setdefault(source_of(path), Counter())
            row.update({"files": 1, "roots": r["surveyed_roots"], "accepted_roots": r["accepted_roots"]})
    keys = ("root_count", "surveyed_roots", "geometry_roots", "accepted_roots", "shells", "open_shells", "meshed_open_shells",
            "closed_shells", "watertight_closed_shells", "faces", "open_boundary_edges", "triangles")
    return {"files_surveyed": len(results), "files_with_roots": sum(r["root_count"] > 0 for _, r in results),
            **{key: total(key) for key in keys},
            "outcomes": merged("outcomes"), "categories": merged("categories"),
            "unsupported_entity_types": merged("unsupported_entity_types"),
            "sources": {name: dict(row) for name, row in sorted(sources.items(), key=lambda kv: -kv[1]["roots"])}}


def markdown(stats, cell):
    """Summary lines for the surface model class, or none when no file has one."""
    s = stats.get("surface_models") or {}
    if not s.get("files_with_roots"):
        return []
    lines = ["", f'**Surface models (shells, never solids):** {s["root_count"]:,} SHELL_BASED_SURFACE_MODEL roots in '
             f'{s["files_with_roots"]:,} files; {s["geometry_roots"]:,} passed geometry/topology and {s["accepted_roots"]:,} '
             f'produced shell meshes. {s["meshed_open_shells"]:,} of {s["open_shells"]:,} open shells meshed '
             f'({s["open_boundary_edges"]:,} open boundary edges in surveyed shells); {s["watertight_closed_shells"]:,} of '
             f'{s["closed_shells"]:,} closed shells meshed watertight.', "",
             "| Source | Files | Roots | Meshed roots |", "| --- | ---: | ---: | ---: |"]
    lines += [f'| {cell(name)} | {row["files"]:,} | {row["roots"]:,} | {row["accepted_roots"]:,} |' for name, row in s["sources"].items()]
    lines += ["", "| Surface model first failing stage : kind | Roots |", "| --- | ---: |"]
    lines += [f"| {cell(k)} | {v:,} |" for k, v in s["outcomes"].items()]
    lines += ["", "| Top surface model failure categories | Roots |", "| --- | ---: |"]
    lines += [f"| {cell(k)} | {v:,} |" for k, v in list(s["categories"].items())[:10]]
    return lines
