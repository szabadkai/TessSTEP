# Schema-aware instance decoding

Status: first Milestone 4 slice implemented; milestone remains in progress.
No application protocol, CAD interpretation or full EXPRESS conformance is claimed.

`tessstep::model::decode::decode` (or `tessstep_model::decode::decode`) accepts an
immutable physical `Document`, supplied `tessstep_schema::SchemaSet`, a schema name,
and decoding `Limits`. Generated `SCHEMA_SET` metadata can be used directly:

```rust,ignore
let decoded = tessstep::model::decode::decode(
    &document,
    &bindings::SCHEMA_SET,
    "example",
    Default::default(),
)?;
for entity in decoded.entities() {
    for attribute in &entity.attributes {
        println!("{} {} {:?}", entity.id, attribute.declaration.name, attribute.value.kind);
    }
}
```

The caller explicitly selects the schema. The decoder does not search the filesystem,
fetch schemas, interpret FILE_SCHEMA identifiers, or infer DATA population mappings.
Each parameterless DATA section uses the selected schema. A parameterized DATA section
returns `Unsupported`. The library leaves the physical document unchanged.

## Supported structural checks

Simple entity names resolve through schema visibility symbols, including imported
aliases. Single inheritance contributes ancestor attributes first, followed by local
explicit attributes. Abstract entity instantiation and incorrect attribute counts fail.
Required `$` values and `*` in explicit slots fail; optional attributes may contain `$`.

Primitive checks cover INTEGER, REAL/NUMBER (including integer values), STRING, BINARY,
BOOLEAN and LOGICAL, plus enumeration membership and named type aliases. Literal
STRING/BINARY widths and FIXED lengths are checked in Unicode scalars/logical bits.
ARRAY length follows inclusive lower/upper indices; optional ARRAY elements are allowed.
LIST/BAG cardinality uses literal bounds, including `?` as an unbounded upper limit.
Nested aggregates consume both depth and work budgets. Values remain lossless physical
values; the decoder does not coerce integers into floating point.

Entity references must exist locally and target the required entity or a subtype.
Forward, backward, self and cyclic instance references work without traversing instance
graphs. An externally declared target returns `Unsupported`; a missing target returns
`MissingReference`. Sparse physical IDs never determine allocation size.

## Views, errors and limits

Success publishes a `DecodedDocument` with source-order entities, declaration IDs,
attribute metadata and borrowed physical values. `get` uses an ordered ID index. It
borrows both inputs and cannot outlive either; fields in stored views cannot be mutated
through the document API. It is structural evidence for this subset, not a general
validated EXPRESS model. It does not construct generated owned Rust records or resolve
an `EntityRef<T>` into a document-bound typed handle.

Failure returns the first deterministic typed `Error`, with the owning entity,
attribute name and exact physical value span when available. No partial view is returned.
Errors distinguish unknown schema/entity, invalid metadata IDs, abstract instantiation,
attribute count, required value, type, width, cardinality, missing/incompatible reference,
unsupported behavior and resource limits. Schema-wide errors have no physical owner.
Identity resolution precedes attribute checks, so ordering is phase order, not globally
sorted physical byte order.

Defaults: 5,000,000 logical work units and depth 128, with a hard depth ceiling of 128.
Lookups, hierarchy expansion, value visits and width character counts consume work;
allocations scale with retained entities and attributes. These are not exact time/RSS
limits. Schema symbol lookup is linear; ID indexing is O(log n). Repeated inherited
attribute expansion can be superlinear. Malformed cyclic metadata terminates at a budget.
Metadata is expected to be unmodified generator output, not an independently validated
schema-set serialization format.

## Explicit remaining work

Complex entity mapping, multiple inheritance, SELECT/typed-parameter decoding,
SET/UNIQUE equality, expression bounds, numeric precision, DERIVE/INVERSE, WHERE/UNIQUE
rules, supertype constraints, constants and value references remain unsupported.
A schema set with any preserved unsupported diagnostics is conservatively rejected,
even if a particular instance does not use the declaration. For an unset optional
attribute, its domain is not traversed; no claim is made about unevaluated constraints.
External reference fetching, AP schemas, automatic population selection, generated
record conversion, public C/C++ decoding operations and industrial hardening remain open.

Tests include an EXPRESS → generated metadata → physical document consumer, adversarial
metadata, error spans, deterministic mutation smoke and structural checks. The external
STEP corpus still has no supplied AP schema metadata or schema-stage CLI integration;
its `schema` stage remains `not_implemented` in that runner. Physical acceptance there
is not schema success. See [VALIDATION.md](VALIDATION.md) for observed results.
