# Corpus verification

For parser, model, geometry or tessellation implementation changes, run the
relevant Rust tests and `python3 scripts/corpus.py --check` when `~/step-corpus`
(or `TESSSTEP_CORPUS`) is available. Inspect the generated per-file progress
report and include outcome changes and remaining unsupported stages in the
completion summary. If the external corpus is unavailable, report that fact.

Do not refresh `corpus/baseline.json` merely to silence regressions. Review the
changed inputs first. Physical parsing success is not schema or tessellation
success; add stage checks as implementation becomes available. See
`docs/corpus-testing.md` for the runner and baseline workflow.

# Public C and C++ interfaces

The C ABI and C++ wrapper are supported public interfaces, with compatibility,
documentation and release testing obligations. No Rust type, layout, allocator,
panic, or ownership semantics may cross the ABI boundary.

The C++ API must provide RAII ownership, zero-copy read-only mesh views where
possible, typed errors, and a normal CMake package target `TessSTEP::TessSTEP`.
Express ownership and view lifetimes through explicit C contracts and C++ RAII;
keep Rust implementation details private. See `docs/C_API.md` for the contract.
Track actual implementation status separately from this requirement; do not
claim an interface is available before its implementation and consumer tests.
