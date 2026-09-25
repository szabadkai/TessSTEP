#!/usr/bin/env python3
"""Enforce the declared downward crate graph without an external Python package."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {
    "tessstep-part21": set(),
    "tessstep-express": set(),
    "tessstep-schema": {"tessstep-express"},
    "tessstep-codegen": {"tessstep-express", "tessstep-schema"},
    "tessstep-model": {"tessstep-part21", "tessstep-schema"},
    "tessstep-math": set(),
    "tessstep-curves": {"tessstep-math"},
    "tessstep-surfaces": {"tessstep-math", "tessstep-curves"},
    "tessstep-topology": {"tessstep-math", "tessstep-curves", "tessstep-surfaces"},
    "tessstep-trim": {"tessstep-math", "tessstep-curves", "tessstep-surfaces", "tessstep-topology"},
    "tessstep-mesh": {"tessstep-math"},
    "tessstep-tessellate": {"tessstep-math", "tessstep-curves", "tessstep-surfaces", "tessstep-topology", "tessstep-trim", "tessstep-mesh"},
    "tessstep-validate": {"tessstep-math", "tessstep-model", "tessstep-schema", "tessstep-topology", "tessstep-mesh"},
    "tessstep-ap242": {"tessstep-model", "tessstep-schema", "tessstep-product", "tessstep-topology", "tessstep-curves", "tessstep-surfaces", "tessstep-math"},
    "tessstep-product": {"tessstep-math", "tessstep-topology"},
    "tessstep-io": {"tessstep-mesh", "tessstep-product", "tessstep-math"},
    "expressc": {"tessstep-express"},
    "stepdump": {"tessstep-part21", "tessstep-model"},
}
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version=1", "--no-deps"], cwd=ROOT))
for package in metadata["packages"]:
    name = package["name"]
    internal = {d["name"] for d in package["dependencies"] if d["name"].startswith("tessstep-")}
    if name not in {"tessstep", "tessstep-capi"}:
        assert name in ALLOWED, f"Declare an architectural boundary for {name}"
        assert internal <= ALLOWED[name], f"Forbidden dependencies in {name}: {internal - ALLOWED[name]}"
    # Milestones 0–2 deliberately have no third-party production dependencies.
    assert all(d.get("path") for d in package["dependencies"]), f"Review new dependency in {name}"
    for target in package["targets"]:
        if "lib" in target["kind"] or "bin" in target["kind"]:
            source = Path(target["src_path"]).read_text(encoding="utf-8")
            assert "#![forbid(unsafe_code)]" in source, f"Missing unsafe policy in {target['src_path']}"
print(f"architecture: {len(metadata['packages'])} packages; dependency and unsafe boundaries passed")
