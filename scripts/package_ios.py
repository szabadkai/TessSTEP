#!/usr/bin/env python3
"""Package the iOS static library as an XCFramework and test it as an iOS consumer would.

Needs Xcode and the release static libraries for aarch64-apple-ios and
aarch64-apple-ios-sim (`cargo build --release --locked -p tessstep-capi --target ...`).
Every object in a slice must be built for its platform and require no iOS newer than
the deployment target, and each archive must define exactly the ABI's C symbols. The standalone C11 and C++17 consumers link both
slices of the packaged XCFramework with the iOS SDK; the simulator builds run in the
simulator named by TESSSTEP_IOS_SIMULATOR. `--select-simulator` boots one and prints
that variable for $GITHUB_ENV.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

from check_capi import FIXTURES, ROOT, SYMBOLS, check_archive_symbols, run
from ios_sim_run import find_device
from package_release import ABI_LEVEL, write_archive
from release import current_version

# An explicit environment value wins over .cargo/config.toml, as it does for Cargo.
DEPLOYMENT_TARGET = os.environ.get("IPHONEOS_DEPLOYMENT_TARGET") or tomllib.loads(
    (ROOT / ".cargo/config.toml").read_text(encoding="utf-8"))["env"]["IPHONEOS_DEPLOYMENT_TARGET"]
# Rust target -> (Mach-O platform, SDK, clang target, XCFramework platform variant).
SLICES = {
    "aarch64-apple-ios": ("2", "iphoneos", f"arm64-apple-ios{DEPLOYMENT_TARGET}", None),
    "aarch64-apple-ios-sim": ("7", "iphonesimulator", f"arm64-apple-ios{DEPLOYMENT_TARGET}-simulator", "simulator"),
}
# simctl spawn starts a process in the device's data directory; the consumers open
# their fixtures by relative name, so this runs before their main().
ENTER_FIXTURES = """#include <stdlib.h>
#include <unistd.h>
__attribute__((constructor)) static void enter_fixture_directory(void) {
    const char *directory = getenv("TESSSTEP_CONSUMER_DIR");
    if (directory && chdir(directory) != 0) abort();
}
"""


def version_key(text):
    return tuple(int(part) for part in text.split("."))


def build_versions(otool_output):
    """(platform, minimum OS) of each object in `otool -l` output; the older
    LC_VERSION_MIN_IPHONEOS (Rust's prebuilt device std) means iOS devices."""
    found = re.findall(r"cmd LC_BUILD_VERSION\n\s*cmdsize \d+\n\s*platform (\S+)\n\s*minos (\S+)", otool_output)
    found += [("2", minimum) for minimum in
              re.findall(r"cmd LC_VERSION_MIN_IPHONEOS\n\s*cmdsize \d+\n\s*version (\S+)", otool_output)]
    return found


def check_slice(library, target):
    platform = SLICES[target][0]
    versions = build_versions(subprocess.check_output(["otool", "-l", str(library)], text=True))
    if not versions:
        raise RuntimeError(f"{library}: no Mach-O build versions")
    platforms = {found for found, _ in versions}
    newest = max((minimum for _, minimum in versions), key=version_key)
    if platforms != {platform}:
        raise RuntimeError(f"{library}: objects for Mach-O platforms {sorted(platforms)}, expected {platform} for {target}")
    if version_key(newest) > version_key(DEPLOYMENT_TARGET):
        raise RuntimeError(f"{library}: an object requires iOS {newest}, newer than the deployment target {DEPLOYMENT_TARGET}")
    check_archive_symbols(library)
    print(f"{target}: {len(versions)} objects for platform {platform}, none newer than iOS {DEPLOYMENT_TARGET}", flush=True)


def simulator_candidates(listing):
    """Available devices on iOS runtimes at or past the deployment target, best first:
    a booted one saves the boot; otherwise the newest runtime, iPads first."""
    candidates = []
    for runtime, devices in listing["devices"].items():
        match = re.search(r"\.iOS-(\d+)-(\d+)$", runtime)
        if match and version_key(".".join(match.groups())) >= version_key(DEPLOYMENT_TARGET):
            candidates += [((d["state"] == "Booted", tuple(map(int, match.groups())), d["name"].startswith("iPad")), d)
                           for d in devices if d.get("isAvailable", True)]
    return [device for _, device in sorted(candidates, key=lambda c: c[0], reverse=True)]


def select_simulator():
    listing = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "devices", "available", "--json"], text=True))
    candidates = simulator_candidates(listing)
    if not candidates:
        sys.exit(f"package_ios: no available simulator runs iOS {DEPLOYMENT_TARGET} or later")
    device = candidates[0]
    # stdout is the variable alone; simctl's progress goes to stderr.
    if device["state"] != "Booted":
        run(["xcrun", "simctl", "boot", device["udid"]], stdout=sys.stderr)
    run(["xcrun", "simctl", "bootstatus", device["udid"], "-b"], stdout=sys.stderr)
    print(f"Booted {device['name']} ({device['udid']})", file=sys.stderr)
    print(f"TESSSTEP_IOS_SIMULATOR={device['udid']}")


def test_consumers(xcframework, simulator):
    """Link the standalone consumers against each slice the way Xcode resolves them,
    from the XCFramework's Info.plist, and run the simulator builds."""
    libraries = plistlib.loads((xcframework / "Info.plist").read_bytes())["AvailableLibraries"]
    with tempfile.TemporaryDirectory(prefix="tessstep-ios-consumer-") as temporary:
        work = Path(temporary)
        for local, authored in FIXTURES.items():
            shutil.copy2(ROOT / "corpus/geometry" / authored, work / local)
        (work / "enter.c").write_text(ENTER_FIXTURES, encoding="utf-8")
        binaries = []
        for target, (_, sdk, clang_target, variant) in SLICES.items():
            matching = [l for l in libraries if l["SupportedPlatform"] == "ios" and l.get("SupportedPlatformVariant") == variant]
            if len(matching) != 1:
                raise RuntimeError(f"Expected one ios {variant or 'device'} library in {xcframework.name}: {libraries}")
            slice_dir = xcframework / matching[0]["LibraryIdentifier"]
            build = work / target
            build.mkdir()
            platform = ["-target", clang_target]
            run(["xcrun", "--sdk", sdk, "clang", *platform, "-c", work / "enter.c", "-o", build / "enter.o"])
            # The flags of tests/capi/CMakeLists.txt.
            for app, source, driver, standard in (("c_consumer", "consumer.c", "clang", "c11"),
                                                  ("cpp_consumer", "consumer.cpp", "clang++", "c++17")):
                binary = build / app
                run(["xcrun", "--sdk", sdk, driver, *platform, f"-std={standard}", "-O2", "-UNDEBUG",
                     "-Wall", "-Wextra", "-Wpedantic", "-Werror",
                     "-I", slice_dir / matching[0]["HeadersPath"], ROOT / "tests/capi" / source, build / "enter.o",
                     slice_dir / matching[0]["LibraryPath"], "-o", binary])
                if variant == "simulator":
                    binaries.append(binary)
        # Device builds need signing to run; they are linked only.
        env = dict(os.environ, SIMCTL_CHILD_TESSSTEP_CONSUMER_DIR=str(work))
        for binary in binaries:
            run(["xcrun", "simctl", "spawn", simulator, binary], env=env)
    print(f"iOS XCFramework: C11/C++17 consumers linked for devices and passed in simulator {simulator}", flush=True)


def package(simulator):
    version = current_version()
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version=1", "--no-deps"], cwd=ROOT))
    build = Path(metadata["target_directory"])
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="tessstep-package-") as temporary:
        package_dir = Path(temporary) / f"tessstep-{version}-ios"
        package_dir.mkdir()
        arguments = []
        for target in SLICES:
            library = build / target / "release/libtessstep_capi.a"
            check_slice(library, target)
            arguments += ["-library", library, "-headers", ROOT / "include"]
        run(["xcodebuild", "-create-xcframework", *arguments, "-output", package_dir / "TessSTEP.xcframework"])
        for filename in ("README.md", "LICENSE-MIT"):
            shutil.copy2(ROOT / filename, package_dir / filename)
        shutil.copytree(ROOT / "docs", package_dir / "docs")
        shutil.copytree(ROOT / "abi", package_dir / "abi")
        test_consumers(package_dir / "TessSTEP.xcframework", simulator)
        (package_dir / "PACKAGE.json").write_text(json.dumps({"version": version, "target": "ios",
            "commit": os.environ.get("GITHUB_SHA", "local"), "slices": list(SLICES),
            "deployment_target": DEPLOYMENT_TARGET, "xcframework": "TessSTEP.xcframework",
            "c_abi": {"version": 1, "frozen": "1.0", "level": ABI_LEVEL, "exports": len(SYMBOLS),
                      "record": "abi/exports.txt", "linkage": ["static"]},
            "cpp_wrapper": {"standard": "C++17", "header": "tessstep/tessstep.hpp"}}, indent=2) + "\n", encoding="utf-8")
        archive = write_archive(package_dir, output, zipped=True)
    print(f"Packaged and consumer-tested: {archive}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--select-simulator", action="store_true",
                        help="boot an iOS simulator and print TESSSTEP_IOS_SIMULATOR=<udid>")
    args = parser.parse_args()
    if sys.platform != "darwin":
        sys.exit("package_ios: needs macOS with Xcode")
    if args.select_simulator:
        select_simulator()
        return
    wanted = os.environ.get("TESSSTEP_IOS_SIMULATOR")
    if not wanted:
        sys.exit("package_ios: set TESSSTEP_IOS_SIMULATOR to a simulator UDID or name (see --select-simulator)")
    device = find_device(wanted)
    if device["state"] != "Booted":
        run(["xcrun", "simctl", "boot", device["udid"]])
    package(device["udid"])


if __name__ == "__main__":
    main()
