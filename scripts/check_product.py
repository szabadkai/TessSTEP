#!/usr/bin/env python3
"""Verify generated fixture metadata and run separate schema/product corpus stages."""
import json
import math
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
    generated = subprocess.check_output([str(compiler), "--rust", "corpus/product/sample.exp"], cwd=ROOT)
    assert generated == (ROOT / "crates/tessstep-ap242/tests/support/schema.rs").read_bytes(), "Regenerate product fixture metadata"
    schema_source = subprocess.check_output([str(compiler), "--validator", "corpus/product/sample.exp"], cwd=ROOT)
    with tempfile.TemporaryDirectory(prefix="tessstep-product-") as directory:
        work = Path(directory)
        (work / "src/bin").mkdir(parents=True)
        (work / "src/bin/schema-validator.rs").write_bytes(schema_source)
        (work / "src/bin/product-validator.rs").write_bytes(generated + (ROOT / "crates/tessstep-ap242/tests/support/validator.rs.txt").read_bytes())
        dependencies = "\n".join(f'{name} = {{ path = {json.dumps(str(ROOT / "crates" / name))} }}' for name in ("tessstep-model", "tessstep-schema", "tessstep-ap242"))
        (work / "Cargo.toml").write_text('[package]\nname="product-checkers"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\n' + dependencies + '\n', encoding="utf-8")
        subprocess.run(["cargo", "build", "--offline", "--release", "--manifest-path", str(work / "Cargo.toml"), "--target-dir", str(work / "target")], check=True)
        expectations = {Path(item["path"]).name: {"stages": {"schema": "accepted", "product": item["product"]}}
                        for item in json.loads((ROOT / "corpus/manifest.json").read_text(encoding="utf-8"))["files"]
                        if item["path"].startswith("product/") and item["path"].endswith(".step")}
        for item in json.loads((ROOT / "corpus/manifest.json").read_text(encoding="utf-8"))["files"]:
            if item.get("product") == "accepted":
                measurements = dict(item["product_counts"])
                # Only the first relationship is an authored numerical oracle.
                expectations[Path(item["path"]).name]["first_relationship_matrix"] = item["first_relationship_matrix"]
                if Path(item["path"]).name == "indirect.step":
                    measurements["uncertainty_values_si"] = [0.00001]
                expectations[Path(item["path"]).name]["product_result_contains"] = measurements
        (work / "expectations.json").write_text(json.dumps({"cases": expectations}), encoding="utf-8")
        subprocess.run([sys.executable, str(ROOT / "scripts/corpus.py"), "--corpus", str(ROOT / "corpus/product"), "--output", str(ROOT / "reports/product"), "--baseline", str(work / "no-baseline.json"), "--expectations", str(work / "expectations.json"), "--title", "Authored product structure", "--repeat", "2", "--schema-validator", str(work / "target/release" / ("schema-validator" + suffix)), "--product-validator", str(work / "target/release" / ("product-validator" + suffix)), "--schema-name", "product_test"], cwd=ROOT, check=True)
    report = json.loads((ROOT / "reports/product/latest.json").read_text())
    expected = {Path(item["path"]).name: item["product"] for item in json.loads((ROOT / "corpus/manifest.json").read_text())["files"] if item["path"].startswith("product/") and item["path"].endswith(".step")}
    actual = {path: case["stages"]["product"] for case in report["cases"] for path in case["paths"]}
    assert actual == expected, (actual, expected)
    assert all(case["stages"]["schema"] == "accepted" for case in report["cases"])
    expectations = {Path(item["path"]).name: item for item in json.loads((ROOT / "corpus/manifest.json").read_text())["files"] if item["path"].startswith("product/")}
    for case in report["cases"]:
        if case["stages"]["product"] == "accepted":
            result = case["product_result"]
            item = expectations[case["paths"][0]]
            assert {key: result[key] for key in item["product_counts"]} == item["product_counts"]
            matrix = result["relationship_matrices"][0]
            assert all(math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-12) for row, expected_row in zip(matrix, item["first_relationship_matrix"], strict=True) for a, b in zip(row, expected_row, strict=True))
            if case["paths"] == ["indirect.step"]:
                assert result["uncertainty_values_si"] == [0.00001]
    print(f"product corpus: {len(actual)} reviewed outcomes verified; reports/product/index.html")


if __name__ == "__main__":
    main()
