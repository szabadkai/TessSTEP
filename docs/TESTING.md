# Testing

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 scripts/check_architecture.py
python3 scripts/check_corpus.py
python3 scripts/check_cli.py
python3 scripts/check_express.py
python3 scripts/conformance.py --check
cargo bench -p tessstep-part21 --bench parser
cargo bench -p tessstep-express --bench compiler
cargo bench -p tessstep-codegen --bench generator
cargo test -p tessstep-model --test fuzz_smoke -- --nocapture
cargo test -p tessstep-express --test fuzz_smoke -- --nocapture
```

Tests cover every value family, scalar boundaries, all supported page encodings,
invalid Unicode, exact binary bit packing, complex record structure, mandatory headers,
section order, external references/signatures, source spans, sparse and duplicate IDs,
cycles, missing references, every resource budget, deep recursion, and I/O errors.
Chunk sizes 1–64 must produce identical events; the first event must arrive without
reading the whole file. An independent Python JSON decoder checks CLI output and byte
determinism over every corpus input. Generated graphs exercise reference properties.

Corpus files are authored synthetic fixtures, not proprietary CAD exports. Each file
has source/license/hash provenance in `corpus/manifest.json`. Invalid inputs and missing
references are deliberately included. New regressions need a fixture or focused test.
`corpus/private/` is ignored; do not commit customer models. Third-party corpus acquisition
requires documented permission, license, and exporter/version provenance.

`fuzz_smoke` shares bounded exercise code with libFuzzer. It replays all committed files,
every prefix, seven byte replacements at each offset, and seeded arbitrary-byte inputs.
It is deterministic and runs in normal stable CI. This is useful regression evidence,
not an exhaustive security result. The separate `fuzz` package provides lexer, parser,
and entity-database targets. Nightly cargo-fuzz adds coverage and sanitizer instrumentation;
see `fuzz/README.md`. Save a minimized failure as a reproducible regression.

PR CI covers stable Linux/Windows/macOS, the MSRV, parser/property/corpus tests, architecture,
format/lint, conformance generation, JSON determinism, dependency policy and advisory checks.
Scheduled CI runs release tests, benchmarks, and short instrumented fuzz sessions. Longer
runs for memory profiling, huge datasets, CAD differential testing and cross-platform
numerics belong to later milestones and are not claimed by the current suite.

When `~/step-corpus` or `TESSSTEP_CORPUS` is available, repository instructions also
require `python3 scripts/corpus.py --check`. Inspect the per-file report and summarize
changes without refreshing the baseline to hide regressions. See
[corpus-testing.md](corpus-testing.md). Run the Python runner tests with
`python3 -m unittest discover -s scripts -p 'test_*.py'`.

EXPRESS tests cover source spans, literal/comment lexing, declarations, preserved opaque
expressions, imports/aliases/reexports, name collisions, wrong declaration kinds,
inheritance/type cycles, diamond inheritance, inverse target lookup, malformed syntax,
and all logical budgets including expansion. Original `.exp` fixtures have hashes and
license provenance in the same manifest as Part 21 fixtures. `check_express.py` checks
CLI JSON with Python's independent decoder, strict-policy exit status, resolved identities,
source spans and byte determinism. The separate EXPRESS fuzz target shares its finite
exercise function with stable mutation/prefix/random-input tests. Schema-source frontend
success is separate from schema validation of STEP instances (Milestone 4).

Milestone 3 tests compile generated Rust with an independent rustc consumer that links
only `tessstep-schema`, run its reflection assertions, and require a nominal-reference
assignment to fail compilation. Coverage includes repeated byte-identical generation,
imports and aliases, recursive references, diamond inheritance, cross-schema inherited
anonymous domains, enum/SELECT construction, scalar/aggregate/optional mappings, retained
rules/constants/algorithms and exact output budget boundaries. Shared `schema_codegen`
fuzz exercise code checks deterministic generation under finite budgets. Stable prefix,
mutation and seeded arbitrary-input smoke runs are part of `cargo test --workspace`.
The generator benchmark reports throughput for an already compiled 5,001-declaration IR.
Public C/C++ consumer checks are described below.

## Milestone 4 initial decoder

`cargo test -p tessstep-model` includes borrowed views, inherited attributes, forward/cyclic
references, domain/bounds checks, precise error context, unsupported cases, malformed
metadata and deterministic decoder mutation smoke. `cargo test -p tessstep-codegen generated_metadata_decodes_physical_instances` compiles and runs a consumer of freshly
generated metadata with the model runtime. See DECODING.md for supported-stage limits.

## Public C/C++ package

`cargo test -p tessstep-capi` checks bridge ownership, diagnostic mapping, frozen
layouts, every exposed parse budget, cleared failure outputs, Send/Sync storage and
panic containment (including a panic payload with a panicking destructor).
`python3 scripts/check_capi.py` builds/installs the shared package, removes its original
installation path and tests relocated standalone C11/C++17 consumers in Debug and
Release. Consumers resolve only `TessSTEP::TessSTEP`, with Rust commands blocked.
C tests check the independent ABI layouts, statuses, pointer stability under retained
ownership and reports after document release. C++ tests check moves, typed failures,
owned diagnostics and concurrent reads. Exact export checks reject Rust or accidental
public symbols. Packaging invokes these tests on every CI/release target.

Use `--library-dir target/release` to reuse a release build and `--sanitizers` to
instrument native consumers with ASan/UBSan (Linux/macOS CI). Rust-library sanitizer
instrumentation, static packages and mesh-view tests are not claimed by this slice.

## Milestone 4 structural completion

Run `python3 scripts/check_schema.py` for the generated validator and six authored
per-fixture schema outcomes; it exercises real corpus-stage integration and protocol
validation. `cargo test -p tessstep-model --test decode` covers complex/diamond membership,
SELECT tags/nesting, semantic aggregate equality, numeric boundaries, expression limits,
and malformed metadata. `decoder_fuzz` shares generated fixture metadata and the bounded
exercise function with the nightly `schema_decoder` target. A generator regression test
verifies that its checked-in metadata still matches the authored `.exp` source.

## Product graph slice

Run `cargo test -p tessstep-product -p tessstep-ap242` and
`python3 scripts/check_product.py`. Independent model tests do not need STEP.
Adapter tests use generated metadata from the authored product schema, cover complex
membership and assembly/map reuse, and run 1,500 deterministic mutations. A
10,000-definition chain verifies iterative graph checks.
`cargo bench -p tessstep-ap242 --bench product` measures 10,002 repeated occurrences
after parsing/decoding, including output allocations and destruction. `product_adapter`
is the corresponding libFuzzer target; nightly CI configures an instrumented run.
Observed checks are recorded separately in VALIDATION.md.

## Milestone 6 independent math

Run `cargo test -p tessstep-math` (debug and release) and
`cargo bench -p tessstep-math --bench transforms`. Tests cover known affine inverses,
composition order, inverse-transpose normals, scale/shear/reflection, frame handedness,
pivot swaps, singular/near-singular inputs, non-finite/overflow/subnormal handling and
2,000 seeded affine round trips. Compile-fail doctests enforce frame separation and
composition order. The deterministic fuzz smoke exercises 10,000 arbitrary-bit inputs
and 870 boundary/prefix inputs through the same fixed-work harness as `math_primitives`.
Nightly CI configures its instrumented run. The benchmark measures one inverse and
10,000 checked point transforms per batch; point construction is outside timing.

## Milestone 7 analytic curves

Run `cargo test -p tessstep-curves` (debug and release) and
`cargo bench -p tessstep-curves --bench analytic`. Independent tests cover known conic
positions/derivatives, finite-difference checks, implicit conic equations, 2D/3D frames,
periodicity, seam-crossing and reversed spans, chain-rule factors, affine covariance,
and invalid/overflow/subnormal inputs. A compile-fail doctest checks frame spaces.
The stable arbitrary-bit harness uses 10,000 random inputs plus 774 boundary/prefix
inputs and is shared with the `analytic_curves` nightly libFuzzer target. Its maximum
consumed input is 128 bytes and work is fixed. The benchmark measures 10,000 checked
ellipse evaluations (position and both derivatives), with curve setup outside timing.

## Milestones 8–10 surfaces and NURBS

Run `cargo test -p tessstep-curves -p tessstep-surfaces` in debug/release and
`cargo bench -p tessstep-surfaces --bench evaluators`. Analytic tests check known
positions, all five partials by finite differences, domains, periodic seams and
pole/apex singularities. Spline tests cover rational circles/cylinders, known weighted
bilinear derivatives, repeated knots and one-sided limits, nonclamped/periodic nets,
affine/weight-scale invariance, degree 16, invalid structure/ranges and resource limits.
Both curve and surface refinement must preserve positions and derivatives.

Two stable harnesses each run 5,000 arbitrary-bit, 512 mutation and 129 prefix inputs.
They are shared with `nurbs_curves` and `surface_evaluators` libFuzzer targets; each
consumes at most 128 bytes and uses small finite construction limits. The benchmark
reports separate 1,000-evaluation batches for analytic torus, rational curve and
rational surface jets. Setup is outside timing; checked evaluation is inside.

## Topology, UV trimming and shared-edge boundaries (Milestones 11–13)

```sh
cargo test -p tessstep-topology -p tessstep-trim -p tessstep-tessellate
cargo test -p tessstep-topology -p tessstep-trim -p tessstep-tessellate --release
cargo bench -p tessstep-tessellate --bench boundaries
cargo fuzz run brep_pipeline -- -max_total_time=120 -max_len=96
```

Constructed fixtures cover invalid ownership/references, shell closure/connectivity,
edge orientation and vertex-link defects, planar holes, curved boundaries, periodic
seams, singularities, canonical position identity, knot corners and discontinuities.
Compile-fail examples distinguish handle kinds and prevent bypassing validity states.
A shared 3,582-case stable harness mutates bounded raw topology and arbitrary float
intervals, then exercises each successful downstream stage with finite budgets.
The same harness is a libFuzzer target in nightly CI. External STEP corpus acceptance
continues to measure physical parsing until a real geometry adapter is implemented.

## Adaptive tessellation and meshes (Milestones 15–17)

Run `cargo test -p tessstep-tessellate -p tessstep-mesh -p tessstep-topology`
and the same command with `--release`. Adaptive tests independently probe every output
triangle on a denser barycentric grid, check tighter tolerances, regular analytic/rational
NURBS patches, narrow knot spans, shared boundary refinement, sharp corner normals, caps,
doubly periodic tori, positive/inward volume, planar holes and finite limits. Mesh tests
cover malformed attributes, duplicates, orientation, disconnected/open components and
pinched vertices. Topology has a closed-edge endpoint-incidence regression.

The combined mutation harness now includes closed cylinders and NURBS bump fixtures,
then exercises bounded adaptive tessellation and owned-mesh invariants after earlier
stages succeed. The boundary benchmark includes full closed-cylinder and NURBS-patch
meshing, with construction/normalization outside timing. Native mesh buffers and view
lifetimes are release gates via `python3 scripts/check_capi.py --sanitizers`.

## Assembly assets

`cargo test -p tessstep-mesh` covers nested/shared occurrences, mirrored/nonuniform
transforms, provenance, deep/cyclic graphs, resource failures and bounded mutations.
`cargo bench -p tessstep-mesh --bench scenes` measures graph construction and explicit
baking separately. `cargo test -p tessstep-capi` and the installed C/C++ consumer gate
include scene layouts, acquire/release symmetry, zero-copy assets and failure outputs.

## Appearance

`cargo test -p tessstep-mesh` includes palette/assignment validation, all precedence
levels, transparent versus unstyled results, face identity, reflected baking correspondence,
10,000-deep inheritance and 2,000 deterministic cases checked against an independent
parent-walking oracle. `cargo bench -p tessstep-mesh --bench appearance` measures
construction and allocation-free triangle queries. C ABI tests and installed C/C++
consumers exercise appearance records, input copying, retained scene/mesh lifetimes,
concurrent reads, typed errors and unchanged earlier ABI layouts.

Milestone 5 completion also checks evaluated SI matrices, axis defaults, Cartesian
reflection/nonuniform scale, indirect context/shape associations and positive uncertainty.
Explicit assembly expansion tests cover reversed endpoints, noncommuting nested maps,
alternative selection, missing placements, exact instance limits and a 2,000-level graph.
The product corpus now contains eleven schema-accepted fixtures with six accepted, four
rejected and one unsupported product outcomes. Its checker verifies numerical values.
