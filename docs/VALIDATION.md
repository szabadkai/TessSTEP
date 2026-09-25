# Milestones 11–13 boundary pipeline validation — 2026-09-25

The existing Milestones 8–10 work was checkpointed as `52c752c` after the workspace
suite passed. This delivery adds independent structural topology states (11), bounded
supplied-pcurve UV reconstruction (12), and shared-edge boundary sampling (13).
Contracts and limits are in [TOPOLOGY.md](TOPOLOGY.md), [TRIMMING.md](TRIMMING.md),
and [TESSELLATION.md](TESSELLATION.md). Planar face triangulation is next; no STEP
geometry adaptation, triangle mesh or watertight-solid result is claimed.

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | 146 passed, including doctests |
| New topology/trim/tessellate release tests | 23 passed, including two compile-fail doctests |
| Workspace/fuzz formatting and Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture/conformance/authored fixture provenance | passed; 17 packages and 40 fixtures |
| Python corpus/CI tests | 21 passed |
| Relocated C/C++ package consumers | passed; Debug/Release, ABI exports/layout, ownership and concurrency |
| Stable combined boundary mutation harness | 3,000 random mutations plus 388 prefixes passed |
| New libFuzzer driver | built; 1,000 smoke runs completed without crashes |

Topology tests check shared identity without geometry edits, malformed references and
ownership, endpoint mismatch, wire closure, closed/open/disconnected shells, conflicting
orientations, excess edge incidence and pinched vertex links. UV tests exercise outer
and hole classification, analytic and NURBS boundaries, full cylinder seams, integer
chart shifts, reversed parameter intervals, one-sided discontinuities, singular surfaces,
missing/mismatched pcurves, invalid winding and noncontractible loops. Resource limits
are failure paths, not successful partial output.

Sampling tests verify zero-copy reversed/shared/seam views, exact canonical vertex
coordinates, circle sagitta/tangent criteria, tighter-tolerance refinement, rational
arc dense-probe error, deterministic output, NURBS knot corners/discontinuities,
collapsed curves, endpoint tolerance failures and face-specific UV mappings.
These checks establish the documented constructed-input behavior, not global continuous
error bounds, exact intersection predicates or AP conformance.

The libFuzzer binary used stable Rust and four original 96-byte seeds. It warned about
missing sanitizer/coverage instrumentation and no interesting coverage; the 1,000 runs
are driver smoke evidence only. Full-size arbitrary mutations are covered separately by
the stable harness. An instrumented nightly CI target is configured but was not observed
here. Hosted cross-platform/MSRV CI and long numerical-hardening campaigns remain open.

## External corpus

Both `python3 scripts/corpus.py --check` runs passed; the reviewed baseline was unchanged:

| Checkpoint | Run | Seconds |
| --- | --- | ---: |
| Initial integrated boundary pipeline | 2026-09-25T18-36-39-590816Z | 33.857 |
| Final boundary pipeline | 2026-09-25T18-46-06-520711Z | 32.926 |

Each report contains **6,438 paths / 3,227 unique inputs**: **2,821 clean**, **38 with
reference errors**, and **368 structured rejections**. Outcome/diagnostic change lists
against the previous run and reviewed baseline are empty. There were no crashes,
timeouts or runner errors. Per-file stage counts and representative clean,
reference-error and rejected entries were inspected in the final report.

All 3,227 external inputs still report schema/product/geometry/tessellation as
`not_implemented`: no AP validators are configured and no STEP geometry adapter/checker
exists. Independent boundary tests are not relabeled as STEP geometry acceptance.
See the generated [per-file HTML report](../reports/corpus/index.html) and
[JSON results](../reports/corpus/latest.json). `corpus/baseline.json` was not refreshed.

## Benchmark and architecture review

`cargo bench -p tessstep-tessellate --bench boundaries --locked` uses an original
cylinder-strip fixture, one warmup and one-second samples. Geometry construction is
outside timing; raw-model cloning is included in validation timing.

| Operation | Iterations | Microseconds/iteration |
| --- | ---: | ---: |
| Structural validation and normalization | 1,535,261 | 0.651 |
| UV seam reconstruction | 265,717 | 3.763 |
| Shared-edge sampling and face boundary mapping | 4,825 | 207.265 |

These are local observations, not statistical comparisons or whole-model tessellation
measurements. The architecture review confirms the reserved downward crate graph,
immutable private validity states, explicit model/cache borrows, deterministic iterative
walks, logical resource budgets and quadratic bounded polygon comparisons. No unsafe
kernel code, third-party production dependency or C ABI symbol/layout was introduced.
Public C/C++ geometry and mesh APIs remain pending under the existing release contract.

---

# Milestones 8–10 surface/NURBS validation — 2026-09-25

The verified analytic-curve milestone was committed as `7758269`. This work implements
independent analytic surfaces (8), bounded positive-weight NURBS curves (9), and
bounded tensor-product NURBS surfaces (10). See [SURFACES.md](SURFACES.md) and
[NURBS.md](NURBS.md) for parameterizations, published algorithms and supported limits.
STEP geometry adapters, topology, trimming and tessellation remain pending.

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed, 123 tests including doctests |
| Curve/surface release tests | passed, 27 tests including doctests |
| Workspace/fuzz formatting and Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture/conformance/authored fixture provenance | passed; 14 packages, 40 fixtures |
| Python corpus/CI tests | 21 passed |
| Relocated C/C++ package consumers | passed; Debug/Release, ABI layout/exports, ownership and concurrency |
| Two new stable bounded geometry/spline smoke harnesses | 5,641 cases each: 5,000 arbitrary-bit, 512 mutations, 129 prefixes |
| Two new libFuzzer driver smoke runs | 1,000 runs each, no crashes |

Analytic tests independently compare all first/second partials with finite differences,
check known points/loci, parameter domains, periodic seams and exact pole/apex singularities.
NURBS tests include a rational quarter circle and cylinder, hand-calculated weighted
bilinear partials, mixed-derivative checks, one-sided repeated knots, nonclamped/periodic
representations, degree 16, affine/weight-scale invariance and refinement preservation.
Invalid shapes, knots, counts, weights, domains and logical resource budgets are tested.
Surface refinement with unequal weights across rows verifies that normalization remains
common to the whole net. A checked rectangular layout prevents dimension-product overflow.

Both libFuzzer drivers were compiled with stable Rust and each loaded one 128-byte
seed. They completed but reported missing coverage/sanitizer instrumentation and no
interesting coverage. These are driver smoke checks only; full-size mutation/random
inputs are exercised independently by the stable harnesses. Nightly CI adds instrumented
targets and increases its budget to 35 minutes for the longer target list. Hosted CI,
Rust 1.85, long instrumented campaigns and cross-platform numerical behavior were not
observed locally. No unsafe code, external production dependency or C ABI layout was added.

## External corpus checks

Ran `python3 scripts/corpus.py --check` after the analytic surface and rational curve
steps and after the combined implementation. Each run examined **6,438 paths / 3,227
unique contents** and passed without refreshing `corpus/baseline.json`:

| Stage checkpoint | Run | Seconds |
| --- | --- | ---: |
| Analytic surfaces | 2026-09-25T18-07-55-526622Z | 32.137 |
| Rational curve implementation | 2026-09-25T18-12-44-870866Z | 33.819 |
| Combined surfaces/NURBS | 2026-09-25T18-21-38-235130Z | 34.324 |

Per-file stage counts and comparison lists were inspected for all three runs; representative
clean/reference-error/rejected entries were also inspected in the final report. All runs
retain **2,821 clean**, **38 reference-error**, **368 rejected**, with **zero outcome or
diagnostic changes** versus the previous run and reviewed baseline. There were no crashes,
timeouts or runner errors. See [the final per-file report](../reports/corpus/index.html).

All external inputs still show schema/product/geometry/tessellation as `not_implemented`.
No AP schema/product checker is configured, and no STEP-to-geometry adapter/checker exists.
Independent NURBS tests are not relabeled as STEP geometry acceptance. Real geometry stage
checks must be added with those adapters. Physical acceptance alone establishes none of
the later stages.

## Benchmarks and architecture

`cargo bench -p tessstep-surfaces --bench evaluators --locked` uses one warmup and
one-second samples of 1,000 checked evaluations per batch. Geometry construction is
outside timing; position and all implemented derivative calculations are inside:

| Evaluator | Batches | Microseconds/batch |
| --- | ---: | ---: |
| Analytic torus, position and five partials | 16,797 | 59.536 |
| Rational curve, position and two derivatives | 9,623 | 103.925 |
| Rational surface, position and five partials | 5,736 | 174.347 |

These are local throughput observations, not statistical comparisons, whole-model
benchmarks or numerical-accuracy certificates. The architecture review records shared
iterative basis evaluation, bounded immutable spline storage/refinement, fixed stack
scratch and no raw STEP dependency. Public C/C++ geometry operations remain pending
under the existing ownership/ABI/release-testing contract. Milestone 11 topology validity
states are next; documented Milestone 5 semantic gaps also remain open.

---

# Milestone 7 analytic curve validation — 2026-09-25

Milestone 6 was committed as `cd28385` before this work. The next milestone adds
`tessstep-curves`, depending only on math and reexported as `tessstep::curves`.
It provides independent 2D/3D lines and conics, first/second derivatives, domains,
explicit plane frames, affine evaluation transformation and oriented parameter spans.
See [CURVES.md](CURVES.md) for parameterizations and numerical limits. STEP geometry
adapters, surfaces, NURBS, topology and tessellation remain pending.

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed, 106 tests including doctests |
| Analytic curve debug/release tests | passed, 8 integration tests and 2 doctests |
| Frame-space compile-fail example | passed (one of the doctests) |
| Workspace/fuzz formatting and Clippy with warnings denied | passed |
| Workspace rustdoc with warnings denied | passed after fixing an interval rendered as a link |
| Architecture/conformance/fixture provenance | passed; 13 packages, 40 authored fixtures |
| Python corpus/CI tests | 21 passed |
| Relocated C/C++ installed-package consumers | passed; Debug/Release, ABI layouts/exports, ownership and concurrency |
| Deterministic curve arbitrary-float smoke | 10,000 random inputs plus 774 boundary/prefix inputs |
| Curve libFuzzer driver smoke | 1,000 runs completed without crashes |

Independent evidence includes known coordinates and derivatives, implicit conic loci,
central finite differences of positions and first derivatives, 2D orientation and 3D
plane membership, scale-first frames at subnormal/maximum magnitudes, reversed spans,
seam crossing, multiple revolutions, affine covariance and explicit failure cases.
Parabola/hyperbola parameterizations and ellipse axis order are documented and tested.
These are constructed-geometry tests; they do not establish STEP/AP interpretation.

The local libFuzzer driver used stable Rust, no coverage/sanitizer instrumentation and
an empty initial corpus. Runtime warnings confirm those limitations; the full-size
arbitrary-bit cases come from the separate stable harness. Nightly CI configures an
instrumented curve target. No hosted CI, Rust 1.85, extended instrumented fuzzing or
cross-platform numerical behavior was observed locally. No third-party production
dependency, unsafe code, public C symbol or ABI layout was added.

## External corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T17-53-15-066870Z` examined
**6,438 paths / 3,227 unique contents** in **33.837 seconds**. The generated per-file
report, all stage counts, comparison lists and representative clean/reference-error/
rejected entries were inspected. Outcomes remain **2,821 clean**, **38 reference-error**
and **368 rejected**, with **zero changes** against both the previous run and reviewed
baseline. There were no crashes, timeouts or runner errors. `corpus/baseline.json` is
unchanged. See [the external per-file report](../reports/corpus/index.html).

All 3,227 external inputs still report schema/product/geometry/tessellation as
`not_implemented`: no AP schema/product checker was configured, and independent curve
evaluation provides no schema-to-geometry adapter/checker. No geometry acceptance is
inferred from physical parsing. The geometry stage must receive a real checker when
STEP geometry adaptation becomes available.

## Analytic benchmark

`cargo bench -p tessstep-curves --bench analytic --locked` measured 10,000 ellipse
position/first/second-derivative evaluations per batch, with curve construction outside
timing and checked evaluations inside. One warmup and a two-second sample yielded
**6,596 batches / 2.000 seconds / 303.216 microseconds per batch**. This is a local
throughput observation, not a statistical comparison, whole-model benchmark or accuracy
certificate.

The architecture review in [ARCHITECTURE.md](ARCHITECTURE.md) records the independent
math-only dependency, private construction invariants, fixed stack/work costs and the
unchanged C/C++ boundary. Analytic surfaces are the next independent geometry milestone;
STEP curve/placement adapters and the remaining Milestone 5 semantics are still open.

---

# Milestone 6 math foundation validation — 2026-09-25

The verified Milestone 5 product/assembly slice was committed as `d84e5f3`.
The next milestone adds `tessstep-math`, an independent checked coordinate, unit,
tolerance and affine-transform layer, reexported as `tessstep::math`. Its scope and
numerical limitations are in [GEOMETRY.md](GEOMETRY.md) and
[NUMERICAL_ROBUSTNESS.md](NUMERICAL_ROBUSTNESS.md). STEP placement evaluation,
analytic curves/surfaces, NURBS, topology and tessellation remain future work.

Observed locally on macOS arm64, Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed, 96 tests including doctests |
| Math debug and release tests | passed, 8 integration tests and 3 doctests |
| Space separation and transform composition compile-fail examples | passed (2 of the doctests) |
| Workspace/fuzz formatting and Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture, conformance and authored-fixture provenance | passed; 12 packages, 40 fixtures |
| Relocated C/C++ installed-package consumers | passed, Debug/Release, layouts, exports, ownership and concurrency |
| Math seeded affine round trips | 2,000 diagonally dominant matrices, both inverse application orders |
| Deterministic arbitrary-float smoke | 10,000 arbitrary-bit cases plus 870 boundary/prefix cases |
| Math libFuzzer driver smoke | 1,000 runs completed without crashes |

The local libFuzzer driver used stable Rust without coverage/sanitizer instrumentation
and started from an empty corpus. Its warnings explicitly report missing instrumentation;
it is only a driver smoke check. The deterministic harness exercises full-size arbitrary
float inputs independently. Nightly CI configures the new instrumented target; its job
budget was increased to accommodate the added two-minute run. No hosted CI, Rust 1.85,
extended instrumented fuzzing or cross-platform numerical results were observed here.

## External corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T17-37-32-443182Z` checked
**6,438 paths / 3,227 unique contents** in **34.107 seconds**. The generated per-file
JSON report, stage counts and representative accepted/reference-error/rejected entries
were inspected: **2,821 clean**, **38 reference-error**, **368 rejected**, and no crashes,
timeouts or runner errors. Both previous-run and baseline comparisons contain **zero
outcome or diagnostic changes**. `corpus/baseline.json` was not modified.

All external inputs still report schema/product/geometry/tessellation as
`not_implemented`: no AP schema/product checker is configured, and math primitives do
not supply a STEP geometry validator. Existing authored product outcomes were rechecked
before the commit: three accepted, two rejected and one unsupported, all schema-accepted.
See [the external per-file report](../reports/corpus/index.html).

## Math benchmark

`cargo bench -p tessstep-math --bench transforms --locked` measured a known affine
inverse plus 10,000 checked point transformations per batch. Point allocation/construction
is outside timing; inverse calculation, result checks and point transformation are inside.
One warmup and a two-second sample yielded **47,964 batches / 2.000 seconds /
41.698 microseconds per batch**. This is a local observation, not a statistical performance
comparison, whole-model benchmark or certified numerical-accuracy measurement.

## Architecture review

The new crate has no dependencies, unsafe code, heap allocation or global mutable state.
Its types preserve finite storage and explicit frame/tolerance contracts; numerical
inversion failure remains separate from finite transform construction. Existing parser,
model, product and C ABI dependencies/layouts are unchanged. Geometry C/C++ operations
remain pending and must follow the existing public-interface contract.

---

# Milestone 5 initial product graph validation — 2026-09-25

Existing Milestone 4 work was committed as `4c4bdc6` after workspace tests, formatting,
Clippy, schema fixture and relocated C/C++ checks, and an unchanged external corpus run.
The subsequent Milestone 5 slice adds independent product graphs, schema-decoded
adapters, explicit unit scales, assemblies and preserved mapping/placement descriptions.
Milestone 5 remains in progress; the exact supported subset and remaining work are in
[PRODUCT_MODEL.md](PRODUCT_MODEL.md).

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed |
| Independent product and adapter debug/release tests | passed, including iterative 10,000-definition chain and numeric limits |
| Workspace and fuzz formatting/Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture, conformance and fixture provenance checks | passed; 11 packages and 40 authored fixtures |
| Python corpus/CI tests | 21 passed |
| Product fixture metadata regeneration and compiled checker consumers | passed |
| Deterministic product mutation smoke | 1,500 mutations; repeat results agree |
| Product libFuzzer driver smoke | 1,000 runs completed without crashes |

The libFuzzer driver was compiled with stable Rust without coverage/sanitizer
instrumentation; the runtime explicitly reported missing instrumentation. Hosted
nightly CI configures the instrumented target, but no hosted, extended fuzz or
cross-platform result was observed locally. The Rust 1.85 MSRV was not run here.
No production dependencies or public C ABI layouts were added.

## Product and external corpus observations

The authored product report, run `2026-09-25T17-18-57-773768Z`, was inspected per file.
All six inputs pass physical parsing and structural schema decoding:

- `assembly.step`, `complex.step`, `mapped.step`: accepted product graphs; repeated
  occurrences reuse definitions and representations, with ordered placement/map links.
- `cycle.step`: rejected with graph cycle error.
- `missing-units.step`: rejected with located missing length unit error at entity 23.
- `unsupported-context.step`: explicitly unsupported 2D context at entity 23.

These outcomes and expected graph counts are checked by `scripts/check_product.py`.
The original reduced test schema is not an ISO/AP schema. Geometry and tessellation
remain `not_implemented` for these fixtures. See [the product report](../reports/product/index.html).

`python3 scripts/corpus.py --check` passed: run `2026-09-25T17-24-51-515512Z` examined
**6,438 paths / 3,227 unique contents** in **32.341 seconds**. The report, all stage
counts and representative accepted, reference-error and rejected inputs were inspected.
There were **2,821 clean**, **38 reference-error** and **368 rejected** inputs, with
**zero outcome/diagnostic changes** versus both the previous run and reviewed baseline.
No crashes, timeouts or runner errors occurred. `corpus/baseline.json` is unchanged.

All external inputs remain `not_implemented` for schema and product stages because no
AP metadata/checker was configured; geometry and tessellation are also unimplemented.
The added product column normalizes absent older fields to `not_implemented` for
comparison, without changing the baseline. See [the external report](../reports/corpus/index.html).

## Product adaptation benchmark

`cargo bench -p tessstep-ap242 --bench product --locked` adapted a 540,877-byte,
10,034-entity document with 10,002 occurrences after physical parsing and schema
decoding. One warmup and a two-second sample produced **373 iterations / 2.003 s /
5.370 ms per adaptation**. Output allocations, graph validation and destruction are
included. This is one local observation, not a statistical comparison or a large
industrial assembly memory measurement.

---

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
