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
