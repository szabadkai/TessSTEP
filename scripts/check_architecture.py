#!/usr/bin/env python3
"""Enforce the declared downward crate graph without an external Python package."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {
    "tessstep-part21": set(),
    "tessstep-express": set(),
    "tessstep-schema": set(),
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
    "expressc": {"tessstep-express", "tessstep-codegen"},
    "stepdump": {"tessstep-part21", "tessstep-model"},
    "tessstep-capi": {"tessstep-part21", "tessstep-model"},
}
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version=1", "--no-deps"], cwd=ROOT))
for package in metadata["packages"]:
    name = package["name"]
    internal = {d["name"] for d in package["dependencies"] if d["name"].startswith("tessstep-")}
    if name != "tessstep":
        assert name in ALLOWED, f"Declare an architectural boundary for {name}"
        assert internal <= ALLOWED[name], f"Forbidden dependencies in {name}: {internal - ALLOWED[name]}"
    # Production crates deliberately have no third-party dependencies.
    assert all(d.get("path") for d in package["dependencies"]), f"Review new dependency in {name}"
    for target in package["targets"]:
        if {"lib", "rlib", "cdylib", "staticlib", "bin"} & set(target["kind"]):
            source = Path(target["src_path"]).read_text(encoding="utf-8")
            policy = "#![deny(unsafe_op_in_unsafe_fn)]" if name == "tessstep-capi" else "#![forbid(unsafe_code)]"
            assert policy in source, f"Missing unsafe policy in {target['src_path']}"
print(f"architecture: {len(metadata['packages'])} packages; dependency and unsafe boundaries passed")
