import io
from pathlib import Path
import re
import tarfile
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile

import corpus_summary
import fetch_corpus
import package_ios
import package_release
import release

ROOT = Path(__file__).resolve().parents[1]


class AcquisitionTests(unittest.TestCase):
    def test_archive_paths_cannot_escape_destination(self):
        for path in ("../escape.stp", "/absolute.step", "repo/../../escape.stp", "C:/file.stp", "dir\\file.stp"):
            with self.assertRaises(ValueError):
                fetch_corpus.safe_path(path)

    def test_extracts_only_step_files_and_strips_github_root(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "files.tar.gz"
            with tarfile.open(archive, "w:gz") as stream:
                for name, content in (("repo-sha/data/part.STP", b"model"), ("repo-sha/script.py", b"do not execute")):
                    entry = tarfile.TarInfo(name)
                    entry.size = len(content)
                    stream.addfile(entry, io.BytesIO(content))
            self.assertEqual(fetch_corpus.extract_steps(archive, root / "out"), 1)
            self.assertEqual((root / "out/data/part.STP").read_bytes(), b"model")
            self.assertFalse((root / "out/script.py").exists())

    def test_symlink_step_entries_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "files.tar.gz"
            with tarfile.open(archive, "w:gz") as stream:
                entry = tarfile.TarInfo("repo/link.step")
                entry.type = tarfile.SYMTYPE
                entry.linkname = "/tmp/private"
                stream.addfile(entry)
            with self.assertRaises(ValueError):
                fetch_corpus.extract_steps(archive, root / "out")

    def test_zip_extraction_preserves_nested_names(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "files.zip"
            with zipfile.ZipFile(archive, "w") as stream:
                stream.writestr("NIST/data/part.stp", "model")
            self.assertEqual(fetch_corpus.extract_steps(archive, root / "out", zipped=True), 1)
            self.assertTrue((root / "out/NIST/data/part.stp").exists())

    def test_missing_or_changed_inputs_fail_inventory_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "a.stp").write_bytes(b"model")
            with self.assertRaisesRegex(ValueError, "1 missing, 1 extra"):
                fetch_corpus.verify(root, {"cases": [{"sha256": "different"}]})


class ReportingTests(unittest.TestCase):
    def report(self):
        return {"summary": {"unique_inputs": 2, "files": 4, "seconds": 1, "unique_statuses": {"clean": 1, "rejected": 1}},
                "binary_sha256": "abc", "baseline_changes": [], "cases": [
                    {"sha256": "a", "paths": ["a.stp"], "status": "clean", "seconds": 0.5, "diagnostics": []},
                    {"sha256": "b", "paths": ["bad.stp"], "status": "rejected", "seconds": 0.5, "diagnostics": []}]}

    def test_known_rejection_is_not_a_conformance_failure(self):
        report = self.report()
        suite = ET.fromstring(corpus_summary.junit(report))
        self.assertEqual(suite.get("tests"), "2")
        self.assertEqual(suite.get("failures"), "0")
        self.assertIn("not an AP validity verdict", corpus_summary.markdown(report))

    def test_regressions_and_missing_inputs_fail_junit(self):
        report = self.report()
        report["baseline_changes"] = [{"kind": "regression", "sha256": "b", "path": "bad.stp"},
                                      {"kind": "removed", "sha256": "c", "path": "gone.stp"}]
        suite = ET.fromstring(corpus_summary.junit(report))
        self.assertEqual(suite.get("tests"), "3")
        self.assertEqual(suite.get("failures"), "2")

    def test_crash_fails_even_when_not_in_baseline(self):
        report = self.report()
        report["cases"][0]["status"] = "crash"
        self.assertEqual(ET.fromstring(corpus_summary.junit(report)).get("failures"), "1")

    def test_release_tag_must_match_workspace_version(self):
        package_release.validate_tag("refs/tags/v0.1.0", "0.1.0")
        for ref in ("refs/heads/main", "refs/tags/v0.2.0", "refs/heads/v0.1.0", ""):
            with self.assertRaises(ValueError):
                package_release.validate_tag(ref, "0.1.0")


LOCK = """version = 4

[[package]]
name = "tessstep-a"
version = "1.0.0"
dependencies = [
 "vendored",
]

[[package]]
name = "vendored"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "00"
"""
CHANGELOG = """# Changelog

## Unreleased

- See [the API](docs/C_API.md#ios), [upstream](https://example.com/x) and [below](#changelog).

## 1.0.0 — 2026-10-10

- First.

## 0.1.0 — 2026-09-27 (pre-release)

- Zeroth.
"""


class ReleaseTests(unittest.TestCase):
    def workspace(self, directory, changelog=CHANGELOG):
        root = Path(directory)
        (root / "fuzz").mkdir()
        (root / "Cargo.toml").write_text('[workspace.package]\nversion = "1.0.0"\n\n[workspace.dependencies]\n'
                                         'tessstep-a = { path = "a", version = "=1.0.0" }\n', encoding="utf-8")
        (root / "Cargo.lock").write_text(LOCK, encoding="utf-8")
        (root / "fuzz/Cargo.lock").write_text(LOCK, encoding="utf-8")
        (root / "CMakeLists.txt").write_text("project(TessSTEP VERSION 1.0.0 LANGUAGES C CXX)\n", encoding="utf-8")
        (root / "CHANGELOG.md").write_text(changelog, encoding="utf-8")
        return root

    def test_bump_sets_every_version_and_dates_the_unreleased_section(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.workspace(directory)
            self.assertEqual(release.bump("1.1.0", "2026-10-11", root), "1.0.0")
            self.assertEqual(release.current_version(root), "1.1.0")
            self.assertIn('version = "=1.1.0"', (root / "Cargo.toml").read_text(encoding="utf-8"))
            self.assertIn("project(TessSTEP VERSION 1.1.0 ", (root / "CMakeLists.txt").read_text(encoding="utf-8"))
            for lockfile in ("Cargo.lock", "fuzz/Cargo.lock"):
                text = (root / lockfile).read_text(encoding="utf-8")
                # The path package moves; the registry package at the same version does not.
                self.assertIn('name = "tessstep-a"\nversion = "1.1.0"\n', text)
                self.assertIn('name = "vendored"\nversion = "1.0.0"\nsource', text)
            changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
            self.assertNotIn("## Unreleased", changelog)
            self.assertIn("## 1.1.0 — 2026-10-11\n", changelog)

    def test_bump_rejects_without_writing_anything(self):
        cases = (("1.0.0", CHANGELOG, "greater"), ("0.9.0", CHANGELOG, "greater"), ("2.0.0", CHANGELOG, "major"),
                 ("1.1", CHANGELOG, "MAJOR.MINOR.PATCH"), ("v1.1.0", CHANGELOG, "MAJOR.MINOR.PATCH"),
                 ("1.1.0", CHANGELOG.replace("## Unreleased", "## Later"), "Unreleased"),
                 ("1.1.0", CHANGELOG.replace("- See [the API](docs/C_API.md#ios), [upstream](https://example.com/x) and [below](#changelog).\n", ""), "empty"))
        for version, changelog, message in cases:
            with self.subTest(version=version, message=message), tempfile.TemporaryDirectory() as directory:
                root = self.workspace(directory, changelog)
                before = {path: path.read_bytes() for path in root.rglob("*") if path.is_file()}
                with self.assertRaisesRegex(ValueError, message):
                    release.bump(version, "2026-10-11", root)
                self.assertEqual(before, {path: path.read_bytes() for path in root.rglob("*") if path.is_file()})

    def test_notes_pin_relative_links_to_the_tag_and_compare_with_the_previous_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.workspace(directory)
            release.bump("1.1.0", "2026-10-11", root)
            text = release.notes("1.1.0", "owner/repo", "https://github.com/owner/repo/actions/runs/42", root)
            self.assertIn("[the API](https://github.com/owner/repo/blob/v1.1.0/docs/C_API.md#ios)", text)
            self.assertIn("[upstream](https://example.com/x) and [below](#changelog)", text)
            self.assertIn("Release gate: [run 42](https://github.com/owner/repo/actions/runs/42)", text)
            self.assertTrue(text.endswith("**Full Changelog**: https://github.com/owner/repo/compare/v1.0.0...v1.1.0\n"))
            self.assertNotIn("First.", text)
            # The oldest release has nothing to compare with.
            self.assertNotIn("Full Changelog", release.notes("0.1.0", "owner/repo", root=root))
            with self.assertRaisesRegex(ValueError, "no section for 1.2.0"):
                release.notes("1.2.0", "owner/repo", root=root)

    def test_the_repository_changelog_has_notes_for_every_release(self):
        for version in re.findall(r"^## (\d+\.\d+\.\d+)", (ROOT / "CHANGELOG.md").read_text(encoding="utf-8"), re.M):
            self.assertTrue(release.notes(version, "owner/repo").strip())


OTOOL = """lib.a(std.o):
Load command 1
      cmd LC_VERSION_MIN_IPHONEOS
  cmdsize 16
  version 10.0
      sdk n/a
lib.a(tessstep.o):
Load command 1
      cmd LC_BUILD_VERSION
  cmdsize 24
 platform 7
    minos 17.0
      sdk 18.5
"""


class IosPackageTests(unittest.TestCase):
    def test_build_versions_read_both_load_commands(self):
        self.assertEqual(sorted(package_ios.build_versions(OTOOL)), [("2", "10.0"), ("7", "17.0")])

    def test_simulator_choice_prefers_booted_then_newest_ipad(self):
        def device(name, state="Shutdown"):
            return {"name": name, "udid": name, "state": state, "isAvailable": True}
        listing = {"devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-16-4": [device("old iPad")],
            "com.apple.CoreSimulator.SimRuntime.iOS-18-6": [device("iPhone 16"), device("iPad Air")],
            "com.apple.CoreSimulator.SimRuntime.iOS-27-0": [device("iPhone 18"), device("iPad Pro")],
            "com.apple.CoreSimulator.SimRuntime.watchOS-11-0": [device("Watch", "Booted")]}}
        self.assertEqual([d["name"] for d in package_ios.simulator_candidates(listing)],
                         ["iPad Pro", "iPhone 18", "iPad Air", "iPhone 16"])
        listing["devices"]["com.apple.CoreSimulator.SimRuntime.iOS-18-6"][0]["state"] = "Booted"
        self.assertEqual(package_ios.simulator_candidates(listing)[0]["name"], "iPhone 16")


class ReleaseWorkflowTests(unittest.TestCase):
    WORKFLOWS = {path.name: path.read_text(encoding="utf-8") for path in (ROOT / ".github/workflows").glob("*.yml")}

    def test_actions_are_pinned_by_commit(self):
        for name, text in self.WORKFLOWS.items():
            for action in re.findall(r"uses: (\S+)", text):
                if not action.startswith("./"):
                    self.assertRegex(action, r"@[0-9a-f]{40}$", name)

    def test_ci_builds_tests_and_packages_ios(self):
        text = self.WORKFLOWS["ci.yml"]
        for step in ("targets: aarch64-apple-ios,aarch64-apple-ios-sim", "python scripts/package_ios.py --select-simulator >> \"$GITHUB_ENV\"",
                     "cargo test --release --locked -p tessstep-capi --target aarch64-apple-ios-sim",
                     "run: python scripts/package_ios.py\n", "name: build-ios"):
            self.assertIn(step, text)

    def test_release_drafts_changelog_notes_with_every_package(self):
        text = self.WORKFLOWS["release.yml"]
        for step in ("python scripts/release.py notes", "name: changelog-notes", "pattern: release-*", "name: build-ios",
                     "--draft --notes-file release-notes.md"):
            self.assertIn(step, text)
        self.assertNotIn("--generate-notes", text)

    def test_prepare_release_pushes_commit_and_tag_together_then_dispatches_release(self):
        text = self.WORKFLOWS["prepare-release.yml"]
        for step in ("workflow_dispatch:", "refs/heads/main", "workflows/ci.yml/runs?head_sha=$GITHUB_SHA&status=success",
                     'python scripts/release.py bump "$VERSION"', "cargo metadata --locked",
                     "cargo metadata --manifest-path fuzz/Cargo.toml --locked",
                     'git push --atomic origin HEAD:refs/heads/main "refs/tags/v$VERSION"',
                     'gh workflow run release.yml --ref "v$VERSION"', "contents: write", "actions: write"):
            self.assertIn(step, text)
        # The typed-in version reaches the shell only through the environment.
        for line in text.splitlines():
            if "inputs." in line:
                self.assertRegex(line, r"^\s+[A-Z_]+: \$\{\{ inputs\.\w+ \}\}$")


if __name__ == "__main__":
    unittest.main()
