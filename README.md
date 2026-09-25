# TessSTEP

[![CI](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml/badge.svg)](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml)

**STEP parsing, EXPRESS tooling, and geometry foundations in Rust, with public C and C++ interfaces.**

TessSTEP is building toward a STEP geometry kernel and adaptive tessellator.
Today it provides physical-file parsing, structural schema decoding, product and
assembly graphs, independent analytic and NURBS geometry, structural B-rep validation,
UV reconstruction, and shared-edge boundary sampling.

**STEP-to-mesh conversion is not implemented.** The geometry evaluators operate on
explicitly constructed geometry; they are not yet connected to STEP records.
Face triangulation and solid tessellation remain under development. No AP203, AP214, or AP242
conformance is claimed, and successful parsing does not establish schema or CAD
validity.

## Current capabilities

| Area | Available now | Scope and limitations |
| --- | --- | --- |
| STEP Part 21 | Streaming parser, generic entity storage, source spans, resource limits, and reference diagnostics | A documented clear-text subset; external references and signatures are retained but not fetched or verified |
| EXPRESS | Declaration parsing, import resolution, structural checks, deterministic Rust bindings, and static schema reflection | Schemas and their dependencies must be supplied explicitly; general expression and algorithm evaluation is unsupported |
| Schema decoding | Structural validation of simple and complex instances, SELECTs, bounds, widths, aggregate uniqueness, and local references | Requires supplied schema metadata; does not implement full EXPRESS rules or application-protocol semantics |
| Products and assemblies | Product/representation graphs, assembly occurrences, mapped reuse, and explicit unit scales | A structural subset; placement descriptions are preserved without evaluating geometric transforms |
| Geometry | Typed coordinates, tolerances, affine transforms, analytic curves and surfaces, and NURBS evaluation with derivatives and knot insertion | Independent Rust APIs; no STEP geometry adapter or meshes |
| B-rep boundaries | Typed topology validity states, supplied-pcurve UV loops, and canonical shared-edge samples | Constructed geometry only; sampled error checks, no triangle meshes or volume validity proof |
| C and C++ | Shared C ABI library, C++17 RAII wrapper, typed errors, and an installable CMake package | Physical buffer parsing, immutable document inspection, and reference diagnostics only |

The [conformance table](docs/CONFORMANCE.md) records supported behavior and test
evidence. See the [roadmap](docs/ROADMAP.md) for remaining work.

## Build and try the tools

Building from source requires **Rust 1.85 or later** (edition 2024). The production
workspace uses only the Rust standard library and local crates. CI is configured
for Linux, Windows, and macOS.

Run these commands from the repository root:

```sh
cargo build --workspace --locked

# Inspect STEP headers, entity counts, and references.
cargo run -p stepdump -- corpus/part21/valid/values.step
cargo run -p stepdump -- --json corpus/part21/valid/extended.step

# Inspect explicitly supplied EXPRESS schemas.
cargo run -p expressc -- --json corpus/express/valid/base.exp corpus/express/valid/imports.exp

# Generate Rust bindings and schema metadata.
cargo run -p expressc -- --rust corpus/express/valid/base.exp > target/bindings.rs
```

`stepdump` also accepts `-` for standard input. It reports physical syntax and
reference issues; it does not perform schema validation or geometry interpretation.

`expressc` preserves unsupported semantics with diagnostics. Use `--strict` to
reject them. Generated bindings require a matching `tessstep-schema` dependency;
see [schema bindings](docs/SCHEMA.md) for integration and
[structural decoding](docs/DECODING.md) for the generated validator workflow.
No ISO application-protocol schema is bundled.

## Rust API

The `tessstep` crate re-exports the parser, model, schema, product, and geometry
crates. To use a local checkout, add a path dependency to your application's
`Cargo.toml`, adjusting the path as needed:

```toml
[dependencies]
tessstep = { path = "../TessSTEP/crates/tessstep" }
```

This example loads a physical document, counts record types, and reports reference
and trust diagnostics:

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

`parse` retains the complete generic document and rejects duplicate IDs. Reference
analysis is separate: `document.diagnostics()` reports missing references as errors
and external references or unverified signatures as warnings.

For streaming consumption, use `tessstep::part21::Parser` and drain it to EOF.
The `EndExchange` event precedes optional signatures and trailing-input validation.
See [Part 21 parsing](docs/PART21.md) for format support and resource limits.

## C and C++ API

The public interfaces provide physical-document operations through ABI 1 and a
header-only C++17 wrapper with move-only RAII ownership and typed results.
Schema decoding, product graphs, geometry, and mesh views are not exposed through
these interfaces yet. Shared linkage is supported; static linkage is not provided.

Building the package from source requires Rust, **CMake 3.20 or later**, and a
**C11/C++17 toolchain**:

```sh
cmake -S . -B target/cmake -DCMAKE_INSTALL_PREFIX=/absolute/install/path
cmake --build target/cmake --config Release
cmake --install target/cmake --config Release
```

Set `CMAKE_PREFIX_PATH` to the installation directory when configuring your
application, then link the package target:

```cmake
find_package(TessSTEP 0.1 CONFIG REQUIRED)
target_link_libraries(my_app PRIVATE TessSTEP::TessSTEP)
```

Consumers of the installed package do not need a Rust toolchain. See the
[C/C++ API guide](docs/C_API.md) for examples, ownership contracts, platform details,
and runtime library setup.

## Development and verification

Run the core Rust checks from the repository root:

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Additional integration checks use Python 3.11 or later; the C/C++ check also
requires the native toolchain described above:

```sh
python3 scripts/check_schema.py
python3 scripts/check_product.py
python3 scripts/check_capi.py
python3 scripts/conformance.py --check
```

For parser, model, geometry, or tessellation changes, also run
`python3 scripts/corpus.py --check` when the external corpus is available at
`~/step-corpus` or `TESSSTEP_CORPUS`. Review the per-file progress report and any
outcome changes before considering a baseline update. Physical parsing, schema
decoding, product interpretation, and tessellation are separate verification
stages; passing one does not establish support for the next.

See [testing](docs/TESTING.md) for the full workflow, fuzzing, and benchmarks;
[corpus testing](docs/corpus-testing.md) for external inputs and baselines; and
[validation results](docs/VALIDATION.md) for recorded checks and known limits.

## Documentation

- [Architecture and dependency boundaries](docs/ARCHITECTURE.md)
- [EXPRESS frontend](docs/EXPRESS.md) and [schema decoding](docs/DECODING.md)
- [Products, units, and assembly graphs](docs/PRODUCT_MODEL.md)
- [Geometry foundations](docs/GEOMETRY.md) and [numerical robustness](docs/NUMERICAL_ROBUSTNESS.md)
- [Analytic curves](docs/CURVES.md), [surfaces](docs/SURFACES.md), and [NURBS](docs/NURBS.md)
- [CI, corpus reports, and release packaging](docs/RELEASING.md)

## License

TessSTEP is licensed under the [MIT License](LICENSE-MIT). External corpus files
remain subject to their upstream licenses.
