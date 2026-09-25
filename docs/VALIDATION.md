# Milestone 4 structural decoder validation — 2026-09-25

Milestone 4 now delivers the supported structural-decoding scope described in
[DECODING.md](DECODING.md). Full EXPRESS rule/algorithm evaluation and AP conformance
remain unsupported. Milestone 5 product/representation/unit/assembly semantics is next.

Observed on macOS arm64, rustc/cargo 1.95.0:

| Check | Observed result |
|---|---|
| Workspace locked debug and release tests | passed; final model changes also passed focused debug/release tests |
| Decoder regression tests | 13 passed, including complex/diamond membership, SELECT tags, numeric/enum/logical equality, bounds, limits and malformed metadata |
| Generated metadata and validator consumers | compiled and ran; valid/invalid instances, JSON status/exit policy and exact generation output budgets passed |
| Authored schema corpus through generated executable | all six reviewed outcomes matched: 2 accepted, 3 rejected, 1 unsupported |
| Shared decoder mutation smoke | 11,354 bounded cases passed |
| Decoder libFuzzer driver smoke | built offline; 1,000 runs completed without crashes |
| Workspace and fuzz formatting/Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture, conformance, fixture provenance and CLI checks | passed; 9 packages, 33 authored fixtures |
| Python corpus/CI tests | 19 passed |
| Relocated release package | passed; generated-validator flag, C/C++ consumers and ABI checks included |

The libFuzzer driver used stable Rust without coverage/sanitizer instrumentation; runtime
warnings confirmed this limitation. Nightly CI now includes the instrumented decoder
target, but its hosted run, extended fuzzing, other operating systems and the MSRV were
not observed locally. These are regression checks, not industrial-hardening claims.

## Corpus stages and outcomes

`python3 scripts/corpus.py --check` passed. Final run
`2026-09-25T16-59-17-593268Z` examined **6,438 paths / 3,227 unique contents** in
32.041 seconds. All per-file stage counts, representative cases and prior/baseline
comparisons were inspected in [the report](../reports/corpus/index.html).

- 2,821 inputs: physical syntax accepted and references resolved.
- 38 inputs: physical syntax accepted with missing references.
- 368 inputs: structured rejection.
- Zero crashes, timeouts or runner errors.

**No outcome or diagnostic changes** from the preceding run or reviewed baseline.
`corpus/baseline.json` remains unchanged. Without supplied AP metadata, all external
inputs still show schema/geometry/tessellation as `not_implemented` in this default run.

The separate authored [schema report](../reports/schema/index.html), run
`2026-09-25T16-44-26-716422Z`, verifies the real configured schema stage: `simple.step`
and `complex.step` are accepted; bad SELECT, duplicate aggregate and missing-reference
fixtures are rejected with the expected typed errors; parameterized DATA is unsupported.
All six physically parse. These schema outcomes are checked against reviewed expectations
in `corpus/manifest.json`, not inferred from physical success. Configured-validator hash,
schema identity, structured errors and per-file stages are retained in the report.

## Decoder benchmark

`cargo bench -p tessstep-model --bench decoder --locked` used the same 486,831-byte /
20,000-entity cyclic-reference fixture after physical parsing, with one warmup and a
two-second sample. Result: **158 iterations / 2.011 s / 12.730 ms per input /
1,571,033 entities per second**. View/index allocation and destruction are included.
The earlier simple-decoder observation was 5.242 ms; the expanded hierarchy/membership
checks and storage add measurable cost. This single local run is not a statistical
performance guarantee or an AP-scale benchmark. Optimization and memory profiling
remain future work; no linear complexity or exact RSS claim is made.

---

# Public C/C++ document interface validation — 2026-09-25

Observed on macOS arm64 with Rust 1.95.0 and Apple Clang 21. ABI 1 exposes physical
buffer parsing, document inspection and independent reference diagnostics only.

| Check | Observed result |
|---|---|
| Workspace locked tests and warnings-denied Clippy | Passed |
| Bridge debug/release tests | 5 passed: ownership/report lifetime, all ten budgets, cleared outputs, frozen layouts, Send/Sync and panic containment |
| Installed C11/C++17 consumers | Passed Debug and Release after relocation; Cargo/rustc/rustup invocations blocked |
| C-only CMake project | Passed without enabling C++ |
| Native consumer ASan/UBSan | Passed; Rust library itself was not sanitizer-instrumented |
| ABI exports and native runtime | Exactly 12 C symbols, no Rust exports; macOS identity uses @rpath and runtime dependency is libSystem |
| Architecture/conformance/Python tests | Passed; 16 Python tests |
| Release packaging | macOS arm64 archive and checksum built with relocated consumer gates |

C consumers exercise null/empty arguments, parse/unsupported/limit/not-found statuses,
independent reports after document release, retained document text pointer stability,
source-order inspection and frozen ABI layouts. C++ consumers exercise moves, destruction,
owned diagnostics, construction failures, typed results and simultaneous immutable reads.
Runtime identity inspection caught and fixed a build-path install name; the check now
requires a relocatable macOS identity and an ELF SONAME on Linux.

`python3 scripts/corpus.py --check` covered 6,438 paths / 3,227 unique inputs:
2,821 clean, 38 reference errors, 368 rejected; **no baseline changes**, crashes or
timeouts. The per-file progress report was inspected, including clean, missing-reference
and rejected examples. Schema/AP validation, geometry and tessellation are not established
by this physical-parser corpus run. This public interface does not expose those stages.
The corpus baseline was not refreshed.

Verification scope: the workspace test/Clippy pass above preceded concurrent changes
in the decoder. The final `cargo fmt --all -- --check` was blocked because that work
references `crates/tessstep-model/src/decode/select.rs`, which was not yet present.
Bridge-only formatting, architecture/conformance checks, and relocated package tests
passed afterward. No concurrent decoder edits were reverted or filled in here.

Linux/Windows and macOS Intel consumers are wired into existing CI/release packaging;
they were not run locally. Static linkage, public schema decoding and zero-copy mesh
views remain unimplemented. No cross-platform or industrial-hardening result is claimed.

---

# Milestone 4 initial decoder validation — 2026-09-25

Milestone 4 is in progress. This slice adds structural decoding against explicitly
supplied reflection metadata, with borrowed views, single inheritance, primitive and
named domains, ARRAY/LIST/BAG bounds and local reference compatibility. It does not
claim complete schema validation. See [DECODING.md](DECODING.md).

Observed on macOS arm64 with rustc/cargo 1.95.0:

- Workspace locked debug and release tests passed; seven decoder tests include
  structural acceptance/rejection, exact source context, malformed metadata and
  deterministic mutation smoke. The two latest decoder tests also passed in debug.
- The generated-metadata consumer compiled and ran: EXPRESS source → generated Rust
  reflection → physical instance decoding, with a negative type case.
- Workspace Clippy (all targets/features, warnings denied), formatting, rustdoc with
  warnings denied, architecture checker, conformance evidence and diff whitespace passed.
- Decoder benchmark: 486,831 physical source bytes / 20,000 pre-parsed entities with
  cyclic references; one warmup and 382 iterations over 2.003 seconds. Observed
  **5.242 ms/input / 3,815,003 entities per second**. View/index allocation and destruction
  are timed; physical parsing is excluded. This is a local observation, not a guarantee.

`python3 scripts/corpus.py --check` passed: run `2026-09-25T12-19-02-233794Z`,
**6,438 paths / 3,227 unique inputs**, 31.593 seconds. All per-file stages and both
previous-run/baseline comparisons were inspected, including representative accepted,
missing-reference and rejected inputs in the [progress report](../reports/corpus/index.html).

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, references resolved |
| 38 | physical syntax accepted, missing references |
| 368 | structured rejection |
| 0 | crashes, timeouts or runner errors |

**No outcome or diagnostic changes** versus the preceding run or reviewed baseline.
`corpus/baseline.json` was not changed. All 3,227 external inputs still report
`schema`, `geometry` and `tessellation` as `not_implemented`: the external runner has no
supplied AP metadata or schema-decoding CLI integration. The authored generated consumer
checks the new library stage separately; physical acceptance is not schema success.

Complex/multiple-inheritance mappings, SELECT/typed values, uniqueness, expression
semantics and generated owned-record conversion remain open for Milestone 4. Extended
instrumented decoder fuzzing, hosted platform CI and the MSRV run were not observed.
Concurrent C/C++ interface work in the shared workspace is outside this decoder slice.

---

# Milestone 3 validation — 2026-09-25

Environment: macOS arm64, Homebrew rustc/cargo 1.95.0. Milestone 3 adds deterministic
Rust generation and standalone static schema reflection. Generated records/references
are unchecked owned representations; physical schema decoding remains Milestone 4.

| Check | Observed result |
|---|---|
| Workspace debug and release tests, locked | passed; 50 integration tests and 2 doctests |
| Generated Rust consumer | compiled and ran against only the reflection runtime; unrelated-reference assignment correctly failed compilation |
| Generation determinism and limits | passed; repeat bytes, exact output boundary, invalid compilation and work limit |
| Workspace debug/release builds | passed |
| Workspace/fuzz formatting and Clippy, all targets, `-D warnings` | passed |
| Rustdoc with warnings denied | passed |
| Architecture checker | passed; 8 active packages, no third-party production dependencies |
| Authored corpus provenance | passed; 26 fixtures |
| Independent Part 21 / EXPRESS CLI checks | passed |
| Conformance table and test evidence | passed |
| Python corpus/CI runner tests | 16 passed |
| Codegen stable fuzz smoke | passed; 9,395 bounded prefix/mutation/arbitrary-input cases |
| Codegen libFuzzer driver smoke | built; 1,000 runs completed without crashes |
| Relocated release package smoke | passed, including `expressc --rust` output |

Consumer coverage includes imports/aliases, cross-schema inherited anonymous domains,
diamond deduplication, recursive typed references and upcasts, enum/SELECT construction,
aggregate/optional mappings, metadata identities/spans, abstract/supertype flags,
WHERE/UNIQUE/DERIVE/INVERSE source, constants and opaque algorithms. No generated
expression is executed. Architecture review is recorded in ARCHITECTURE.md.

The local libFuzzer driver used stable Rust without coverage or sanitizer instrumentation;
runtime warnings confirmed that limitation. Nightly CI now includes `schema_codegen`.
Hosted Linux/Windows/macOS CI, Rust 1.85 MSRV, dependency audits and instrumented/extended
fuzzing were not observed here; rustup is unavailable locally. These checks do not establish
industrial hardening, full EXPRESS conformance or AP schema coverage.

## Milestone 3 external corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T12-03-06-207101Z` examined
**6,438 paths / 3,227 unique contents** in 33.556 seconds. The per-input progress data,
diagnostics, all stage counts and both prior-run/baseline comparisons were inspected.

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, references resolved |
| 38 | physical syntax accepted, missing references |
| 368 | structured rejection |
| 0 | crashes, timeouts or runner errors |

**No outcome or diagnostic changes** from the prior run or reviewed baseline.
`corpus/baseline.json` was not changed. The local [per-file report](../reports/corpus/index.html)
and [JSON](../reports/corpus/latest.json) retain every input result.

The external `schema`, `geometry` and `tessellation` stages remain `not_implemented` on
all 3,227 inputs. Generating schema-source bindings does not validate physical instances.
EXPRESS expression/algorithm semantics, EXTENSIBLE/BASED_ON types, attribute redeclaration,
AP support and the public C/C++ interfaces also remain unimplemented.

## Milestone 3 generator benchmark

`cargo bench -p tessstep-codegen --bench generator --locked`: optimized build, precompiled
IR from **223,944 source bytes / 5,001 declarations**, one warmup and a two-second sample.
Generation allocates and drops the output within the measurement; input construction and
EXPRESS compilation are excluded. The final implementation produced **9,836,240 bytes**
per iteration: **216 iterations, 9.29 ms/input, 1,009.54 generated MiB/s**. This measures
source emission, not rustc throughput, AP-scale compatibility or memory usage. It is a
local observation, not a statistical performance guarantee.

**Next: Milestone 4 — schema-aware instance decoding and validation.**

---

# Milestone 2 validation — 2026-09-25

Environment: macOS arm64, rustc/cargo 1.95.0. Milestone 2 adds the EXPRESS declaration
frontend and `expressc`; geometry and schema-aware STEP instance validation remain future
work. No ISO/AP schema was bundled or manually transcribed.

| Check | Observed result |
|---|---|
| Workspace build and locked tests | passed; 46 integration tests and 2 doctests |
| Workspace/fuzz formatting | passed |
| Workspace clippy, all targets/features, `-D warnings` | passed |
| Fuzz clippy, all targets, `-D warnings` | passed |
| Workspace rustdoc, `RUSTDOCFLAGS=-D warnings` | passed |
| Architecture / unsafe-boundary checker | passed, 6 active packages |
| Authored corpus provenance | passed, 26 fixtures (20 Part 21, 6 EXPRESS) |
| Part 21 and EXPRESS independent CLI JSON checks | passed, including exit policy, escaping and determinism |
| Generated conformance table / test evidence | passed |
| Python corpus-runner tests | 7 passed |
| EXPRESS stable fuzz smoke | passed, 12,285 prefix/mutation/arbitrary-input exercise calls |
| EXPRESS libFuzzer driver smoke | built and completed 1,000 runs without crashes |

The EXPRESS smoke harness exercises lexing, compilation, determinism and diagnostic
spans under finite budgets; invalid UTF-8 is skipped because the library accepts `&str`.
The local libFuzzer driver was built with stable Rust, without coverage/sanitizer
instrumentation; runtime warnings confirmed that limitation. Nightly instrumented CI
now includes the EXPRESS target but has not been observed running here. Hosted platform
CI, the 1.85 MSRV job, advisory/license audits and extended fuzzing remain unobserved
locally. This is regression evidence, not industrial hardening.

## Milestone 2 external corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T11-40-21-681116Z` examined
**6,438 paths / 3,227 unique contents** in 32.921 seconds. The per-file progress report,
stage counts, representative accepted/rejected/reference-error cases, and both previous
run and reviewed-baseline comparisons were inspected:

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, references resolved |
| 38 | physical syntax accepted, missing references |
| 368 | structured rejection |
| 0 | crashes, timeouts or runner errors |

**No outcome or diagnostic changes** from the previous run or reviewed baseline.
`corpus/baseline.json` was not changed. Reports remain local generated artifacts:
[per-file progress](../reports/corpus/index.html) and [JSON](../reports/corpus/latest.json).

The external `schema`, `geometry` and `tessellation` stages remain `not_implemented` for
all inputs. The schema stage refers to validation of physical STEP instances, which is
Milestone 4. `scripts/check_express.py` separately verifies source declaration parsing,
basic schema compilation, import resolution and explicit unsupported semantics over
original `.exp` fixtures. The frontend does not turn STEP physical acceptance into
schema conformance. EXPRESS expression/algorithm semantics, EXTENSIBLE/BASED_ON types,
attribute redeclaration and full schema conformance remain unsupported as documented.

## Milestone 2 compiler benchmark

`cargo bench -p tessstep-express --bench compiler`: optimized build, one warmup and a
two-second sample per phase; **233,946 bytes, 5,000 entities and one type**. Input generation
is outside timing; token/AST/IR allocation and destruction are included.

| Phase | Iterations | Mean ms/input | MiB/s |
|---|---:|---:|---:|
| EXPRESS lexer | 539 | 3.72 | 60.05 |
| Declaration AST | 79 | 25.35 | 8.80 |
| Full basic compilation | 67 | 30.00 | 7.44 |

This is a local throughput baseline, not a statistical comparison, AP-scale benchmark,
or memory profile. The source-order IR and import graph are deterministic for repeated
identical ordered inputs. Architecture review is recorded in ARCHITECTURE.md.

**Next: Milestone 3 — deterministic Rust bindings and reflection.**

---

# Milestone 1 validation — 2026-09-25

Environment: macOS arm64, Homebrew rustc 1.95.0 / cargo 1.95.0, edition 2024.
This records observed results, not a full ISO or industrial-readiness certification.

## Quality checks

| Check | Result |
|---|---|
| `cargo build --workspace --locked` | passed |
| `cargo test --workspace --locked` | 33 integration tests and 1 public API doctest passed |
| workspace and fuzz `cargo fmt -- --check` | passed |
| workspace clippy, all targets/features, `-D warnings` | passed |
| fuzz-package clippy, all targets, `-D warnings` | passed |
| rustdoc, `-D warnings` | passed |
| architecture / unsafe-boundary checker | passed, 4 active packages |
| authored corpus provenance checker | passed, 20 fixtures |
| independent CLI JSON / escaping / duplicate-header / determinism checks | passed |
| generated conformance table and named test evidence | passed |
| Python corpus-runner tests | 7 passed |
| handwritten unsafe / unimplemented macro review | none in production code |

The deterministic fuzz smoke exercised **44,220** seed, prefix, mutation and arbitrary-byte
cases. All three libFuzzer drivers built and completed **1,000 runs each** over the supplied
seed directories without crashes. These local driver runs were built with stable Rust:
they lacked coverage and sanitizer instrumentation and emitted warnings to that effect.
They do not replace the nightly instrumented sessions configured in CI. No long fuzzing
or memory-profiling campaign has been performed.

## External corpus

The repository's optional `~/step-corpus` was available. Ran:

```sh
python3 scripts/corpus.py --check
```

Run `2026-09-25T11-21-26-354686Z` examined **6,438 paths / 3,227 unique input contents**
in 31.632 seconds. The report and baseline comparison were inspected. Observations:

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, no missing references |
| 38 | physical syntax accepted, missing references reported |
| 368 | structured rejection |
| 0 | crashes, timeouts, or runner errors |

There were **no changes from the reviewed baseline or previous run**. The baseline was
not refreshed. Representative rejections include invalid enumeration framing, malformed
typed parameters, duplicate IDs, nesting limits, overflow and invalid Unicode. An input
labelled as an encoding control contains an enumeration-wrapped typed value; the label
alone is not evidence that rejection is incorrect. These observations are compatibility
results, not an assertion that every rejected input has been independently adjudicated.

The searchable [per-file report](../reports/corpus/index.html) and
[machine-readable results](../reports/corpus/latest.json) remain local generated artifacts.
See [corpus-testing.md](corpus-testing.md) for baseline/report operation. External source
models were not copied into the repository. Schema, geometry and tessellation stages
remain `not_implemented`; no new semantic stage is claimed from this run.

## Benchmark

Command: `cargo bench -p tessstep-part21 --bench parser`. Optimized build, one warmup,
two-second samples, 957,938 input bytes and 20,000 entities. Fixture construction is
outside timing; token/event allocation, decoding and destruction are inside timing.

| Phase | Iterations | Mean ms / input | MiB/s |
|---|---:|---:|---:|
| Lexer | 154 | 13.07 | 69.89 |
| Parser events | 102 | 19.74 | 46.28 |

This is an initial local throughput baseline. It is not a statistical comparison,
a whole-document database benchmark, or a large-assembly memory measurement.

## Remaining verification and capability limits

Hosted Linux/Windows/macOS CI, the Rust 1.85 MSRV job, cargo-deny, advisory audits and
instrumented nightly fuzzing are configured but have not been observed running here.
Physical parsing does not validate declared implementation levels, schema-population
rules, entity attributes, inheritance, units or CAD semantics. Legacy scopes, archives,
BOM handling and tolerant syntax recovery are unsupported. External URI resolution and
cryptographic signature verification are not performed. The parser uses bounded logical
budgets and finite numeric types, not an exact process memory cap or arbitrary precision.
See CONFORMANCE.md for the feature matrix.

The record above describes the earlier Milestone 1 run. The Milestone 2 results at the
top of this document supersede its frontend status and current workspace test counts.
