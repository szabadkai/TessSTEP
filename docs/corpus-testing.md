# Corpus progress

Run from the repository root (Python 3.10+ and Rust are required):

```sh
python3 scripts/corpus.py --check --repeat 2
```

This builds the release `stepdump`, discovers `.step`, `.stp` and `.p21` files
recursively under `~/step-corpus`, and checks each unique file content in a
separate process. It prints progress for every input. The corpus is read only;
archives are not extracted and external resources are not fetched. Directory
symlinks are not followed. Identical contents run once, with all file paths
retained in the report. This includes the actual inputs in `vendor/` even when
the curated `corpus/` directories are empty.

Open `reports/corpus/index.html` for the searchable per-file report, run history,
diagnostics and changes since the preceding run. Machine-readable results are
in `reports/corpus/latest.json`; complete timestamped snapshots remain under
`reports/corpus/runs/`. These generated files are ignored by Git. Every run
records source and executable hashes, input SHA-256 hashes, elapsed time,
entity counts and parser diagnostic codes. Runtime is observational and is not
a performance gate. Run only one corpus runner per output directory at a time.

Report format 2 adds source and stage coverage, field-level baseline differences,
expectation verdicts, diagnostic frequencies, timing percentiles and searchable
per-file evidence. Filter the offline HTML by source, physical outcome, verdict,
or stage outcome. Expand a file for aliases and diagnostics. `summary.md`,
`junit.xml` and `cases.csv` accompany every completed run. JSON retains the
individual measurements; CSV lists every path, including duplicate aliases.
Source totals assign each unique content to its first discovered source, so
shared copies do not inflate unique coverage.

## What the results mean

| Observation | Meaning |
| --- | --- |
| `clean` | Physical syntax accepted; no missing references |
| `reference_errors` | Physical syntax accepted; some references are missing |
| `rejected` | Parser returned a structured diagnostic, including resource limits |
| `crash` | Process terminated by signal or Rust panic |
| `timeout` | Process exceeded the per-input deadline (30 seconds by default) |
| `runner_error` | I/O, CLI, output protocol or output budget failure |
| `nondeterministic` | Repeated physical parsing produced different JSON bytes |

Without configured validators, schema/product validation is `not_configured`.
Geometry and tessellation are `not_integrated` in this runner; the workspace's
constructed-model tests are reported separately. A stage blocked by an earlier
failure is `not_run`. None of these are passes. Physical acceptance does not establish correct CAD geometry.
Rejection of a deliberately malformed fixture may be correct. The external
defect catalog includes schema, geometric and runtime expectations, so its
entries are not automatically converted into parser pass/fail assertions.
External references and cryptographic signatures are not verified.

## Baselines and implementation work

`corpus/baseline.json` is the reviewed observation baseline. `--check` fails if a
previously clean input becomes non-clean, a previously accepted input stops
parsing, an input disappears entirely, or any input crashes, times out or has a
runner error. Changed diagnostic codes and newly accepted inputs are reported
as changes, not automatically claimed as conformance improvements. Comparisons
use content hashes, so modified fixtures appear as removed and added inputs.
Duplicate aliases do not multiply test coverage; disappearing aliases of a
still-present input do not count as removed unique inputs.
Increased missing-reference counts, accepted schema/product stages becoming
non-accepted, and nondeterministic output also fail. Old `not_implemented` stage
labels are normalized for comparison without modifying the committed baseline.

After each implementation milestone, run the Rust tests and corpus check,
inspect changed files, and summarize the change in supported stages and outcomes.
As schema, geometry and tessellation become available, extend the runner with
real checks for those stages and reviewed per-fixture expected outcomes before
claiming coverage. Preserve the original baseline until changes are reviewed.
To explicitly accept a new observation baseline:

```sh
python3 scripts/corpus.py --save-baseline
```

Baseline replacement is refused if any input crashes, times out or encounters a
runner error. The baseline is an observed compatibility contract, not an oracle
for whether malformed files should be accepted.

## Reviewed expectations and generated corpus

The report separates `passed` (explicit expectations matched), `compatible`
(no observation-baseline regression), `failed` and `unreviewed`. New inputs
without an oracle are unreviewed even when a baseline exists. JUnit emits them
as skipped, while missing baseline inputs, runtime faults and mismatched
expectations fail. A reviewed malformed input passes only when its expected
rejection is observed.

```sh
python3 scripts/check_metamorphic.py
python3 scripts/check_schema.py
python3 scripts/check_product.py
python3 scripts/check_kernel.py
```

The generated suite creates 76 original fixture paths (74 unique byte streams)
with expectations defined independently of parser output. Graphs of 1–127
entities vary IDs, order, comments, Unicode, CRLF and DATA sections while keeping
known entity/reference properties. Additional cases cover missing references,
invalid IDs, numeric overflow/underflow, bad encodings, binary padding, duplicate
records, deep nesting, truncation and strings up to 65,536 characters. Two runs
must produce byte-identical physical-parser JSON. Fixtures are generated in a
temporary directory; per-case expectations and content hashes remain in
`reports/generated/`. No external download is needed.

To supply your own oracle, use `--expectations expected.json` containing
`{"cases": {"relative/file.step": {"status": "clean", "entity_count": 3}}}`.
Every discovered path must have a nonempty expectation. Supported checks are
`status`, `entity_count`, `entity_counts`, `missing_references`, exact
`diagnostic_codes`, a subset of `stages`, exact `schema_result`/`product_result`,
and `product_result_contains`/`first_relationship_matrix` for product numerical
checks. Product object subsets use exact array lengths and float tolerances of
1e-12 relative/absolute. Expectation mismatches fail regardless of `--check`.
The report records the expectation-file hash. `--repeat 2` repeats physical
parsing only; schema/product results have separate reviewed assertions.

`reports/kernel/` contains individual Rust test outcomes, JUnit and the full
log. The runner uses `--no-fail-fast` to collect other suites after a failure;
build failures and timeouts also fail the report. It includes constructed
geometry/mesh tests, authored integration tests and doctests. These results
must not be counted as successful external STEP geometry imports. The added
planar tessellation test checks 72 combinations of scale, translation, boundary
start and face orientation against independent area and edge-incidence checks.

Useful options:

```sh
python3 scripts/corpus.py --corpus /path/to/step-corpus --timeout 60
python3 scripts/corpus.py --output reports/experiment --baseline corpus/baseline.json
python3 -m unittest discover -s scripts -p 'test_*.py'
```

`TESSSTEP_CORPUS` also overrides the default corpus location. Alternate or
partial corpora should use a separate output directory and baseline. The large
external corpus is not required for ordinary `cargo test --workspace` runs.

Schema-source checks and physical-instance checks are separate. `check_express.py`
checks original source declarations; generated-consumer tests verify Rust bindings.
`check_schema.py` now verifies structural instance outcomes with supplied metadata.
No source compilation or schema success is inferred from a STEP header.

## GitHub Actions

Every PR and main push reconstructs all 3,227 unique inputs from pinned public
sources, runs the baseline check, and publishes an Actions summary plus HTML,
JSON, CSV and JUnit artifacts. CI has 3,230 paths because local duplicate directory
copies are omitted. Unique content coverage is identical. See
[CI and releases](RELEASING.md) for downloads, retention and release gates.
All three OS jobs also publish `test-reports-<os>` with authored schema/product,
generated transformations and per-test Rust evidence. Summaries and uploads run
after failures. Repeated parsing is enabled in both authored and external suites.

## Configured schema stage (Milestone 4)

`--schema-validator EXE --schema-name NAME` enables structural checks using a validator
compiled from `expressc --validator` output. See [DECODING.md](DECODING.md) for building
and invoking one. Both flags are required together. Validators run only after physical
acceptance, with a separate timeout and 64 KiB output cap; JSON/exit/count inconsistencies
are runner errors. Executable hash and schema identity are recorded, and the validator
is snapshotted before the run. Schema acceptance regressions now fail `--check`.

Use a separate output/baseline for a configured schema corpus. Without these options,
the existing physical baseline is unchanged and schema is reported as `not_configured`.
`python3 scripts/check_schema.py` builds an original tiny schema validator and checks six
reviewed positive/negative/unsupported fixtures through the real runner. Expected outcomes
and provenance are in `corpus/manifest.json`; the per-file report is `reports/schema/`.
This establishes a structural schema stage, not AP conformance or geometry support.

## Configured product stage (Milestone 5)

`--product-validator EXE` adds a `product` stage after the configured schema stage
accepts an input. It requires `--schema-validator` and uses the same `--schema-name`.
The checker runs in a separate process with its own timeout and 64 KiB JSON limit;
its scope must be `product-structure`, with the schema-validator status/exit/count
contract. The runner snapshots/hashes both executables and reports outcomes separately.
Product acceptance regressions fail `--check`.

`python3 scripts/check_product.py` generates metadata from an original reduced test
schema, verifies the checked-in test bindings, compiles two checkers and runs eleven
reviewed fixtures through physical, schema and product stages. See `reports/product/`.
Six product models are accepted, four rejected and one unsupported; all eleven pass
structural decoding. Accepted matrices, graph counts and uncertainty values are verified. These are test applications, not bundled AP validators.

Without a checker, product remains `not_configured`. Older unmeasured baseline
entries compare equivalently, avoiding artificial outcome changes. The physical
baseline is not rewritten. No semantic result is inferred for the external AP
corpus; its geometry and tessellation stages remain `not_integrated` in this runner.
