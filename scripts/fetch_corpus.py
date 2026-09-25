#!/usr/bin/env python3
"""Reconstruct the external compatibility corpus from pinned public sources."""
import argparse
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
import zipfile

from corpus import ROOT, EXTENSIONS, digest, discover


def safe_path(name):
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts or "\\" in name or ":" in name:
        raise ValueError(f"Unsafe archive path: {name}")
    return path


def extract_steps(archive, destination, zipped=False):
    """Copy regular STEP files only, without executing or extracting other content."""
    total = 0
    count = 0
    with (zipfile.ZipFile(archive) if zipped else tarfile.open(archive, "r:gz")) as container:
        for member in (container.infolist() if zipped else container):
            name = member.filename if zipped else member.name
            path = safe_path(name)
            if path.suffix.lower() not in EXTENSIONS:
                continue
            if not zipped and not member.isfile():
                raise ValueError(f"Nonregular STEP member: {name}")
            if zipped and ((member.external_attr >> 16) & 0o170000) == 0o120000:
                raise ValueError(f"Symlink STEP member: {name}")
            size = member.file_size if zipped else member.size
            total += size
            if total > 2 * 1024**3:
                raise ValueError("Archive STEP inputs exceed the 2 GiB extraction budget")
            # GitHub tarballs add a repository-revision directory.
            relative = path if zipped else PurePosixPath(*path.parts[1:])
            target = destination.joinpath(*relative.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            with (container.open(member) if zipped else container.extractfile(member)) as src, target.open("xb") as dst:
                shutil.copyfileobj(src, dst)
            count += 1
    return count


def verify(root, baseline):
    expected = {case["sha256"] for case in baseline["cases"]}
    cases = discover(root)
    actual = {case["sha256"] for case in cases}
    missing, extra = expected - actual, actual - expected
    if missing or extra:
        raise ValueError(f"Corpus content differs from baseline: {len(missing)} missing, {len(extra)} extra unique inputs")
    print(f"Verified {len(actual)} unique inputs against the baseline ({sum(len(c['paths']) for c in cases)} paths)", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="New directory to populate (never use your existing local corpus)")
    parser.add_argument("--sources", type=Path, default=ROOT / "corpus/sources.json")
    parser.add_argument("--baseline", type=Path, default=ROOT / "corpus/baseline.json")
    args = parser.parse_args()
    output = args.output.expanduser().resolve()
    if output.exists():
        parser.error("output already exists; use a new directory to avoid mixing snapshots")
    output.parent.mkdir(parents=True, exist_ok=True)
    sources = json.loads(args.sources.read_text(encoding="utf-8"))["sources"]
    baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="corpus-fetch-", dir=output.parent) as temporary:
        staging = Path(temporary) / "inputs"
        for source in sources:
            safe_path(source["name"])
            zipped = "sha256" in source
            url = source["url"] if zipped else source["url"] + "/archive/" + source["revision"] + ".tar.gz"
            archive = Path(temporary) / (source["name"] + (".zip" if zipped else ".tar.gz"))
            print(f'Downloading {source["name"]} from {url}', flush=True)
            subprocess.run(["curl", "--fail", "--location", "--retry", "3", "--max-time", "600", "--silent", "--show-error", "--output", str(archive), url], check=True)
            if zipped and digest(archive) != source["sha256"]:
                raise ValueError(f'{source["name"]}: archive checksum mismatch; review upstream changes')
            destination = staging / "vendor" / source["name"]
            if zipped:
                destination /= "unpacked"
            count = extract_steps(archive, destination, zipped)
            print(f'Extracted {count} STEP files from {source["name"]}', flush=True)
            archive.unlink()
        verify(staging, baseline)
        staging.rename(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
