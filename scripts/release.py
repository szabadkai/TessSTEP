#!/usr/bin/env python3
"""Prepare a release's version change and write its notes from CHANGELOG.md.

`bump VERSION` sets the workspace, internal dependency, lockfile and CMake package
versions and dates the CHANGELOG's Unreleased section as VERSION. `notes` prints a
version's CHANGELOG section with repository links pinned to its tag.
"""
import argparse
import datetime
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def current_version(root=ROOT):
    return tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]


def parse(version):
    match = re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", version)
    if not match:
        raise ValueError(f"Release version must be MAJOR.MINOR.PATCH; got {version!r}")
    return tuple(map(int, match.groups()))


def section(changelog, title):
    """The text under `## <title>` or `## <title> — <date>`, up to the next section."""
    match = re.search(rf"^## {re.escape(title)}(?: — [^\n]*)?\n(.*?)(?=^## |\Z)", changelog, re.M | re.S)
    return match[1].strip() if match else None


def replace(text, old, new, path, count=None):
    found = text.count(old)
    if not found or (count is not None and found != count):
        raise ValueError(f"{path}: expected {count or 'some'} occurrence(s) of {old!r}, found {found}")
    return text.replace(old, new)


def bump_lockfile(text, old, new):
    """Path packages (no source line) at the old version: the workspace and its path dependencies."""
    pattern = r'(\[\[package\]\]\nname = "[^"]+"\nversion = ")' + re.escape(old) + r'("\n(?!source = ))'
    return re.subn(pattern, rf"\g<1>{new}\g<2>", text)


def bump(version, date, root=ROOT):
    old = current_version(root)
    if parse(version) <= parse(old):
        raise ValueError(f"Release version {version} must be greater than the current {old}")
    if parse(version)[0] != parse(old)[0]:
        # SameMajorVersion package compatibility: consumers' find_package minima change too.
        raise ValueError(f"{version} changes the major version of {old}; prepare it in a reviewed change (docs/RELEASING.md)")
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    if not section(changelog, "Unreleased"):
        raise ValueError("CHANGELOG.md has no Unreleased section to release, or it is empty")
    files = {}
    text = (root / "Cargo.toml").read_text(encoding="utf-8")
    text = replace(text, f'\nversion = "{old}"\n', f'\nversion = "{version}"\n', "Cargo.toml", count=1)
    files["Cargo.toml"] = replace(text, f'version = "={old}"', f'version = "={version}"', "Cargo.toml")
    files["CMakeLists.txt"] = replace((root / "CMakeLists.txt").read_text(encoding="utf-8"),
                                      f"project(TessSTEP VERSION {old} ", f"project(TessSTEP VERSION {version} ", "CMakeLists.txt", count=1)
    files["CHANGELOG.md"] = replace(changelog, "\n## Unreleased\n", f"\n## {version} — {date}\n", "CHANGELOG.md", count=1)
    for lockfile in ("Cargo.lock", "fuzz/Cargo.lock"):
        files[lockfile], changed = bump_lockfile((root / lockfile).read_text(encoding="utf-8"), old, version)
        if not changed:
            raise ValueError(f"{lockfile}: no workspace package at version {old}")
    # Nothing is written until every file has its edit.
    for name, text in files.items():
        (root / name).write_text(text, encoding="utf-8", newline="\n")
    return old


def notes(version, repository, run_url=None, root=ROOT):
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    body = section(changelog, version)
    if not body:
        raise ValueError(f"CHANGELOG.md has no section for {version}")
    # Relative links resolve against the repository root at the release's tag.
    base = f"https://github.com/{repository}/blob/v{version}/"
    text = re.sub(r"\]\((?![a-z][a-z0-9+.-]*:|#)([^)\s]+)\)", lambda link: f"]({base}{link[1]})", body)
    if run_url:
        text += (f"\n\nRelease gate: [run {run_url.rstrip('/').rsplit('/', 1)[-1]}]({run_url}) built these packages"
                 " at the tag and ran the relocated C/C++ consumers on every target.")
    # The next older section is the previous release.
    headings = re.findall(r"^## (\S+)", changelog, re.M)
    older = headings[headings.index(version) + 1:]
    if older:
        text += f"\n\n**Full Changelog**: https://github.com/{repository}/compare/v{older[0]}...v{version}"
    return text + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    bumping = commands.add_parser("bump", help="set VERSION everywhere and date the Unreleased changes")
    bumping.add_argument("version")
    bumping.add_argument("--date", default=datetime.date.today().isoformat())
    noting = commands.add_parser("notes", help="print a version's CHANGELOG section as release notes")
    noting.add_argument("version", nargs="?", help="defaults to the workspace version")
    noting.add_argument("--repository", required=True, help="OWNER/REPO for links pinned to the tag")
    noting.add_argument("--run-url", help="the Release workflow run that built the packages")
    args = parser.parse_args()
    try:
        if args.command == "bump":
            print(f"Version {bump(args.version, args.date)} -> {args.version}", file=sys.stderr)
        else:
            sys.stdout.write(notes(args.version or current_version(), args.repository, args.run_url))
    except ValueError as error:
        sys.exit(f"release: {error}")


if __name__ == "__main__":
    main()
