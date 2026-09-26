#!/usr/bin/env python3
"""Verify the reduced faceted profile and real-file import stages, without AP claims."""
import argparse
import hashlib
import html
import json
import math
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {
    "box.step": ("accepted", "accepted", "accepted"),
    "tube.step": ("accepted", "accepted", "accepted"),
    "open-shell.step": ("accepted", "rejected", "not_run"),
    "nonplanar.step": ("accepted", "rejected", "not_run"),
    "duplicate-point.step": ("rejected", "not_run", "not_run"),
    "missing-point.step": ("rejected", "not_run", "not_run"),
    "curved.step": ("unsupported", "not_run", "not_run"),
}

PLANAR_EXPECTED = {
    "planar-box.step": ("accepted", "accepted", "accepted"),
    "planar-tube.step": ("accepted", "accepted", "accepted"),
    "planar-off-line.step": ("accepted", "rejected", "not_run"),
    "planar-sense.step": ("accepted", "rejected", "not_run"),
    "planar-placeholder.step": ("rejected", "not_run", "not_run"),
    "planar-open-shell.step": ("accepted", "rejected", "not_run"),
    "planar-curved.step": ("unsupported", "not_run", "not_run"),
}


def inspect(binary, path, root_id=1000, unit=0.001, scope="faceted-solid"):
    started = time.monotonic()
    p = subprocess.run([str(binary), str(path), str(root_id), str(unit)],
                       capture_output=True, text=True, timeout=30, cwd=ROOT)
    assert p.returncode in (0, 1), (path, p.returncode, p.stderr)
    result = json.loads(p.stdout)
    assert result["format_version"] == 1 and result["scope"] == scope
    assert (p.returncode == 0) == (result["status"] == "accepted")
    return {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "result": result, "diagnostic": p.stderr, "seconds": round(time.monotonic()-started, 4)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-external", action="store_true", help="Require the pinned unmodified exporter cuboid")
    args = parser.parse_args()
    subprocess.run(["cargo", "build", "--locked", "-p", "expressc", "-p", "tessstep-import", "--examples"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "-p", "expressc"], cwd=ROOT, check=True)
    meta = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version=1"], cwd=ROOT))
    target = Path(meta["target_directory"]) / "debug"
    suffix = ".exe" if os.name == "nt" else ""
    generated = subprocess.check_output([str(target / ("expressc"+suffix)), "--rust", "corpus/geometry/faceted.exp"], cwd=ROOT)
    assert generated == (ROOT / "crates/tessstep-import/src/profile.rs").read_bytes(), "Regenerate the faceted profile"
    planar_generated = subprocess.check_output([str(target / ("expressc"+suffix)), "--rust", "corpus/geometry/planar.exp"], cwd=ROOT)
    assert planar_generated == (ROOT / "crates/tessstep-import/src/planar_profile.rs").read_bytes(), "Regenerate the planar profile"
    binary = target / "examples" / ("faceted"+suffix)
    planar_binary = target / "examples" / ("planar"+suffix)
    cases = []
    for name, expected in {**EXPECTED, **PLANAR_EXPECTED}.items():
        planar = name in PLANAR_EXPECTED
        case = inspect(planar_binary if planar else binary, ROOT / "corpus/geometry" / name,
                       scope="planar-solid" if planar else "faceted-solid")
        r = case["result"]
        actual = tuple(r[k] for k in ("profile", "geometry", "tessellation"))
        case["expected"] = expected
        case["passed"] = actual == expected
        if name in {"box.step", "planar-box.step"}:
            case["passed"] &= (r.get("vertices"),r.get("triangles"),r.get("boundary_edges"),r.get("components")) == (8,12,0,1)
            case["passed"] &= math.isclose(r.get("volume_m3",0),6e-6,rel_tol=1e-12)
        if name in {"tube.step", "planar-tube.step"}:
            case["passed"] &= (r.get("vertices"),r.get("triangles"),r.get("boundary_edges"),r.get("components")) == (16,32,0,1)
            case["passed"] &= math.isclose(r.get("volume_m3",0),3.84e-6,rel_tol=1e-12)
        cases.append(case)
    # Selected upstream adversarial faceted inputs: no copying, healing or expected
    # success is inferred from their parser baseline. These known malformed inputs
    # must never produce a solid mesh under this profile.
    external = Path(os.environ.get("TESSSTEP_CORPUS", str(Path.home()/"step-corpus")))
    upstream = external / "vendor/dodgy-step-files/step-examples"
    for name, root_id in [("12-3a-shells/Tsh002.stp",180),("12-8-mixed/M054.stp",31),
                          ("12-8-mixed/M055.stp",14),("12-8-mixed/M197.stp",14)]:
        path = upstream / name
        if path.is_file():
            case = inspect(binary,path,root_id)
            case["expected"] = "unsupported or rejected; never a mesh"
            case["passed"] = case["result"]["status"] in {"unsupported","rejected"}
            cases.append(case)
    specification = json.loads((ROOT / "corpus/geometry/external-planar.json").read_text())
    cuboid = external / specification["path"]
    if args.require_external and not cuboid.is_file():
        raise RuntimeError(f"Required exporter fixture unavailable: {cuboid}")
    if cuboid.is_file():
        assert hashlib.sha256(cuboid.read_bytes()).hexdigest() == specification["sha256"], "Review changed exporter fixture before updating its hash"
        case = inspect(planar_binary, cuboid, specification["root"], specification["metres_per_unit"], "planar-solid")
        r = case["result"]
        case["expected"] = specification
        case["passed"] = (r["status"] == "accepted" and r.get("vertices") == specification["vertices"]
            and r.get("triangles") == specification["triangles"] and r.get("boundary_edges") == 0
            and r.get("components") == 1 and math.isclose(r.get("volume_m3",0),specification["volume_m3"],rel_tol=1e-12))
        cases.append(case)
    output = ROOT / "reports/geometry"
    output.mkdir(parents=True,exist_ok=True)
    report = {"scope":"selected-root faceted and edge-based planar import; no full-document/AP validation",
              "binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),
              "planar_binary_sha256":hashlib.sha256(planar_binary.read_bytes()).hexdigest(),
              "external_available":external.is_dir(),"cases":cases}
    (output/"latest.json").write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
    rows=[]
    for c in cases:
        r=c["result"]
        rows.append("<tr>"+"".join("<td>"+html.escape(str(v))+"</td>" for v in
                    [Path(c["path"]).name,c["passed"],r["profile"],r["geometry"],r["tessellation"],c["diagnostic"]])+"</tr>")
    (output/"index.html").write_text('<!doctype html><html lang="en"><meta charset="utf-8"><title>Planar STEP import verification</title>'
        '<style>body{font:16px system-ui;margin:32px}td,th{padding:12px;border:1px solid #ccc;text-align:left}table{border-collapse:collapse}</style>'
        '<h1>Planar STEP import verification</h1><p>Selected-root reduced profile. Explicit source units. Full AP schema and product validation are unmeasured.</p>'
        '<table><thead><tr><th>File</th><th>Expectation passed</th><th>Profile</th><th>Geometry/topology</th><th>Tessellation</th><th>Diagnostic</th></tr></thead><tbody>'
        +''.join(rows)+'</tbody></table></html>',encoding="utf-8")
    assert all(c["passed"] for c in cases), json.dumps(report,indent=2)
    print(f"planar/faceted import: {len(cases)} reviewed outcomes passed; {output/'index.html'}")


if __name__ == "__main__":
    main()
