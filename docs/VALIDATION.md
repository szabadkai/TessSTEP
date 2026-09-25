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
