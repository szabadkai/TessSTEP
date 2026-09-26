#!/usr/bin/env python3
"""Compile an authored schema validator and check real per-fixture schema stages."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    subprocess.run(["cargo", "build", "--locked", "-p", "expressc"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version=1"], cwd=ROOT))
    suffix = ".exe" if os.name == "nt" else ""
    compiler = Path(metadata["target_directory"]) / "debug" / ("expressc" + suffix)
    generated = subprocess.check_output([str(compiler), "--validator", "corpus/schema/sample.exp"], cwd=ROOT)
    with tempfile.TemporaryDirectory(prefix="tessstep-schema-") as directory:
        work = Path(directory)
        (work / "src").mkdir()
        (work / "src/main.rs").write_bytes(generated)
        dependencies = "\n".join(f'{name} = {{ path = {json.dumps(str(ROOT / "crates" / name))} }}' for name in ("tessstep-model", "tessstep-schema"))
        (work / "Cargo.toml").write_text('[package]\nname="schema-validator"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\n' + dependencies + '\n', encoding="utf-8")
        subprocess.run(["cargo", "build", "--offline", "--release", "--manifest-path", str(work / "Cargo.toml"), "--target-dir", str(work / "target")], check=True)
        validator = work / "target/release" / ("schema-validator" + suffix)
        expectations = {Path(item["path"]).name: {"stages": {"schema": item["schema"]}}
                        for item in json.loads((ROOT / "corpus/manifest.json").read_text(encoding="utf-8"))["files"]
                        if item["path"].startswith("schema/") and item["path"].endswith(".step")}
        (work / "expectations.json").write_text(json.dumps({"cases": expectations}), encoding="utf-8")
        subprocess.run([sys.executable, str(ROOT / "scripts/corpus.py"), "--corpus", str(ROOT / "corpus/schema"), "--output", str(ROOT / "reports/schema"), "--baseline", str(work / "no-baseline.json"), "--expectations", str(work / "expectations.json"), "--title", "Authored schema structure", "--repeat", "2", "--schema-validator", str(validator), "--schema-name", "sample"], cwd=ROOT, check=True)
    report = json.loads((ROOT / "reports/schema/latest.json").read_text())
    expected = {Path(item["path"]).name: item["schema"] for item in json.loads((ROOT / "corpus/manifest.json").read_text())["files"] if item["path"].startswith("schema/") and item["path"].endswith(".step")}
    actual = {path: case["stages"]["schema"] for case in report["cases"] for path in case["paths"]}
    assert actual == expected, (actual, expected)
    assert all(case["stages"]["physical_parse"] == "accepted" for case in report["cases"])
    print(f"schema corpus: {len(actual)} expected structural outcomes verified; reports/schema/index.html")


if __name__ == "__main__":
    main()
