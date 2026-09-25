#!/usr/bin/env python3
"""Independent CLI JSON checks for original EXPRESS fixtures; no AP conformance claim."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
subprocess.run(["cargo", "build", "-p", "expressc", "--locked"], cwd=ROOT, check=True)
EXE = ROOT / "target" / "debug" / ("expressc.exe" if os.name == "nt" else "expressc")

def run(paths, code, strict=False):
    args = [str(EXE), "--json"] + (["--strict"] if strict else []) + list(map(str, paths))
    a = subprocess.run(args, cwd=ROOT, capture_output=True, check=False)
    b = subprocess.run(args, cwd=ROOT, capture_output=True, check=False)
    assert a.returncode == code, (args, a.stderr, a.stdout)
    assert a.stdout == b.stdout and a.returncode == b.returncode
    assert not a.stderr
    result = json.loads(a.stdout)
    assert result["success"] == (code == 0)
    assert result["expression_semantics"] == "not_implemented"
    for d in result["diagnostics"]:
        span = d["span"]
        assert 0 <= span["source"] < len(paths)
        assert 0 <= span["start"] <= span["end"] <= (ROOT / paths[span["source"]]).stat().st_size
    return result

base = Path("corpus/express/valid/base.exp")
imports = Path("corpus/express/valid/imports.exp")
c = run([base, imports], 0)
assert len(c["schemas"]) == 3 and len(c["declarations"]) == 9
assert all(d["severity"] == "unsupported" for d in c["diagnostics"])
assert c["schemas"][1]["symbols"]["COMPONENT"] == c["schemas"][0]["symbols"]["PART"]
assert "FACTOR" not in c["schemas"][1]["exports"]
assert run([base], 1, strict=True)["structural_valid"]
assert not run([imports], 1)["structural_valid"]
for path in sorted((ROOT / "corpus/express/invalid").glob("*.exp")):
    assert not run([path], 1)["structural_valid"]
run([Path("corpus/express/valid/opaque.exp")], 0)
with tempfile.TemporaryDirectory() as directory:
    filename = 'unicode-é.exp' if os.name == 'nt' else 'quote"slash\\line\né.exp'
    path = Path(directory) / filename
    path.write_text("SCHEMA clean; ENTITY item; a : REAL; END_ENTITY; END_SCHEMA;", encoding="utf-8")
    result = run([path], 0, strict=True)
    assert result["sources"] == [str(path)]
    assert not result["diagnostics"]
print("EXPRESS CLI: authored fixtures, stages, spans, strict policy, JSON and determinism passed")
