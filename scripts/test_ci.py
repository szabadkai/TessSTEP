import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile

import corpus_summary
import fetch_corpus
import package_release


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


if __name__ == "__main__":
    unittest.main()
