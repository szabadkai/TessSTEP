# Corpus progress

Run from the repository root (Python 3.10+ and Rust are required):

```sh
python3 scripts/corpus.py --check
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

## What the results mean

| Observation | Meaning |
| --- | --- |
| `clean` | Physical syntax accepted; no missing references |
| `reference_errors` | Physical syntax accepted; some references are missing |
| `rejected` | Parser returned a structured diagnostic, including resource limits |
| `crash` | Process terminated by signal or Rust panic |
| `timeout` | Process exceeded the per-input deadline (30 seconds by default) |
| `runner_error` | I/O, CLI, output protocol or output budget failure |

Schema validation, geometry construction and tessellation remain explicitly
`not_implemented`. Physical acceptance does not establish correct CAD geometry.
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
JSON and JUnit artifacts. CI has 3,230 paths because local duplicate directory
copies are omitted. Unique content coverage is identical. See
[CI and releases](RELEASING.md) for downloads, retention and release gates.

## Configured schema stage (Milestone 4)

`--schema-validator EXE --schema-name NAME` enables structural checks using a validator
compiled from `expressc --validator` output. See [DECODING.md](DECODING.md) for building
and invoking one. Both flags are required together. Validators run only after physical
acceptance, with a separate timeout and 64 KiB output cap; JSON/exit/count inconsistencies
are runner errors. Executable hash and schema identity are recorded, and the validator
is snapshotted before the run. Schema acceptance regressions now fail `--check`.

Use a separate output/baseline for a configured schema corpus. Without these options,
the existing physical baseline and `schema: not_implemented` observations remain unchanged.
`python3 scripts/check_schema.py` builds an original tiny schema validator and checks six
reviewed positive/negative/unsupported fixtures through the real runner. Expected outcomes
and provenance are in `corpus/manifest.json`; the per-file report is `reports/schema/`.
This establishes a structural schema stage, not AP conformance or geometry support.
