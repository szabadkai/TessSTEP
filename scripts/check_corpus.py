#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
manifest = json.loads((ROOT / "corpus" / "manifest.json").read_text(encoding="utf-8"))
expected = {item["path"] for item in manifest["files"]}
actual = {str(path.relative_to(ROOT / "corpus")).replace("\\", "/") for path in (ROOT / "corpus" / "part21").glob("*/*.step")}
actual |= {str(path.relative_to(ROOT / "corpus")).replace("\\", "/") for path in (ROOT / "corpus" / "express").glob("*/*.exp")}
actual |= {path.relative_to(ROOT / "corpus").as_posix() for path in (ROOT / "corpus/schema").iterdir() if path.suffix in {".step", ".exp"}}
assert expected == actual, "Every fixture needs provenance"
for item in manifest["files"]:
    path = ROOT / "corpus" / item["path"]
    assert hashlib.sha256(path.read_bytes()).hexdigest() == item["sha256"], path
    assert item["source"] and item["license"]
print(f"corpus provenance: {len(expected)} authored fixtures verified")
