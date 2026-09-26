# TessSTEP

[![CI](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml/badge.svg)](https://github.com/szabadkai/TessSTEP/actions/workflows/ci.yml)

**STEP and EXPRESS tooling, geometry, and adaptive tessellation in Rust, with public C and C++ interfaces.**

TessSTEP provides STEP physical-file parsing, structural schema decoding, product
and assembly graphs, analytic and NURBS geometry, and adaptive tessellation of
constructed boundary representations (B-reps). Meshes retain shared boundary
positions, per-corner normals and UVs, and face provenance. Assembly scenes reuse
mesh assets across nested placements and support explicit appearance overrides.

**STEP-to-mesh supports explicit planar solid profiles.** The importer accepts a
selected `FACETED_BREP` with polygon boundaries or `MANIFOLD_SOLID_BREP` with
straight edge curves, using caller-supplied units. General curved STEP geometry import is not implemented.
No AP203, AP214, or AP242 conformance is claimed; successful physical parsing does
not establish schema or CAD validity.

## Current capabilities

| Area | Available now | Scope and limitations |
| --- | --- | --- |
| STEP Part 21 | Streaming parser, generic entity storage, source spans, resource limits, and reference diagnostics | A documented clear-text subset; external references and signatures are retained but not fetched or verified |
| EXPRESS and schemas | Declaration parsing, import resolution, Rust bindings, static reflection, and structural instance decoding | General EXPRESS rules and algorithms are unsupported; no full ISO application-protocol schema is bundled |
| Products and assemblies | Product/representation graphs, unit scales, uncertainty measures, evaluated placements, and bounded nested expansion | Local 3D relationships with supplied metadata and explicit alternative selection; mesh binding remains caller-controlled |
| Geometry | Typed coordinates, tolerances, affine transforms, analytic curves/surfaces, and NURBS evaluation and knot insertion | Independent evaluators; curved STEP geometry adaptation remains pending |
| Topology and trimming | Structural B-rep validation, supplied-pcurve UV reconstruction, and canonical shared-edge sampling | No automatic healing, missing-pcurve projection, or general volume-validity proof |
| Tessellation | Planar and regular curved-face refinement, owned shell/solid meshes, and manifold closure checks | Constructed B-reps; sampled error checks, no certified continuous bounds, singularity repair, or self-intersection proof |
| Planar STEP import | Faceted and edge-based solids converted to owned meshes with original STEP face IDs | One closed shell; planar polygon or LINE/EDGE_LOOP boundaries; explicit root, units and tolerances |
| Mesh scenes and appearance | Shared assets, nested affine instances, explicit baking, linear RGBA palettes, and inherited style overrides | No automatic STEP mesh binding or style adaptation, world-space tolerance guarantee, textures, or rendering |
| C and C++ | Document inspection, planar/planar/faceted STEP tessellation, triangle-buffer import, retained mesh views, scenes, and appearance | Shared ABI 1 library and C++17 RAII; general schema decoding and constructed B-rep tessellation remain Rust APIs |

The [STEP import guide](docs/STEP_IMPORT.md) documents the first end-to-end slice.

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

## Tessellate a planar STEP solid

For a conventional edge-based B-rep:

```sh
cargo run -p tessstep-import --example planar -- corpus/geometry/planar-box.step 1000 0.001
```

Use `import_planar_solid` in Rust or `Document::tessellate_planar` in C++. The
[import guide](docs/STEP_IMPORT.md) also includes an unmodified CAD-exported cuboid.

For the polygon-based faceted profile:

Run the included example against the authored box fixture:

```sh
cargo run -p tessstep-import --example faceted -- corpus/geometry/box.step 1000 0.001
```

Here `1000` selects entity `#1000`, and `0.001` specifies metres per source unit
(millimetres). The example reports stage results and mesh statistics as JSON:
8 vertices, 12 triangles, no boundary edges, and one connected component. It does
not write a mesh file.

The Rust entry point is `tessstep::import::import_faceted_solid`; its result exposes
an owned B-rep and a `tessellate` method. The importer checks the selected root's
reference closure against an original reduced profile. It does not validate
unrelated records or the full `FILE_SCHEMA`, infer units, apply assembly placements,
or repair invalid geometry. Mesh positions are in metres, and triangle face IDs
refer to the original STEP faces. See the
[example source](crates/tessstep-import/examples/faceted.rs) for explicit tolerances
and error handling.

## Rust API

The `tessstep` crate re-exports the parsing, schema, product, geometry, topology,
trimming, tessellation, mesh, and import APIs. To use a local checkout, add a path dependency to your application's
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

The shared library exposes ABI 1 through the [C header](include/tessstep/tessstep.h).
The [C++17 wrapper](include/tessstep/tessstep.hpp) provides move-only RAII ownership
and typed results for documents, meshes, scenes, and appearance layers.

Supported operations include physical-document inspection and diagnostics, selected
planar/faceted STEP tessellation, triangle-buffer import, immutable mesh queries, assembly
scene construction and instance baking, and appearance queries. C++ `MeshView`
retains the mesh, so its zero-copy buffers remain valid after the originating
`Mesh` is destroyed. Raw pointers remain borrowed from the view.

Use `ts_document_tessellate_faceted` in C or `Document::tessellate_faceted` in C++
with an explicit root entity and unit scale. The result owns its mesh independently
of the source document. General schema decoding, product adaptation, and constructed
B-rep tessellation are not exposed through the native interfaces. Shared linkage
is supported; static linkage is not provided.

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
python3 scripts/check_geometry.py
python3 scripts/check_capi.py
python3 scripts/conformance.py --check
```

For parser, model, geometry, or tessellation changes, also run
`python3 scripts/corpus.py --check` when the external corpus is available at
`~/step-corpus` or `TESSSTEP_CORPUS`. Review the per-file progress report and any
outcome changes before considering a baseline update. Physical parsing, schema
decoding, product interpretation, geometry, and tessellation are separate
verification stages; passing one does not establish support for the next.
The faceted import check records its selected-profile results separately in
`reports/geometry/`; these do not establish general external STEP geometry support.

See [testing](docs/TESTING.md) for the full workflow, fuzzing, and benchmarks;
[corpus testing](docs/corpus-testing.md) for external inputs and baselines; and
[validation results](docs/VALIDATION.md) for recorded checks and known limits.

## Documentation

- [Architecture and dependency boundaries](docs/ARCHITECTURE.md)
- [EXPRESS frontend](docs/EXPRESS.md) and [schema decoding](docs/DECODING.md)
- [Products, units, and assembly graphs](docs/PRODUCT_MODEL.md)
- [Geometry foundations](docs/GEOMETRY.md) and [numerical robustness](docs/NUMERICAL_ROBUSTNESS.md)
- [Analytic curves](docs/CURVES.md), [surfaces](docs/SURFACES.md), and [NURBS](docs/NURBS.md)
- [Topology](docs/TOPOLOGY.md), [UV trimming](docs/TRIMMING.md), and [tessellation](docs/TESSELLATION.md)
- [Mesh storage](docs/MESH.md), [assembly assets](docs/ASSEMBLY_ASSETS.md), and [appearance](docs/APPEARANCE.md)
- [C/C++ API and ownership contracts](docs/C_API.md)
- [CI, corpus reports, and release packaging](docs/RELEASING.md)

## License

TessSTEP is licensed under the [MIT License](LICENSE-MIT). External corpus files
remain subject to their upstream licenses.
