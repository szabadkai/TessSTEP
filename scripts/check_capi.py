#!/usr/bin/env python3
"""Install, relocate, and test ABI 1 C/C++ consumers without Cargo in their PATH."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import sys

ROOT = Path(__file__).resolve().parents[1]
# The ABI record: every exported C symbol and the ABI 1.x version that added it.
# Additions are deliberate edits of abi/exports.txt; ABI 1 never removes a symbol.
SYMBOLS = {line.split()[1] for line in (ROOT / "abi/exports.txt").read_text(encoding="utf-8").splitlines()
           if line.strip() and not line.startswith("#")}
# Authored fixtures copied beside the standalone consumers, by consumer file name.
FIXTURES = {
    "faceted.step": "box.step", "planar.step": "planar-box.step",
    "placeholder.step": "planar-placeholder.step", "tessellated.step": "tessellated-connected.step",
    "presentation.step": "presentation.step", "assembly.step": "assembly-nested.step",
    "styled.step": "assembly-styled.step", "missing-placement.step": "assembly-missing-placement.step",
    "cylinder.step": "brep-cylinder.step", "cone.step": "brep-cone.step",
    "cavity.step": "brep-sphere-cavity.step", "selection.step": "tessellated-selection.step",
    "duplicated.step": "brep-repair-duplicated.step", "near.step": "brep-near-surface.step",
    "surface-open.step": "surface-open-patches.step", "mapped.step": "assembly-mapped.step",
    "inch.step": "assembly-inch.step",
}


def run(command, **kwargs):
    subprocess.run([str(c) for c in command], check=True, **kwargs)


def check_symbols(library, consumer_build):
    if os.name == "nt":
        cache = (consumer_build / "CMakeCache.txt").read_text(encoding="utf-8")
        linker = re.search(r"^CMAKE_LINKER:FILEPATH=(.+)$", cache, re.M)
        if not linker:
            raise RuntimeError("Cannot locate MSVC dumpbin for ABI export check")
        dumpbin = Path(linker[1]).with_name("dumpbin.exe")
        output = subprocess.check_output([str(dumpbin), "/EXPORTS", str(library)], text=True)
        exported = set(re.findall(r"^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(\S+)", output, re.M))
    else:
        flags = ["-gUj"] if sys.platform == "darwin" else ["-D", "--defined-only", "--format=posix"]
        output = subprocess.check_output(["nm", *flags, str(library)], text=True)
        exported = {line.split()[0].removeprefix("_") if sys.platform == "darwin"
                    else line.split()[0] for line in output.splitlines() if line.strip()}
    if sys.platform == "darwin":
        identity = subprocess.check_output(["otool", "-D", str(library)], text=True).splitlines()[1:]
        if identity != ["@rpath/libtessstep_capi.dylib"]:
            raise RuntimeError(f"Nonrelocatable library identity: {identity}")
        dependencies = subprocess.check_output(["otool", "-L", str(library)], text=True)
    elif sys.platform.startswith("linux"):
        dependencies = subprocess.check_output(["readelf", "-d", str(library)], text=True)
        if not re.search(r"\(SONAME\).*\[libtessstep_capi.so\]", dependencies):
            raise RuntimeError("Missing relocatable ELF SONAME")
    else:
        dependencies = subprocess.check_output([str(dumpbin), "/DEPENDENTS", str(library)], text=True)
    # A native runtime may depend on OS/runtime libraries, never source/build paths
    # or a Rust toolchain. The exact platform dependency list is retained in CI logs.
    if str(ROOT) in "\n".join(dependencies.splitlines()[1:]):
        raise RuntimeError("Runtime dependency leaks a source-tree path")
    print(dependencies, flush=True)
    if exported != SYMBOLS:
        raise RuntimeError(f"ABI export mismatch with abi/exports.txt: missing={SYMBOLS-exported}, extra={exported-SYMBOLS}")
    print(f"ABI: exactly the {len(SYMBOLS)} C symbols of abi/exports.txt exported; no Rust exports", flush=True)


def verify_installed(package, *, sanitizers=False):
    """Only the copied package and standalone consumer sources are used here."""
    with tempfile.TemporaryDirectory(prefix="tessstep-consumer-") as temporary:
        work = Path(temporary)
        relocated = work / "relocated package"
        shutil.copytree(package, relocated)
        source = work / "consumer"
        shutil.copytree(ROOT / "tests/capi", source)
        for local, authored in FIXTURES.items():
            shutil.copy2(ROOT / "corpus/geometry" / authored, source / local)
        external = Path(os.environ.get("TESSSTEP_CORPUS", str(Path.home() / "step-corpus"))) / "vendor/foxtrot/examples/cuboid.step"
        if external.is_file():
            expected = json.loads((ROOT / "corpus/geometry/external-planar.json").read_text())
            if hashlib.sha256(external.read_bytes()).hexdigest() != expected["sha256"]:
                raise RuntimeError("Unreviewed external cuboid bytes; do not refresh the hash without review")
            # Test-only temporary copy; never installed or added to release archives.
            shutil.copy2(external, source / "external-cuboid.step")
        env = os.environ.copy()
        # Stub executables make accidental Rust toolchain use an explicit failure,
        # including on machines where Cargo shares a directory with C/C++ tools.
        guards = work / "no-rust"
        guards.mkdir()
        for tool in ("cargo", "rustc", "rustup"):
            stub = guards / (tool + (".cmd" if os.name == "nt" else ""))
            stub.write_text("@exit /b 99\n" if os.name == "nt" else "#!/bin/sh\nexit 99\n", encoding="utf-8")
            stub.chmod(0o755)
        system_path = env.get("PATH", "")
        env["PATH"] = os.pathsep.join([str(guards), str(relocated / "bin"), system_path])
        env.pop("CARGO_HOME", None)
        env.pop("RUSTUP_HOME", None)
        for config, c_only in (("Debug", False), ("Release", False), ("Release", True)):
            build = work / (config + ("-C-only" if c_only else ""))
            run(["cmake", "-S", source, "-B", build, f"-DCMAKE_PREFIX_PATH={relocated}",
                 f"-DCMAKE_BUILD_TYPE={config}", f"-DTESSSTEP_C_ONLY={'ON' if c_only else 'OFF'}", f"-DTESSSTEP_SANITIZERS={'ON' if sanitizers else 'OFF'}"], env=env, cwd=work)
            run(["cmake", "--build", build, "--config", config], env=env, cwd=work)
            run(["ctest", "--test-dir", build, "-C", config, "--output-on-failure"], env=env, cwd=work)
            if config == "Release" and not c_only:
                libraries = [p for p in relocated.rglob("*tessstep_capi*") if p.suffix in {".dll", ".dylib", ".so"}]
                if len(libraries) != 1:
                    raise RuntimeError(f"Expected exactly one shared ABI library: {libraries}")
                check_symbols(libraries[0], build)
                if sys.platform == "darwin":
                    for app in ("c_consumer", "cpp_consumer"):
                        linked = subprocess.check_output(["otool", "-L", str(build / app)], text=True)
                        if "@rpath/libtessstep_capi.dylib" not in linked or str(ROOT) in linked:
                            raise RuntimeError(f"Consumer can load a build-tree library: {linked}")
        # Static consumers build and run against a copy of the package without the
        # shared library (or its import library), and without its bin directory on
        # PATH, so any dependency on it fails to link or load. Sanitizers add nothing
        # here: the consumer sources are those instrumented above.
        static_package = work / "static package"
        shutil.copytree(relocated, static_package)
        for shared in list(static_package.rglob("*tessstep_capi*")):
            if shared.suffix in {".dll", ".dylib", ".so"} or shared.name.endswith(".dll.lib"):
                shared.unlink()
        check_static_archive(static_package)
        static_env = dict(env, PATH=os.pathsep.join([str(guards), system_path]))
        build = work / "Release-static"
        run(["cmake", "-S", source, "-B", build, f"-DCMAKE_PREFIX_PATH={static_package}",
             "-DCMAKE_BUILD_TYPE=Release", "-DTESSSTEP_LINK_STATIC=ON"], env=static_env, cwd=work)
        run(["cmake", "--build", build, "--config", "Release"], env=static_env, cwd=work)
        run(["ctest", "--test-dir", build, "-C", "Release", "--output-on-failure"], env=static_env, cwd=work)
    print("C/C++ installed package: relocation, Debug/Release, shared and static linkage, ABI, ownership and concurrent consumers passed", flush=True)


def check_static_archive(package):
    """The archive defines exactly the ABI's C symbols; Rust symbols stay mangled."""
    archives = [p for p in package.rglob("*tessstep_capi*") if p.suffix in {".a", ".lib"}]
    if len(archives) != 1:
        raise RuntimeError(f"Expected exactly one static ABI library: {archives}")
    if os.name == "nt":
        # The static MSVC consumers above link every symbol; dumpbin is not used.
        print(f"Static ABI archive: {archives[0].name}", flush=True)
        return
    check_archive_symbols(archives[0])


def check_archive_symbols(archive):
    flags = ["-gUj"] if sys.platform == "darwin" else ["-g", "--defined-only", "--format=posix"]
    # nm may fail on members it cannot read (symbol-less objects, or embedded bitcode
    # of a newer LLVM in the Rust standard library); the exact ts_ set decides.
    output = subprocess.run(["nm", *flags, str(archive)], stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True).stdout
    names = {line.split()[0].removeprefix("_") if sys.platform == "darwin" else line.split()[0]
             for line in output.splitlines() if line.strip() and not line.endswith(":")}
    defined = {name for name in names if re.fullmatch(r"ts_[a-z0-9_]+", name)}
    if defined != SYMBOLS:
        raise RuntimeError(f"Static ABI mismatch with abi/exports.txt: missing={SYMBOLS-defined}, extra={defined-SYMBOLS}")
    print(f"Static ABI archive: exactly {len(SYMBOLS)} C symbols defined", flush=True)


def install_package(build_dir, prefix, *, library_dir=None, target=None):
    command = ["cmake", "-S", ROOT, "-B", build_dir, f"-DCMAKE_INSTALL_PREFIX={prefix}",
               "-DCMAKE_INSTALL_LIBDIR=lib", "-DCMAKE_INSTALL_BINDIR=bin", "-DCMAKE_INSTALL_INCLUDEDIR=include"]
    if library_dir:
        command += [f"-DTESSSTEP_LIBRARY_DIR={Path(library_dir).resolve()}"]
    if target:
        command += [f"-DTESSSTEP_CARGO_TARGET={target}"]
    run(command)
    run(["cmake", "--build", build_dir, "--config", "Release"])
    run(["cmake", "--install", build_dir, "--config", "Release"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library-dir", type=Path, help="Reuse an existing release cdylib")
    parser.add_argument("--sanitizers", action="store_true", help="ASan/UBSan on native consumers (Rust library is not instrumented)")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="tessstep-capi-") as temporary:
        work = Path(temporary)
        installed = work / "original install"
        install_package(work / "build", installed, library_dir=args.library_dir)
        # Move away from the original install path before testing relocation.
        moved = work / "moved install"
        installed.rename(moved)
        verify_installed(moved, sanitizers=args.sanitizers)


if __name__ == "__main__":
    main()
