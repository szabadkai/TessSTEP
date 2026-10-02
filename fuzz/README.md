# Fuzzing

Development-only workspace, excluded from production builds. `libfuzzer-sys` supplies
the engine; TessSTEP supplies the parsers and shared finite budgets.

Every target's body is a shared exercise function (`harness.rs`, `support/*.rs` or a
crate's `tests/support/`), so stable tests run exactly the code libFuzzer runs. A new
target follows the same pattern and needs an entry in `TARGETS` and `SMOKE` of
`scripts/fuzz_seeds.py` and in `TARGETS` of `crates/tessstep/tests/fuzz_regressions.rs`;
`python3 -m unittest discover -s scripts -p "test_*.py"` and the replay test fail until
it has them.

Stable deterministic regression smoke (part of `cargo test --workspace`):

```sh
cargo test -p tessstep-model --test fuzz_smoke -- --nocapture
cargo test -p tessstep-express --test fuzz_smoke -- --nocapture
cargo test -p tessstep-codegen --test generated codegen_fuzz_smoke -- --nocapture
cargo test -p tessstep --test fuzz_regressions -- --nocapture
cargo check --manifest-path fuzz/Cargo.toml --locked
```

`python3 scripts/fuzz_report.py smoke` runs all fourteen stable smoke tests named in
`SMOKE` and writes `reports/fuzz/smoke/report.json`. Run the `cargo check` from a
checkout that is not nested inside another Cargo workspace.

Directly running a stable-built driver executes inputs but has no sanitizer/coverage
instrumentation. Such runs must be reported as driver smoke only;
`scripts/fuzz_report.py` marks them `instrumented: false`.

## Coverage-guided fuzzing

Requires rustup, nightly Rust, and cargo-fuzz. Assemble each target's seeds into a
scratch directory first: libFuzzer writes new inputs into its first corpus directory,
and `corpus/geometry` also holds generator scripts and JSON.

```sh
cargo install cargo-fuzz --locked
python3 scripts/fuzz_seeds.py --output target/fuzz-seeds
mkdir -p target/fuzz-corpus/part21_parser
cargo +nightly fuzz run part21_parser target/fuzz-corpus/part21_parser target/fuzz-seeds/part21_parser -- -max_total_time=120 -max_len=65536
```

`scripts/fuzz_seeds.py` builds a deterministic seed corpus per target from committed
files and writes `target/fuzz-seeds/<target>.json` listing every seed's source, size
and hash, plus skipped files (duplicates, or longer than the target's `-max_len`):

| Targets | Seeds | `-max_len` |
| --- | --- | ---: |
| `part21_lexer`, `part21_parser`, `entity_database` | every `.step` in `corpus/part21`, `corpus/schema`, `corpus/product`, `corpus/geometry` | 65,536 |
| `express_frontend`, `schema_codegen` | every `.exp` in `corpus/express`, `corpus/schema`, `corpus/product`, `corpus/geometry` | 8,192 |
| `schema_decoder` | `corpus/schema/*.step` | 65,536 |
| `product_adapter` | `corpus/product/*.step` | 65,536 |
| `tessellated_import`, `presentation_import`, `brep_import` | `corpus/geometry/*.step` | 65,536 |
| `math_primitives` | generated: affine identity, reflection/shear, singular and near-singular matrices, repeated boundary values | 144 |
| `analytic_curves`, `nurbs_curves`, `surface_evaluators` | generated: the stable smoke tests' seeds plus conic, supplied-knot and analytic layouts and repeated boundary values | 128 |
| `brep_pipeline` | generated: the six constructed fixtures unmutated and zero-padded (the stable test's prefixes), and the closed cylinder with each mutation kind | 96 |

Every file in `regressions/<target>/` is also a seed of its target. `-max_len` is the
harness's input budget or the bytes it consumes. Per-input limits: `-timeout` 10 s
(30 s for the three import targets and `brep_pipeline`), `-rss_limit_mb` 2048 and
`-malloc_limit_mb` 512 for text inputs, 1024 and 256 for the fixed-size numeric
harnesses. The input budgets bound work and memory far below these, so hitting one is a
finding. The limits live in `TARGETS`.

## Nightly CI

`.github/workflows/nightly.yml` (also `workflow_dispatch`, with a `budget` in seconds,
default 240, maximum 900, and an optional comma-separated `targets` subset):

1. `fuzz-build` reads the targets from `fuzz/Cargo.toml` (`fuzz_seeds.py --plan`),
   requires an up-to-date `fuzz/Cargo.lock`, and builds every target once with
   `cargo fuzz build --sanitizer address --debug-assertions` and line tables.
2. `fuzz` is a matrix with one job per target. `fuzz_report.py run` assembles the
   seeds, runs the binary for the budget with the limits above, then minimizes each
   failing input with libFuzzer's `-minimize_crash=1 -runs=255` loop (what `cargo fuzz
   tmin` runs; without `-max_len`, which that loop rejects) and replays the result. A
   minimized input that no longer fails is discarded. The job fails on a finding, on a
   run error, or when the log shows no coverage counters or the binary has no
   AddressSanitizer runtime.
3. `fuzz-smoke` runs the stable smoke tests and the committed-finding replay.
4. `fuzz-report` aggregates everything, writes the job summary and fails unless every
   selected target ran instrumented and clean and every smoke test passed.

Artifacts: `fuzz-report-<target>` holds `<target>/report.json`, `run.log`,
`seeds.json`, the minimize and replay logs and `findings/` (libFuzzer's `crash-`,
`timeout-`, `oom-` or `leak-` file and its `minimized-` counterpart);
`fuzz-report-smoke` the smoke report; `fuzz-summary` holds `summary.json` and
`summary.md`. Each instrumented row records `instrumented: true` (coverage counters in
the log and the AddressSanitizer runtime in the binary), the libFuzzer seed, duration,
executions, initial and final coverage (`cov`) and features (`ft`), corpus units, seeds,
new corpus files, peak RSS and findings. Smoke rows are `instrumented: false`. A finding
is also an error annotation on the run page naming the artifact and the failure.

## Turning a finding into a regression test

1. Download `fuzz-report-<target>` and take `<target>/findings/minimized-<artifact>`
   (or the original artifact when it was not reduced). Reproduce it with
   `cargo +nightly fuzz run <target> <file>`, or on stable with step 3.
2. Fix the cause. When the fix belongs to a crate, add a focused test there too.
3. Commit the input as `fuzz/regressions/<target>/<short-description>` and run
   `cargo test -p tessstep --test fuzz_regressions`. The test replays every committed
   file through its target's exercise function and names each one that fails; a
   directory that names no target fails the test. Nightly runs also use these files as
   seeds.

Timeout and out-of-memory findings replay on stable for panics only; their time and
memory bounds are checked by the instrumented run, which seeds them again.

## Targets

The schema-codegen target compiles bounded EXPRESS input and checks deterministic
generation/error handling and output size; it does not invoke rustc per fuzz case.

The EXPRESS target exercises lexing and basic compilation under small finite budgets,
checks deterministic results and valid diagnostic spans, and skips non-UTF-8 bytes
because the library API accepts `&str`.

The [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/tutorial.html)
describes the cargo-fuzz driver and crash-minimization workflow.

`schema_decoder` uses generated metadata from `corpus/schema/sample.exp` and the shared
bounded harness in `support/decoder.rs`. Seed with `corpus/schema`; stable mutations run
in the model crate's `decoder_fuzz` test. Regenerate `support/decoder_schema.rs` with
`cargo run -p expressc -- --rust corpus/schema/sample.exp` when the schema changes; the
codegen test checks this exact output.

The `product_adapter` target (`support/product.rs`) parses, structurally decodes with
the original product test schema and adapts bounded graphs twice to check determinism.
Seed with `corpus/product`. Generated metadata is shared with adapter consumer tests and
verified by `scripts/check_product.py`. It does not fuzz a complete AP schema.

`math_primitives` uses the same arbitrary IEEE-754 bit-pattern harness as the independent
math crate's stable smoke test. Each input consumes at most 144 bytes, with fixed-size
inversion, frame and coordinate operations. Successful finite results and repeatable
inverse failures are checked; this is not an exact arithmetic oracle or an industrial
numerical-hardening claim.

`analytic_curves` exercises analytic curve values through a fixed-size
3D harness: axes, positive lengths, arbitrary parameters and oriented spans. It shares
its 128-byte, bounded-work exercise with the stable curve smoke test. Successful outputs
must be finite and repeatable; this is not an exact numerical oracle.

`nurbs_curves` and `surface_evaluators` share the stable bounded spline/geometry
harnesses and consume at most 128 bytes. Inputs cover malformed knots/degrees, arbitrary
finite/non-finite coordinates, weights, parameters, evaluation and refinement. Small
logical limits bound allocations. Seeded stable mutation tests supplement arbitrary-bit
inputs; none is an exact arithmetic oracle.

`brep_pipeline` mutates original constructed topology fixtures, index references,
orientations, memberships and arbitrary-bit edge/pcurve intervals. Successful structural
validation feeds bounded trim reconstruction, shared-edge sampling and per-face UV
mapping, planar triangulation and adaptive mesh assembly with index/finite checks.
Six fixtures include a capped cylinder and a NURBS bump. Inputs are limited to 96 bytes;
the stable test runs 3,000 random mutations plus 582 fixture prefixes. It has no STEP
parser or external corpus dependency.

`tessellated_import` (`support/tessellated.rs`) parses bounded Part 21 input and imports
root `#1000` as an existing tessellation with small decoding, adapter and mesh budgets,
requiring identical results from two runs. The stable
`tessellated_budgets_order_and_mutation_smoke` test runs 300 seeded byte mutations of
the tessellated box with larger budgets.

`presentation_import` (`support/presentation.rs`) imports root `#300` as a presentation
tessellation, as in the authored annotation fixture, and checks that every published
index addresses the published positions.

`brep_import` (`support/brep.rs`) parses bounded Part 21 input, runs root discovery, and
imports and tessellates root `#1000` with the curved B-rep profile under small finite
budgets, requiring deterministic outcomes and watertight meshes with positive volume and
one component per shell: a `BREP_WITH_VOIDS` cavity meshes as an inward component. The
first committed regression, `regressions/brep_import/sphere-cavity-second-component.step`
(the authored `brep-sphere-cavity.step`), failed the target's earlier one-component
`require_solid` check. The stable `brep_budgets_record_order_and_mutation_smoke` test
runs 450 seeded byte mutations of three curved fixtures.
