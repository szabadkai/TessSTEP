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
