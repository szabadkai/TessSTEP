# Schema-aware instance decoding

Status: Milestone 4 structural decoder implemented with the limits below. No complete
EXPRESS language, application protocol or CAD interpretation is claimed.

`tessstep::model::decode::decode` (or `tessstep_model::decode::decode`) accepts an
immutable physical `Document`, supplied `tessstep_schema::SchemaSet`, an explicit schema
name and decoding `Limits`. Generated `SCHEMA_SET` metadata is consumed directly:

```rust,ignore
let decoded = tessstep::model::decode::decode(
    &document, &bindings::SCHEMA_SET, "example", Default::default(),
)?;
for entity in decoded.entities() {
    for attribute in &entity.attributes {
        println!("{} {} {:?}", entity.id, attribute.declaration.name, attribute.value.kind);
    }
}
```

The caller selects the schema. No filesystem search, fetching, FILE_SCHEMA inference,
DATA population selection or mutation of the physical document occurs. Parameterless
DATA sections all use the selected schema; parameterized DATA remains `Unsupported`.

## Structural validation

Entity identity follows schema visibility, including imported aliases. Internal mappings
flatten explicit inherited attributes in parent declaration order, deduplicating diamonds.
External complex mappings check canonical component ordering, uniqueness, ancestor
completeness, connected inheritance and multiple concrete leaves. Each component supplies
only its local explicit attributes, including empty parameter lists where required.
Abstract ancestors are permitted; abstract leaves and wrong parameter counts fail.

Entity references require local existence and compatible membership, including every
branch of multiple inheritance and complex instances. Forward, cyclic and self references
are resolved without expanding instance graphs. External references remain unsupported.
Sparse IDs never determine allocation size.

Domains support primitives, named aliases, enumerations, nested aggregates and SELECTs.
SELECT entity alternatives use references; defined scalar/aggregate and enumeration
alternatives require the correct tag. Nested SELECTs are traversed, while a defined alias
to a SELECT keeps its own tag and nested encoding. Incorrect tags and values fail.
The physical mapping follows [Part 21 clauses 12.1.8 and 12.2.5](https://www.steptools.com/stds/step/IS_final_p21e3.html).

`$` is accepted only for optional attributes/elements. `*` is rejected in explicit slots.
REAL/NUMBER domains accept integer physical values without losing their original value.
STRING widths count Unicode scalars; BINARY widths count logical bits; FIXED is enforced.
ARRAY lengths use inclusive indices. LIST/BAG/SET bounds are checked, with `?` supported
as an unbounded upper aggregate limit.

SET and aggregate UNIQUE constraints use value comparisons that ignore source positions.
References compare occurrence identity. Numeric comparisons avoid rounding distinct large
integers into equal floating-point values. LIST/ARRAY equality is ordered; BAG/SET equality
is unordered and accounts for multiplicity. Unset optional ARRAY positions do not count
as known duplicate values. Cross-defined-type SELECT aggregate equality is conservatively
unsupported; full EXPRESS value comparison is not claimed.

Width and bound expressions support integer literals, parentheses, unary signs, addition,
subtraction, multiplication, comments, and DIV/MOD with nonnegative operands. Arithmetic
uses checked i128 intermediates and requires an i64 result. Division by zero and overflow
fail explicitly. Variables, functions, real arithmetic and negative DIV/MOD operands are
unsupported. The frontend still preserves expressions as opaque; the decoder recognizes
only diagnostics attached to supported bound/width expression spans. All other opaque
schema diagnostics remain failures, even for unused declarations.

## Views and failure contracts

`DecodedDocument` borrows both inputs and publishes source-order immutable views only
after every instance passes. `get` uses a sparse ordered ID index. `EntityView::types`
contains complete entity membership; `declaration` is `Some(leaf)` for internal mapping
and `None` for external complex mapping. `AttributeView::owner` identifies the declaring
entity. Complex attributes follow component order. Values remain borrowed physical values.
The initial single-entity `declaration` field is now optional to represent complexes
without inventing a single type. This Rust view change does not affect the public C ABI.

Errors distinguish schema/metadata/name issues, abstract leaves, complex mapping, attribute
count, required value, type, cardinality, width, duplicate value, missing/incompatible
reference, unsupported behavior and resource exhaustion. Owner, attribute and exact value
span accompany failures when available. Identity/hierarchy checks precede values, so the
first deterministic error follows phase order, not global byte order. No partial view is
returned and no generated owned record or document-bound `EntityRef<T>` is constructed.

Default limits are 5,000,000 logical work units and depth 128, with a hard depth ceiling
of 128. Cyclic metadata fails explicitly or exhausts a budget. Hierarchy and SELECT walks
are iterative; nested values, expressions and comparisons are depth-bounded. Repeated
hierarchy expansion and uniqueness checks can be superlinear/quadratic; scans, comparisons
and allocations consume logical work. Budgets are not exact time/RSS caps. Metadata is
expected to be unmodified generator output, not an independently validated serialization.

## Generated validator and corpus stage

`expressc --validator supplied.exp ...` emits deterministic standalone Rust source using
the same bindings/reflection plus a small CLI. Its Cargo application needs direct local
(or matching-version) dependencies on `tessstep-model` and `tessstep-schema`. Compile it as
a Rust 2024 binary, then invoke `validator SCHEMA FILE.step`. No runtime source compilation
or Rust toolchain is needed by the resulting executable. Generation honors the same
output/work limits, strict policy and mutually exclusive output modes as `--rust`.

The executable emits bounded JSON (`format_version: 1`, `scope: schema-structure`).
`status` is accepted, rejected, unsupported, not_configured, resource_limit or not_run
(the physical parse failed). Exit codes are 0 for accepted, 1 for a checked failure,
and 2 for usage/I/O failure. It uses default finite parser/decoder limits.

`python3 scripts/check_schema.py` builds an authored validator and verifies reviewed
per-fixture outcomes through the corpus runner; results are in `reports/schema/`.
To apply your compiled validator to a separately selected corpus:

```sh
python3 scripts/corpus.py --corpus /path/to/inputs --output reports/schema-custom \
  --baseline /path/to/separate-baseline.json \
  --schema-validator /path/to/validator --schema-name example
```

The runner snapshots both executables, records the validator hash/schema, enforces its
JSON/exit protocol and shows schema results separately. Physical outcomes keep their
meaning. Previously accepted schema inputs becoming non-accepted are regressions.
The default external corpus still lacks AP metadata and keeps its schema stage marked
`not_implemented`; physical success is never promoted to schema acceptance.

## Remaining conformance limits

DERIVE/INVERSE evaluation, attribute redeclaration, WHERE/entity UNIQUE rules, supertype
expressions, algorithms, constants, value references, precision semantics and general
EXPRESS evaluation remain unsupported. This milestone does not bundle AP schemas, fetch
external references, select populations automatically, construct owned generated records,
or expose schema decoding through C/C++. STEP geometry adaptation remains a future stage; independent constructed geometry
and tessellation do not imply schema-to-mesh support.
See [VALIDATION.md](VALIDATION.md) for actual checks and corpus observations.


`decode_reachable(document, schemas, schema_name, roots, limits)` explicitly limits
structural validation to the roots' local entity-reference closure. It shares a work
budget between iterative cycle-safe traversal and decoding. Unrelated entities remain
unchecked; whole-document `decode` retains its existing contract. Missing closure
references retain the referencing owner and value source span. This mode supports
bounded import profiles and must not be presented as full-document validation.


`decode_reachable_profile` additionally takes explicit `OmittedSlot` declarations
for physical profile mappings. Each names a resolved entity declaration and an
unambiguous inherited/local attribute. Only that entity/subtype's named slot accepts
`*`; all other values in that slot are rejected. Duplicate or absent slot definitions
are invalid metadata. Work is charged for policy checks and inherited memberships.
This opt-in mapping is used for planar `ORIENTED_EDGE` endpoint slots, whose semantics
are implemented by the geometry adapter. It does not weaken either ordinary decoder,
evaluate general DERIVE expressions, or bypass unsupported schema diagnostics.

`decode_reachable_profile_with_links` additionally takes explicit `LinkSlot`
declarations. Each resolves to exactly one declaring owner of the named attribute;
duplicates, absent attributes and overlap with an omitted slot are invalid metadata.
Traversal does not follow a link slot, so its target and the target's closure are
neither decoded nor type-checked and are absent from the result unless another
decoded reference reaches them. The slot itself accepts only `$` (when optional) or a
reference to a local entity; anything else is a type mismatch, and a missing target
is a missing reference. When link slots are declared, traversal resolves each
record's type identities first, so parameter-count and name errors can be reported
before closure expansion. Existing-tessellation import uses this for B-rep
provenance links; see [EXISTING_TESSELLATIONS.md](EXISTING_TESSELLATIONS.md).
