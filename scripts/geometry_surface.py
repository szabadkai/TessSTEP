"""Surface-model cases for check_geometry.py (Milestone 22.6): the authored
surface-*.step fixtures through the surface_model example, their surface-model class
in the shape survey, and the hash-pinned exporter files in
corpus/geometry/external-surface.json. Every shell is meshed as a shell, never as a
solid: open shells keep boundary edges, closed shells must be watertight but need no
positive volume.
"""
import hashlib
import json
import math
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CHORD = 1e-5
STAGES = ("profile", "geometry", "tessellation")
# (fixture, root) -> stages, shells as (closed, faces, boundary edges), and the
# closed-form total area (open model) or signed volume per shell (closed model) with
# the smallest curvature radius, checked against twice the chord over that radius.
EXPECTED = {
    ("surface-open-patches.step", 1000): (("accepted",) * 3, [(False, 3, 8)], ("area", (200 * math.pi + 200) * 1e-6, 0.01)),
    ("surface-open-patches.step", 1001): (("accepted",) * 3, [(False, 3, 8)], ("area", (200 * math.pi + 200) * 1e-6, 0.01)),
    ("surface-closed-shells.step", 1000): (("accepted",) * 3, [(True, 3, 0), (True, 6, 0)],
                                           ("volumes", [2000 * math.pi * 1e-9, -1e-6], 0.01)),
    ("surface-closed-shells.step", 1001): (("accepted", "rejected", "not_run"), [], None),
    ("surface-closed-shells.step", 1002): (("unsupported", "not_run", "not_run"), [], None),
}
# The survey's surface-model class over each fixture: root -> status. The fixtures
# have no solid or tessellation roots.
SURVEY_EXPECTED = {
    "surface-open-patches.step": {1000: "accepted", 1001: "accepted"},
    "surface-closed-shells.step": {1000: "accepted", 1001: "rejected", 1002: "unsupported"},
}


def shells_match(result, shells):
    found = [(s["closed"], s["faces"], s["boundary_edges"]) for s in result.get("shells", [])]
    return found == shells and all(s["status"] == "accepted" and s["components"] == 1 and s["watertight"] == s["closed"]
                                   for s in result.get("shells", []) if result["status"] == "accepted")


def measure_matches(result, measure):
    kind, exact, radius = measure
    bound = 2 * CHORD / radius
    if kind == "area":
        area = sum(s["area_m2"] for s in result["shells"])
        return abs(area - exact) <= bound * exact
    return all(abs(s["signed_volume_m3"] - v) <= bound * abs(v) for s, v in zip(result["shells"], exact))


def cases(inspect, surface_binary, survey_binary, external, require_external):
    out = []
    for (name, root), (stages, shells, measure) in EXPECTED.items():
        case = inspect(surface_binary, ROOT / "corpus/geometry" / name, root, scope="surface-model", extra=["--chord", str(CHORD)])
        r = case["result"]
        case["expected"] = {"root": root, "stages": stages, "shells": shells, "measure": measure}
        case["passed"] = tuple(r[k] for k in STAGES) == stages and shells_match(r, shells)
        if measure is not None:
            case["passed"] &= measure_matches(r, measure)
        out.append(case)
    for name, expected in SURVEY_EXPECTED.items():
        path = ROOT / "corpus/geometry" / name
        p = subprocess.run([str(survey_binary), str(path), "0.001"], capture_output=True, text=True, timeout=120, cwd=ROOT, check=True)
        survey = json.loads(p.stdout)
        found = {r["id"]: r["status"] for r in survey["surface_models"]["roots"]}
        passed = found == expected and survey["root_count"] == 0 and survey["surface_models"]["root_count"] == len(expected)
        out.append({"path": str(path), "expected": {"survey": {str(k): v for k, v in expected.items()}},
                    "result": {"status": "accepted" if passed else "rejected", "profile": "survey", "geometry": "survey",
                               "tessellation": "survey", "surface_models": survey["surface_models"]["roots"]},
                    "diagnostic": p.stderr, "passed": passed})
    # Unmodified exporter surface models; hashes pin the reviewed inputs.
    for spec in json.loads((ROOT / "corpus/geometry/external-surface.json").read_text()):
        path = external / spec["path"]
        if require_external and not path.is_file():
            raise RuntimeError(f"Required exporter fixture unavailable: {path}")
        if not path.is_file():
            continue
        assert hashlib.sha256(path.read_bytes()).hexdigest() == spec["sha256"], "Review changed exporter fixture before updating its hash"
        extra = ["--model-tolerance", str(spec["model_tolerance_m"]), "--chord", str(spec["chord_m"]),
                 "--normal-angle", str(spec["normal_angle_rad"])] + (["--degrees"] if spec["degrees"] else [])
        case = inspect(surface_binary, path, spec["root"], spec["metres_per_unit"], "surface-model", extra=extra)
        r = case["result"]
        shells = r.get("shells", [])
        area = sum(s.get("area_m2", 0) for s in shells)
        volume = sum(s.get("signed_volume_m3", 0) for s in shells if s["closed"])
        case["expected"] = spec
        case["passed"] = (r["status"] == "accepted" and len(shells) == spec["shells"]
                          and sum(s["closed"] for s in shells) == spec["closed_shells"]
                          and sum(s["faces"] for s in shells) == spec["faces"]
                          and sum(s["boundary_edges"] for s in shells) == spec["boundary_edges"]
                          and all(s["components"] == 1 and s["watertight"] == s["closed"] for s in shells)
                          and abs(area - spec["area_m2"]) <= spec["tolerance"] * spec["area_m2"]
                          and (spec["signed_volume_m3"] is None
                               or abs(volume - spec["signed_volume_m3"]) <= spec["tolerance"] * abs(spec["signed_volume_m3"])))
        out.append(case)
    return out
