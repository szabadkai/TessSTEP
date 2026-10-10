# Part 21 physical syntax

The implemented reader targets the clear-text exchange grammar shared with the 2016
edition, with the limitations recorded in CONFORMANCE.md. This is a syntax implementation,
not a full edition/conformance-class validator. The declared implementation level and
schema population are retained; their semantic consistency is not checked yet.

## Pipeline

`Lexer<R: BufRead>` produces owned tokens. `Parser<R>` produces `Event` values. Callers
can process and drop an event at a time. `tessstep_model::parse` drains every event,
rejects duplicate IDs and builds an immutable-inspection `Document`. Duplicate IDs are
found through the document's own entity index and external references, without a
second index of every declaration.

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

## Exporter slips

A parse is tolerant by default (`Conformance::Tolerant`): it accepts two exporter slips
that HOOPS Exchange also opens, counts each in `Document::deviations()` (`Deviations`,
with the count and first location of each kind) and reports each kind as one warning
of `Document::diagnostics()` at its first occurrence:

- **Literal backslashes.** A backslash in a string whose following characters complete
  no directive opening (`\\`, `\N\`, `\F\`, `\PA\` to `\PI\`, `\S\`, `\X\`, `\X2\`,
  `\X4\`) is a literal backslash (`TS1006` warning), as in the Windows paths
  `'C:\temp\part.stp'` and `'Alloys\Steel'`. The character that broke the opening is
  read normally, so an apostrophe right after the backslash still ends the string,
  and nothing after it is consumed as a directive operand. A complete opening with a
  malformed payload (`\X\GG`, lowercase or short hexadecimal groups, a missing `\X0\`,
  an undefined page character) is still rejected, as are backslashes outside strings.
- **Null header strings.** `$` stands for a string of FILE_DESCRIPTION or FILE_NAME,
  or for an element of their string lists (`TS1011` warning), as in an unset
  authorisation. The value stays `$`. A string list must still be a list, and
  FILE_SCHEMA, which names the schema, is never relaxed.

`Conformance::Strict` (`tessstep_model::parse_with`, `Parser::with_conformance`,
`stepdump --strict`) rejects both with the errors of Part 21 itself; a strict parse
accepts exactly the inputs that 1.0 accepted, and its deviations are always empty. For
inputs both modes accept, both produce the same document. No other recovery is
attempted: every other syntax error still ends the parse.

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
through Rust's standard allocation APIs in this implementation. Every budget failure is a
`LimitExceeded` diagnostic naming the budget and its configured value, for example
`record budget exceeded (limit 2000000)`. `stepdump` accepts each budget as
`--max-<name>=N` (`--max-input-bytes`, `--max-token-bytes`, `--max-string-bytes`,
`--max-entities`, `--max-nesting-depth`, `--max-aggregate-elements`,
`--max-total-values`, `--max-symbols`, `--max-records`, `--max-sections`); `--help`
prints the defaults. The largest corpus input that parses cleanly (30.7 MB, 413,209
entities) uses about a fifth of the default record and value budgets.

`ParseLimits::for_input(input_bytes)` sizes the budgets from the input length, as
`ImportOptions::for_document` sizes import budgets from the entity count: every
default, with `max_input_bytes` raised to the input length and `max_entities`,
`max_records`, `max_total_values` and `max_aggregate_elements` raised to the most of
each that many bytes of syntax can hold (`BYTES_PER_ENTITY` 7 for `#1=A();`,
`BYTES_PER_RECORD` 3, `BYTES_PER_VALUE` 2 for `$,`). No budget falls below its default,
and no input within its length can exceed those four; the token, string, nesting,
symbol and section budgets keep their defaults because they do not grow with the
model. The budgets stay explicit fields, and failures still name the budget and its
value. `stepdump` (for a file; standard input and `--default-budgets` keep the
defaults, and `--max-<name>=N` applies after sizing), the survey and assembly
examples, generated validators and therefore the corpus runner use it. The C ABI
offers it as `ts_parse_options_for_input` (ABI 1.1); `ts_parse_options_init` keeps
the defaults.

Sizing trades the fixed memory ceiling for one proportional to the input. The densest
corpus input holds a value per 5.45 bytes and the largest PreForm input (55.7 MB,
951,613 entities, 4,582,879 values, over the default value budget) one per 12. A value
takes 80 bytes (48 of them its source span), a record 88 and an entity 144 in the
public 1.x types, so a parsed exporter file takes about ten times its bytes (569 MB
peak for that file), and a hostile input made of `$,` up to forty times. List elements
collect on one shared stack and move into exactly sized vectors when the list closes,
which removed 2.53 million unused value slots (185 MB of peak RSS) from that file;
reference diagnostics collect only missing references
(`Document::missing_references`).

Both lexer and parser stop after their first error. The parser is a fused iterator.
Apart from the counted exporter slips above, tolerant recovery is deliberately not
provided; a fatal parse yields no complete Document. Earlier streaming events are
provisional until the iterator reaches EOF.

Reference used to resolve physical syntax details: [STEP Tools' published Part 21
edition 3 text](https://www.steptools.com/stds/step/IS_final_p21e3.html). No ISO prose,
EXPRESS schema definitions, or source examples were copied into this repository.
