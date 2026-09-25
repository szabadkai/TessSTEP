# Schema bindings and reflection

Status: Milestone 3 implemented. Milestone 4 now provides
[structural physical-instance decoding](DECODING.md) using this metadata.
No AP schema or CAD interpretation is bundled.

## Generate and consume

```sh
cargo run -p expressc -- --rust corpus/express/valid/base.exp corpus/express/valid/imports.exp > bindings.rs
```

The CLI writes deterministic Rust to stdout and frontend unsupported diagnostics to
stderr. `--rust`, `--json` and `--ast` are mutually exclusive. `--strict --rust` rejects
opaque semantics and emits no Rust. Failed compilation or generation emits no partial
Rust. Exit codes remain 0 for success, 1 for compilation/generation/policy failure, and
2 for usage or I/O failures. A shell redirect can still truncate a destination before
the command runs; generate to a temporary file before replacing an existing binding.

Generated code needs Rust 2024 (minimum 1.85) and a direct `tessstep-schema` dependency
matching the generator version. It can be a crate root, module file or `include!` body:

```rust,ignore
mod bindings { include!("bindings.rs"); }
use bindings::{schema_base, SCHEMA_SET};
let schema = SCHEMA_SET.schema("base").unwrap();
let part_id = schema.lookup("part").unwrap();
let description = SCHEMA_SET.declaration(part_id).unwrap();
let part = schema_base::Entity_PART {
    attr_name: "example".into(),
    attr_samples: vec![Some(schema_base::Type_MEASURE(1.0)), None, None],
    attr_tint: None,
};
```

The library API is `tessstep_codegen::generate(&compilation, Limits::default())`.
Supply an unmodified successful `tessstep_express::compile` result; the generator
consumes its resolved IR and linked AST. There is no filesystem access, implicit schema
search, network access, compiler invocation or expression evaluation in the library.
The umbrella crate reexports `codegen` and `schema` modules.

## Rust binding contract

Each schema becomes `schema_<lowercase EXPRESS name>`. Entities are `Entity_<UPPERCASE
NAME>`, defined types are nominal `Type_<UPPERCASE NAME>` tuple structs, and explicit
fields are `attr_<lowercase name>`. Prefixes and preserved underscores avoid keyword and
case-conversion collisions. Imported entities/types are reexports with their local
EXPRESS alias; their Rust type and declaration identity remain those of the defining
schema. Constants and algorithm declarations have metadata only.

| EXPRESS domain | Rust representation |
|---|---|
| BOOLEAN | `bool` |
| LOGICAL | `Logical::{False, True, Unknown}` |
| INTEGER | `i64` |
| REAL / NUMBER | `f64` |
| STRING | `String` |
| BINARY | `Vec<bool>` in logical bit order |
| entity domain | `EntityRef<Entity_NAME>` |
| defined type | nominal `Type_NAME` wrapper |
| ARRAY / BAG / LIST / SET | `Vec<Element>` |
| optional attribute / ARRAY element | `Option<T>` |
| ENUMERATION | generated enum with `Member_<NAME>` variants |
| SELECT | generated enum with `Alternative_<declaration ID>(T)` variants |

Anonymous enum and SELECT definitions use `Domain<N>` helper types. Explicit attribute
domains use `Attribute<declaration ID>_<local attribute index>` aliases. Helpers are
public so consumers can construct every value; their names, declaration IDs and SELECT
variant suffixes depend on ordered input. They are not stable across schema edits,
source reordering or generator releases. Generated type wrappers expose their value
through `.0`; all records/enums implement `Clone`, `Debug` and `PartialEq`.

Entity records flatten explicit inherited fields in depth-first parent declaration
order, followed by local fields; diamonds visit each ancestor once. This is an owned
record layout, **not** a claim about Part 21 complex-entity parameter order. Inherited
anonymous domains retain their original Rust type. DERIVE and INVERSE fields remain
metadata, with no computed accessor. No implicit field override or redeclaration is
supported by the frontend.

`EntityBinding::DECLARATION` connects each entity record to its metadata. `EntityRef<T>`
contains only a nonzero `u64` occurrence number and a type marker. It is `Copy` without
requiring `T: Copy`, and direct-supertype `upcast()` operations can be chained. References
carry no document identity or lifetime, perform no lookup, and prove neither existence
nor schema compatibility of any physical record. They cannot be implicitly downcast.

All records are **unchecked owned data**, including abstract entity records. Aggregate
cardinality/uniqueness, numeric ranges, string widths, optionality, WHERE/UNIQUE rules,
SELECT validity of physical values and EXPRESS expression semantics are not validated.
`i64`/`f64` are implementation representation limits, not arbitrary EXPRESS precision.
Generic Part 21 instances remain independent; no decoder, borrowed instance view,
serialization, public C ABI or C++ wrapper is introduced by this milestone.

## Reflection and limits

`SCHEMA_SET` is immutable static metadata requiring no initialization or allocation.
It contains supplied source names, schema identities/spans, dependencies, visible symbols
and exports, source-order declarations, entity inheritance and abstract/supertype flags,
local explicit/derived/inverse attributes, optionality, resolved domains, aggregate kinds
and bounds, enum members, SELECT alternatives, constants, WHERE/UNIQUE rules and opaque
algorithm bodies. Source expressions/spans and unsupported diagnostics are retained;
reflection never marks opaque constraints as evaluated. Inherited metadata is reached
through direct supertype IDs. IDs are local indices into this generated schema set.
Lookup is ASCII case-insensitive and linear in the corresponding table; out-of-range
IDs return `None`.

Default generation budgets are 64 MiB of Rust output and 5,000,000 logical work units.
`--max-output-bytes` controls output bytes; `--max-work` applies separately to frontend
compilation and generation. Domain recursion is hard-capped at 128. Inheritance traversal
is iterative, with visited ancestors and cycle detection; it charges work as it expands
fields. The bounded writer refuses output before appending beyond its byte budget.
These are logical limits, not exact RSS or compiler resource limits. Large generated
schemas may require additional rustc resources or a consumer recursion-limit attribute.
No time stamps, absolute environment paths or randomized ordering are added; output is
byte-identical for the same compilation, including source names, ordered files and spans.

See [TESTING.md](TESTING.md), [VALIDATION.md](VALIDATION.md) and
[ARCHITECTURE.md](ARCHITECTURE.md) for tests, observed checks and dependency boundaries.

`expressc --validator` emits the same metadata plus a standalone validator application;
its runtime also depends on `tessstep-model`. See [DECODING.md](DECODING.md) for its JSON
protocol, bounds, unsupported semantics and the optional corpus schema stage. Frontend
opaque-expression diagnostics are preserved unchanged; decoding evaluates only its named
bound/width subset and rejects all other unsupported diagnostics.
