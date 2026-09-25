# Error model

The Part 21 `Diagnostic` carries severity, a stable code, an original message, an optional entity
ID, and a source span. `DiagnosticCode::as_str` provides machine-facing TS codes.
TS10xx covers syntax/I/O, TS11xx covers identity/reference/trust observations, TS1201
identifies an exceeded budget, and TS1301 identifies recognized unsupported syntax.
Malformed entity syntax includes its owning ID when known. Duplicate declarations
report the location of the earlier definition. Reference errors identify the exact use.

Lexing/parsing fail at the first error. `model::parse` returns `Result<Document,
Diagnostic>` and never returns a silently incomplete database. `Document::diagnostics`
performs a separate full reference-existence check. Forward references, cycles and
self references are not inherently errors at this layer. External declarations are
warnings requiring later resolution, not local missing-ID failures. Constants remain
symbolic until schema processing. Signatures always receive an unverified warning.

`stepdump` uses exit code 0 for a complete parse without error diagnostics (warnings
are possible), 1 for parse or missing-reference errors, and 2 for usage/open/output I/O
errors. `--json` returns versioned structured parse/reference diagnostics. A fatal
parse has `document: null`; open/output errors go to stderr. Human text follows source
order. JSON headers are an ordered array, preserving repeated extension headers.

No parsing error causes geometry healing, schema guessing, unit selection, or entity
substitution. Future semantic layers should introduce typed domain errors and retain
source links. Any future tolerant API must expose completeness and every skipped record.

The independent EXPRESS frontend uses source-indexed spans and EX codes (see
EXPRESS.md). Declaration syntax fails at the first error per file; compilation gathers
basic semantic errors across parsed schemas and sorts diagnostics by source/offset.
It exposes no IR when errors exist. Retained opaque expressions/algorithm bodies emit
`Severity::Unsupported` (EX2001); successful structural compilation does not mean those
semantics were checked. `expressc --strict` rejects this diagnostic category as well as
errors; ordinary mode returns structural IR with the diagnostics visible.

Tessellation returns `TessellationError` with an optional model-local face and a typed
kind, retaining nested sampling, trim, polygon or mesh errors. Invalid handles/options,
resource exhaustion, singular surfaces and unresolved sampled tolerances are distinct
failures. Mesh validation separately reports invalid attributes/indices, degeneracy,
orientation/manifold defects, open/disconnected components and nonpositive solid volume.
No partial mesh or automatic repair accompanies failure. C mesh import maps invalid
geometry to `TS_INVALID_MESH` and budgets to `TS_RESOURCE_LIMIT`; detailed Rust enums
remain private to Rust callers. See [TESSELLATION.md](TESSELLATION.md) and [C_API.md](C_API.md).

Assembly scenes distinguish missing/duplicate typed asset and occurrence identities,
cycles, empty group baking, resource exhaustion, located numerical transform failures
and mesh validation errors. The C boundary maps malformed scene creation to
`TS_INVALID_SCENE` (8), missing query/bake targets to `TS_NOT_FOUND`, and limits to
`TS_RESOURCE_LIMIT`; C++ exposes the corresponding typed error codes. No partial scene
or baked mesh is published. Existing ABI status meanings are unchanged.

Appearance distinguishes invalid color components, zero/duplicate/missing material IDs,
duplicate bindings, missing asset/instance/face targets, group-face assignments, invalid
triangle queries and resource limits. Unstyled is successful absence, distinct from
an explicit alpha-zero material. The C bridge maps invalid appearance semantics to
`TS_INVALID_APPEARANCE` (9), malformed C records to `TS_INVALID_ARGUMENT`, missing query
targets to `TS_NOT_FOUND` and budgets to `TS_RESOURCE_LIMIT`. Creation never publishes
partially validated assignments; all writable failure outputs are cleared.
