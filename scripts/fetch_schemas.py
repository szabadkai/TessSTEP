#!/usr/bin/env python3
"""Fetch the pinned EXPRESS long forms for the corpus schema stage and verify their hashes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="Directory to populate with the schema files")
    parser.add_argument("--sources", type=Path, default=ROOT / "corpus/ap-schemas.json")
    args = parser.parse_args()
    sources = json.loads(args.sources.read_text(encoding="utf-8"))
    output = args.output.expanduser().resolve()
    output.mkdir(parents=True, exist_ok=True)
    base = f'https://raw.githubusercontent.com/{sources["repository"].split("github.com/")[1]}/{sources["revision"]}/'
    for schema in sources["schemas"]:
        target = output / schema["file"]
        if not target.is_file() or hashlib.sha256(target.read_bytes()).hexdigest() != schema["sha256"]:
            url = base + schema["path"]
            print(f"Downloading {url}", flush=True)
            subprocess.run(["curl", "--fail", "--location", "--retry", "3", "--max-time", "600", "--silent", "--show-error", "--output", str(target), url], check=True)
        actual = hashlib.sha256(target.read_bytes()).hexdigest()
        if actual != schema["sha256"]:
            print(f'{target}: SHA-256 {actual} differs from pinned {schema["sha256"]}', file=sys.stderr)
            return 1
        print(f'{schema["file"]}: {schema["schema"]} verified', flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
