# TessSTEP

[![CI](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml/badge.svg)](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml)

[CI, corpus reports and releases](docs/RELEASING.md)

**A memory-safe STEP/EXPRESS geometry kernel and watertight adaptive tessellator for Rust.**

This repository implements Milestones 0–4: repository infrastructure, streaming
physical-file parsing, generic entity storage, reference analysis, and an EXPRESS
frontend with basic schema validation, deterministic Rust bindings and static schema
reflection, plus [structural instance decoding](docs/DECODING.md) against supplied metadata.
Complex entities, SELECTs, aggregate uniqueness and bounded width/bound expressions are checked. The CLIs are `stepdump` and `expressc`.
Milestone 5 now adds a [structural product graph](docs/PRODUCT_MODEL.md), explicit unit scales,
assembly occurrences and preserved placement/mapping descriptions over supplied decoded metadata.
Milestone 6 adds [independent checked math](docs/GEOMETRY.md): typed coordinates, units,
tolerances and affine transforms. STEP placements, curve/surface geometry and meshes
remain unevaluated; no AP242 conformance is claimed.
General EXPRESS expression semantics remain explicitly unsupported.
A file parsing successfully is not evidence of schema or CAD conformance.

The [C ABI and C++17 wrapper](docs/C_API.md) now expose physical buffer parsing,
immutable document inspection and structured reference diagnostics. The C++ wrapper
provides move-only RAII ownership and typed results. Install the shared library and
headers with CMake, then link `TessSTEP::TessSTEP`. Schema decoding and mesh views
are not exposed by these interfaces yet.

```sh
cargo build --workspace
cargo run -p stepdump -- corpus/part21/valid/values.step
cargo run -p stepdump -- --json corpus/part21/valid/extended.step
cargo run -p expressc -- --json corpus/express/valid/base.exp corpus/express/valid/imports.exp
cargo run -p expressc -- --rust corpus/express/valid/base.exp > bindings.rs
python3 scripts/check_schema.py
python3 scripts/check_product.py
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Rust edition 2024; minimum Rust 1.85. The production workspace uses only the standard
library and local crates. Linux, Windows, and macOS CI is configured. The local
verification recorded in [VALIDATION.md](docs/VALIDATION.md) is on macOS arm64.

```rust
use std::{fs::File, io::BufReader};
use tessstep::{parse, ParseLimits};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = parse(BufReader::new(File::open("part.step")?), ParseLimits::default())?;
    for (name, count) in document.entities().counts_by_type() {
        println!("{name}: {count}");
    }
    for diagnostic in document.diagnostics() {
        eprintln!("{diagnostic}");
    }
    Ok(())
}
```

`part21::Parser` is a pull iterator for consumers that cannot retain a whole document.
Drain it to EOF: an `EndExchange` event precedes optional signatures and trailing-input
validation. `model::parse` retains the generic database and rejects duplicate IDs.
Reference analysis is an explicit subsequent operation. Missing references are errors
in its report; external references and unverified signatures generate warnings.

The parser accepts a documented clear-text subset with simple/complex records,
nested and typed parameters, exact binary bit lengths, Unicode encodings, multiple
DATA sections, anchors, external reference declarations, and signature payloads.
See the generated [conformance table](docs/CONFORMANCE.md) for limitations.
There is no tolerant syntax recovery, external fetching, cryptographic verification,
legacy scope handling, archive reader, full schema validation, or AP support claim.

- [Architecture and dependency boundaries](docs/ARCHITECTURE.md)
- [Physical format and resource limits](docs/PART21.md)
- [Tests, fuzzing, and benchmarks](docs/TESTING.md)
- [EXPRESS frontend and limits](docs/EXPRESS.md)
- [Generated bindings and reflection](docs/SCHEMA.md)
- [Products, units and assembly graphs](docs/PRODUCT_MODEL.md)
- [Milestones and remaining work](docs/ROADMAP.md)

Licensed under [MIT](LICENSE-MIT).
