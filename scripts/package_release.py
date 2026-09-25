#!/usr/bin/env python3
"""Package CLI tools and the C/C++ SDK with relocated consumer tests."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

from check_capi import install_package, verify_installed

ROOT = Path(__file__).resolve().parents[1]


def validate_tag(ref, version):
    if ref != f"refs/tags/v{version}":
        raise ValueError(f"Release requires refs/tags/v{version}; got {ref!r}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--validate-tag", action="store_true")
    parser.add_argument("--target")
    parser.add_argument("--cross-target", action="store_true")
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    if args.validate_tag:
        validate_tag(os.environ.get("GITHUB_REF", ""), version)
        print(f"Validated release tag v{version}")
        return
    if not args.target or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789_-" for c in args.target):
        parser.error("--target must be a Rust target triple")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version=1", "--no-deps"], cwd=ROOT))
    build = Path(metadata["target_directory"])
    if args.cross_target:
        build /= args.target
    build /= "release"
    suffix = ".exe" if "windows" in args.target else ""
    name = f"tessstep-{version}-{args.target}"
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="tessstep-package-") as temporary:
        package = Path(temporary) / name
        (package / "bin").mkdir(parents=True)
        for tool in ("stepdump", "expressc"):
            binary = package / "bin" / (tool + suffix)
            shutil.copy2(build / (tool + suffix), binary)
            subprocess.run([str(binary), "--help"], cwd=temporary, check=True, capture_output=True)
        for filename in ("README.md", "LICENSE-MIT"):
            shutil.copy2(ROOT / filename, package / filename)
        shutil.copytree(ROOT / "docs", package / "docs")
        # Authored fixtures only; external CAD inputs are never shipped.
        shutil.copytree(ROOT / "corpus/part21", package / "examples/part21")
        shutil.copytree(ROOT / "corpus/express", package / "examples/express")
        parsed = subprocess.check_output([str(package / "bin" / ("stepdump" + suffix)), "--json", str(package / "examples/part21/valid/empty.step")], cwd=temporary)
        if json.loads(parsed)["document"] is None:
            raise ValueError("Packaged stepdump failed its fixture smoke test")
        subprocess.run([str(package / "bin" / ("expressc" + suffix)), "--json", str(package / "examples/express/valid/base.exp")], cwd=temporary, check=True, capture_output=True)
        generated = subprocess.check_output([str(package / "bin" / ("expressc" + suffix)), "--rust", str(package / "examples/express/valid/base.exp")], cwd=temporary, stderr=subprocess.PIPE)
        if b"pub static SCHEMA_SET" not in generated:
            raise ValueError("Packaged expressc failed Rust generation smoke test")
        install_package(Path(temporary) / "cmake-build", package, library_dir=build)
        verify_installed(package)
        (package / "PACKAGE.json").write_text(json.dumps({"version": version, "target": args.target,
            "commit": os.environ.get("GITHUB_SHA", "local"), "tools": ["stepdump", "expressc"],
            "c_abi": {"version": 1, "scope": "physical_documents", "linkage": "shared"},
            "cpp_wrapper": {"standard": "C++17", "target": "TessSTEP::TessSTEP"}, "tessellation": "not_implemented"}, indent=2) + "\n", encoding="utf-8")
        if suffix:
            archive = output / (name + ".zip")
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
                for path in sorted(package.rglob("*")):
                    if path.is_file():
                        stream.write(path, path.relative_to(package.parent))
        else:
            archive = output / (name + ".tar.gz")
            with tarfile.open(archive, "w:gz") as stream:
                stream.add(package, arcname=name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{checksum}  {archive.name}\n", encoding="utf-8")
    print(f"Packaged and smoke-tested: {archive}")


if __name__ == "__main__":
    main()
