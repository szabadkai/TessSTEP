# Part 21 physical syntax

The implemented reader targets the clear-text exchange grammar shared with the 2016
edition, with the limitations recorded in CONFORMANCE.md. This is a syntax implementation,
not a full edition/conformance-class validator. The declared implementation level and
schema population are retained; their semantic consistency is not checked yet.

## Pipeline

`Lexer<R: BufRead>` produces owned tokens. `Parser<R>` produces `Event` values. Callers
can process and drop an event at a time. `tessstep_model::parse` drains every event,
rejects duplicate IDs and builds an immutable-inspection `Document`.

Simple records, complex partial records, typed values with exactly one argument, empty
and nested aggregates, references, constants, enumerations, omitted/undefined parameters,
numbers, strings and binaries are represented independently of a schema. Complex component
names may not repeat. Inheritance completeness and schema-dependent ordering are deferred.
Mandatory header ordering and parameter shapes are checked. Unknown additional headers
are retained. DATA parameters remain generic for later semantic interpretation.

Anchors carry an item and ordered tags. Their item grammar is separate from entity
parameters. Resource literals cannot appear as ordinary entity parameters. REFERENCE
declarations retain their URI and typed occurrence ID; network access never happens during
parsing. Signature base64 is checked for framing and canonical padding and retained.
The consumer receives an explicit warning that it has not been cryptographically verified.

## Encodings and positions

Strings decode doubled apostrophes/backslashes, ISO-8859 pages A–I, X byte escapes,
X2 UCS scalar groups, X4 scalar groups, and valid direct UTF-8. X2 surrogate code points
are rejected; this implementation does not reinterpret UCS-2 groups as UTF-16 pairs.
Page selection resets per string. Undefined page entries, truncated directives and
invalid Unicode scalars fail. Unicode normalization is not performed.

Binary storage is MSB-first bytes plus an exact bit count. Physical leading pad bits
must be zero; internal unused trailing bits are zero. A one-bit value cannot be confused
with a one-byte value. Numeric integers use i64; instance numbers use nonzero u64;
reals use finite f64. Overflow and nonzero reals that underflow to zero fail explicitly.
Decimal rounding within representable f64 precision is expected; lexical decimal text
is not retained. No units are inferred.

Physical control octets below 0x20 and DEL are ignored, even inside tokens, while raw
byte offsets still advance. LF and CR/CRLF update physical lines. Explicit N/F print
directives are accepted at token boundaries and inside strings/binaries. Newlines are
not used to delimit records. Comments are scanned, not split or matched with regexes.
Positions use zero-based raw byte offsets and one-based lines and byte columns. Spans
are half-open. Spans are always retained in this version.

## Budgets

| ParseLimits field | Default |
|---|---:|
| max_input_bytes | 256 MiB |
| max_token_bytes | 1 MiB |
| max_string_bytes | 1 MiB decoded UTF-8 |
| max_entities | 1,000,000 |
| max_nesting_depth | 64 |
| max_aggregate_elements | 100,000 |
| max_total_values | 4,000,000 |
| max_symbols | 100,000 |
| max_records | 2,000,000 |
| max_sections | 1,024 |

Limits apply over the parse lifetime, even when streaming events are discarded. Typed
wrappers count toward depth. A hard maximum configuration depth of 128 prevents callers
from disabling recursive parser/drop stack protection. Header/complex/anchor/reference
records share a record budget. Tags and complex components have aggregate budgets.
Comments consume the input budget without token-sized allocation. Logical allocation
budgets do not constitute an exact process RSS cap; deployments handling hostile input
should choose budgets to fit their memory allowance. Allocation failure is not recoverable
through Rust's standard allocation APIs in this implementation.

Both lexer and parser stop after their first error. The parser is a fused iterator.
Tolerant recovery is deliberately not provided yet; a fatal parse yields no complete
Document. Earlier streaming events are provisional until the iterator reaches EOF.

Reference used to resolve physical syntax details: [STEP Tools' published Part 21
edition 3 text](https://www.steptools.com/stds/step/IS_final_p21e3.html). No ISO prose,
EXPRESS schema definitions, or source examples were copied into this repository.
