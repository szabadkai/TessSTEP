# EXPRESS frontend

Status: Milestone 2 implemented as a declaration frontend with basic structural semantic
checks. This is not full ISO 10303-11 conformance, expression validation, AP support,
generated bindings, or schema-aware validation of physical STEP instances.

## API and phases

`tessstep-express` has no third-party or other TessSTEP dependencies. `lex(source_id,
text, limits)` returns tokens; `parse(source_id, text, limits)` returns schemas with a
source-located AST. `compile(&[Source { name, text }], limits)` parses all supplied files
and returns `Compilation`: source names, AST schemas, optional `SchemaIr`, and diagnostics.
All files must be supplied explicitly; the frontend never reads paths or fetches imports.
Schemas can appear in any file/order, including multiple schemas per file.

Spans hold a source index, half-open byte offsets, and one-based line/byte-column values.
Identifiers are ASCII and case-insensitive, normalized to uppercase; original spelling
can be recovered from source spans. Strings/comments can retain UTF-8. The AST owns its
text, including interior expression whitespace/comments, and can outlive input buffers.
Only the complete successful parse of each file is retained; there is no syntax recovery.

The IR has source-order declaration IDs, resolved type/attribute domains, schema symbols,
exports and dependency indices. Each IR declaration links to the complete AST through
`schema` and `ast_declaration` indices, preserving all constraints/expressions. IDs are
stable for identical ordered source arrays, not across reordered inputs or compiler
versions. Ordered maps and sorted diagnostics make repeated compilation deterministic.

An error means `ir` is `None`. `Severity::Unsupported` means source was retained but its
semantics were not checked; it does not suppress IR. Clients must inspect diagnostics.
Even warning-free IR proves only the structural checks below, not full schema conformance.

## Supported syntax

- SCHEMA, ENTITY and TYPE declarations; CONSTANT sections.
- ABSTRACT entities, SUPERTYPE constraints retained as opaque expressions, SUBTYPE OF
  with multiple named parents.
- Explicit attributes, grouped attribute names and OPTIONAL; DERIVE expressions;
  INVERSE with FOR attribute or entity.attribute; UNIQUE and WHERE rules/labels.
- Named domains, BOOLEAN/LOGICAL/INTEGER/NUMBER/REAL/STRING/BINARY; width/precision
  expressions and FIXED on sized strings/binary.
- ARRAY/BAG/LIST/SET, optional bounds (required for ARRAY), ARRAY element OPTIONAL,
  LIST/ARRAY element UNIQUE, nested aggregates, SELECT and ENUMERATION OF.
- USE FROM and REFERENCE FROM, named lists, AS aliases and wildcard imports.
- FUNCTION, PROCEDURE, RULE and SUBTYPE_CONSTRAINT bodies are retained verbatim,
  including their terminators, with unsupported diagnostics. Their internal grammar
  is not validated, and their declarations cannot serve as entity/type domains.

The lexer recognizes nested `(* ... *)` comments, `--` line comments, identifiers,
integer/real spellings (without numeric conversion), apostrophe strings with doubled
apostrophes, eight-hex-digit-group encoded strings, percent-prefixed binary literals,
and EXPRESS punctuation/operators. Encoded strings are not decoded or scalar-validated.
Tokens preserve literal spelling. Resource limits apply before token allocation.

## Basic semantic checks

- Duplicate schemas/declarations, attributes, supertype entries, rule labels,
  enumeration members and SELECT targets (including aliases).
- Missing dependency schemas/imported declarations; local/import collisions and ambiguous
  visibility; unresolved domains or use of a non-type/non-entity declaration as a domain;
  non-entity supertypes.
- USE imports eligible entities/types and reexports them. REFERENCE imports exported
  declarations into local visibility without reexporting them. Wildcard USE filters out
  constants/algorithms; an explicit USE of one is an error. Imported declarations keep
  their original defining-schema identity and resolve their own domains there.
- Cyclic schema import graphs resolve by a bounded fixed point. Inheritance and defined
  type cycles (including aggregate/SELECT paths) are errors. Recursive entity attributes
  are allowed. Dependencies on a cycle also receive a diagnostic.
- Inherited attribute conflicts; diamond inheritance counts an ancestor once. Attribute
  redeclaration/renaming syntax is not implemented, so no implicit overrides are assumed.
- Direct signed integer-literal bound ordering and negative non-ARRAY lower bounds when
  both bounds parse as integers. General bound expressions are not evaluated.
- INVERSE domain resolves to an entity or one SET/BAG of entities, including through
  aliases. FOR resolves to one explicit attribute in that entity's hierarchy, and any
  qualifier must belong to that hierarchy. Forward type compatibility/cardinality is
  explicitly diagnosed as unsupported.

Expressions in DERIVE, WHERE, UNIQUE, bounds, widths, constants and SUPERTYPE constraints
are opaque, nonempty, delimiter-balanced source fragments. Every such fragment generates
an unsupported diagnostic: expression grammar, names, operators, types and evaluation are
not validated. An invalid expression can therefore survive structural compilation; this
is intentional preservation, not expression acceptance. No constraints are executed.

Unrecognized declaration syntax, EXTENSIBLE/BASED_ON types, generic parameter domains,
redeclared SELF-qualified attributes, schema version identifiers and other unimplemented
constructs fail with a structured syntax diagnostic. Unknown declaration bodies are not
silently discarded. No legally supplied ISO/AP schemas are bundled or hand transcribed.

## Limits and diagnostics

Defaults apply to the full compilation source set:

| Budget | Default |
|---|---:|
| Input UTF-8 bytes | 16 MiB |
| Tokens | 1,000,000 |
| Bytes per token | 1 MiB |
| Nested comments/types/expression delimiters | 64 (hard ceiling 128) |
| Sources and schemas | 256 each |
| Declarations | 100,000 |
| AST expansion / semantic work | 5,000,000 |

The work budget charges copied grouped-attribute source sizes, imported-name copies,
resolution operations and graph traversal. Parsing alone applies the syntax budgets;
compilation shares remaining budgets across files and semantic validation. These are
logical limits, not an RSS or time guarantee. Import closure and ancestor checking can
be superlinear but cannot traverse indefinitely. All graph traversal is iterative;
bounded nested type structures are the only recursive compiler traversal.

| Code | Meaning |
|---|---|
| EX1001 | Resource budget exceeded |
| EX1002 | Lexical failure |
| EX1003 | Malformed or unsupported declaration syntax |
| EX2001 | Retained source with unsupported semantics |
| EX2002 | Duplicate declaration/member/label |
| EX2003 | Missing/invalid schema import or export |
| EX2004 | Ambiguous visible declaration |
| EX2005 | Unresolved name or wrong declaration kind |
| EX2006 | Invalid checked literal bounds |
| EX2007 | Inheritance/defined-type cycle or dependent declaration |
| EX2008 | Inherited attribute conflict |
| EX2009 | Invalid inverse target/domain |

## CLI

```sh
cargo run -p expressc -- corpus/express/valid/base.exp corpus/express/valid/imports.exp
cargo run -p expressc -- --json corpus/express/valid/base.exp
cargo run -p expressc -- --ast corpus/express/valid/opaque.exp
cargo run -p expressc -- --strict supplied.exp dependencies.exp
```

Default output summarizes structural compilation and writes diagnostics with filenames
to stderr. `--json` writes a deterministic versioned inspection summary with source names,
resolved schema symbols/exports/dependencies, declaration identities, diagnostics/spans,
`structural_valid`, and `expression_semantics: "not_implemented"`. It is not a serialized
full IR; use the Rust API for typed domains. `--ast` prints the complete debug AST.
`--strict` fails on unsupported diagnostics as well as errors. `--max-bytes`,
`--max-tokens`, `--max-work` and `--max-nesting` override those budgets. `--` ends options.

Exit 0: structural success under the selected policy; exit 1: parse/semantic/resource
failure or strict-policy rejection; exit 2: usage/I/O/UTF-8/file-read budget failure.
The CLI bounds file reads cumulatively before compiling. It never writes generated code.

See TESTING.md, CONFORMANCE.md and VALIDATION.md for test evidence and observed results.
