# TessSTEP

[![CI](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml/badge.svg)](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml)

[CI, corpus reports and releases](docs/RELEASING.md)

**A memory-safe STEP/EXPRESS geometry kernel and watertight adaptive tessellator for Rust.**

This repository implements Milestones 0–2: repository infrastructure, streaming
physical-file parsing, generic entity storage, reference analysis, and an EXPRESS
frontend with basic schema validation. The CLIs are `stepdump` and `expressc`.
It does **not** yet generate bindings or interpret AP242 instances, products, units,
geometry, or meshes. EXPRESS expression semantics remain explicitly unsupported.
A file parsing successfully is not evidence of schema or CAD conformance.

The C ABI and C++ wrapper are supported public-interface commitments. Their
[contract](docs/C_API.md) requires complete isolation from Rust implementation
semantics, C++ RAII ownership, typed errors, zero-copy read-only mesh views where
possible, and the CMake target `TessSTEP::TessSTEP`. Neither interface is implemented
in this initial parser delivery.

```sh
cargo build --workspace
cargo run -p stepdump -- corpus/part21/valid/values.step
cargo run -p stepdump -- --json corpus/part21/valid/extended.step
cargo run -p expressc -- --json corpus/express/valid/base.exp corpus/express/valid/imports.exp
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
legacy scope handling, archive reader, schema-aware instance validation, or AP support claim.

- [Architecture and dependency boundaries](docs/ARCHITECTURE.md)
- [Physical format and resource limits](docs/PART21.md)
- [Tests, fuzzing, and benchmarks](docs/TESTING.md)
- [EXPRESS frontend and limits](docs/EXPRESS.md)
- [Next milestone: generated bindings](docs/ROADMAP.md)

Licensed under [MIT](LICENSE-MIT).
