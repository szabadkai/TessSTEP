# Dependency and unsafe policy

All production crates currently depend only on std and local TessSTEP crates. Core
STEP, EXPRESS, geometry, topology, trimming and tessellation must be owned here; adding
an external CAD kernel is outside project policy. New substantial dependencies need a
written rationale, license review, security review and evidence that their responsibility
is generic infrastructure. Commit lockfiles for reproducibility.

The isolated `fuzz` package uses `libfuzzer-sys` for the fuzz engine and its build/runtime
dependencies. This is development infrastructure, never linked into the public library
or either CLI. The deterministic smoke harness and benchmarks themselves need no external
packages. The minimal std benchmark reports timing/throughput, not confidence intervals.

Every kernel crate root explicitly forbids unsafe code, reinforced by workspace
lints. The isolated `tessstep-capi` bridge is the audited exception, with
`deny(unsafe_op_in_unsafe_fn)` and native ownership/layout tests. Any further exception
must be isolated, justify necessity, state every invariant,
put a SAFETY comment on every unsafe block, and add focused tests. Profiling is required
before proposing an unsafe optimization. Third-party fuzz runtime native code is confined
to test tooling and is not an exception in a kernel crate.

CI enforces architecture, fmt, warnings-as-errors clippy, tests, license/source policy
with cargo-deny, and advisory audits of both workspace and fuzz lockfiles. CI configuration
is not evidence that hosted runs have already passed; see VALIDATION.md for actual results.
