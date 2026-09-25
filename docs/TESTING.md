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
