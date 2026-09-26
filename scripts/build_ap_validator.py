#!/usr/bin/env python3
"""Generate and build one schema validator holding every pinned AP long form.

The validator is a generated `expressc --validator` application compiled against the
workspace's tessstep-model and tessstep-schema crates. Nothing is bundled: the schema
files come from `scripts/fetch_schemas.py`, and the generated crate lives under the
cargo target directory. Prints the built executable's path.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schemas", type=Path, required=True, help="Directory populated by fetch_schemas.py")
    parser.add_argument("--sources", type=Path, default=ROOT / "corpus/ap-schemas.json")
    parser.add_argument("--output", type=Path, help="Copy the executable here (default: target/release/ap-validator)")
    args = parser.parse_args()
    sources = json.loads(args.sources.read_text(encoding="utf-8"))
    schemas = args.schemas.expanduser().resolve()
    files = []
    for schema in sources["schemas"]:
        path = schemas / schema["file"]
        if hashlib.sha256(path.read_bytes()).hexdigest() != schema["sha256"]:
            print(f"{path} does not match its pinned SHA-256; run scripts/fetch_schemas.py", file=sys.stderr)
            return 1
        files.append(path)
    subprocess.run(["cargo", "build", "--release", "--locked", "-p", "expressc"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version=1"], cwd=ROOT))
    target = Path(metadata["target_directory"])
    suffix = ".exe" if os.name == "nt" else ""
    expressc = target / "release" / ("expressc" + suffix)
    # Budgets: the four long forms hold about 1.2 million tokens and generate ~20 MB.
    generated = subprocess.run([str(expressc), "--validator", "--max-tokens", "4000000", "--max-work", "100000000", "--max-output-bytes", "268435456", *map(str, files)],
                               cwd=ROOT, capture_output=True, check=True).stdout
    crate = target / "ap-validator"
    (crate / "src").mkdir(parents=True, exist_ok=True)
    (crate / "Cargo.toml").write_text(
        '[package]\nname = "ap-validator"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n\n'
        f'[dependencies]\ntessstep-model = {{ path = "{(ROOT / "crates/tessstep-model").as_posix()}" }}\n'
        f'tessstep-schema = {{ path = "{(ROOT / "crates/tessstep-schema").as_posix()}" }}\n\n[workspace]\n', encoding="utf-8")
    main_rs = crate / "src/main.rs"
    if not main_rs.exists() or main_rs.read_bytes() != generated:
        main_rs.write_bytes(generated)
    subprocess.run(["cargo", "build", "--release"], cwd=crate, check=True)
    built = crate / "target/release" / ("ap-validator" + suffix)
    output = (args.output or target / "release" / ("ap-validator" + suffix)).expanduser().resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(built.read_bytes())
    output.chmod(0o755)
    print(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
