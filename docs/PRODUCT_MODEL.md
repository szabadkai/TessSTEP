# Products and representations

Milestone 5 now has an initial structural product graph and explicit unit scales.
Placement descriptions are preserved, not evaluated. This is a Rust extraction API,
not AP242 conformance, a geometry loader or an assembly renderer.

## Ownership and entry points

`tessstep-product` has no dependencies. Callers construct `Parts` and validate them
with `Model::new(parts, max_work)`. Successful models expose immutable parts and roots.
Typed product, formation, definition, representation, shape, occurrence, map and item
identities are local to a model; they are not vector indexes or geometric validity.
Products, versions/formations, design definitions and occurrences remain distinct.
Repeated occurrences reuse the same definition and representation records.

`tessstep-ap242::adapt(decoded, schema_name, limits)` adapts a structurally decoded
document. The umbrella exports these crates as `tessstep::product` and `tessstep::ap242`.
The adapter crate name reserves the architecture boundary, not a claim that a complete
AP242 schema is bundled or supported. Callers supply metadata and pass the decoder;
no unsupported EXPRESS rule or diagnostic is bypassed here.

```rust,ignore
let decoded = tessstep::model::decode::decode(
    &document, &bindings::SCHEMA_SET, "MY_SCHEMA", Default::default(),
)?;
let products = tessstep::ap242::adapt(&decoded, "MY_SCHEMA", Default::default())?;
for root in products.roots() {
    println!("assembly root: {root:?}");
}
```

Output owns its strings and graph records. Adapter identity numbers preserve physical
entity IDs. Opaque item IDs are provenance for a future geometry adapter; retain the
original document if those items will need interpretation. No geometry is discovered
by scanning ADVANCED_FACE records. No evaluated placement or world transform is
manufactured from product graph success.

## Supported relationships

Roles resolve through the explicitly selected schema's symbol table. The adapter checks
decoded declaration memberships and named attributes, without a hand-encoded AP
inheritance tree or physical parameter offsets. Both internal and external complex
mappings work within the decoder's subset.

| Input role | Retained meaning |
| --- | --- |
| product → product_definition_formation → product_definition | Product identity, version and design definition |
| product_definition_shape → shape_definition_representation | Shape identity and representation binding |
| representation → context_of_items/items | Reusable representation, units and opaque item identities |
| next_assembly_usage_occurrence | Parent/child definition and distinct occurrence identity |
| representation_relationship | Ordered representation endpoints; unspecified transformation stays explicit |
| representation_relationship_with_transformation → item_defined_transformation | Ordered item pair, never an assumed identity matrix |
| context_dependent_shape_representation | Occurrence-to-representation-relationship placement link |
| representation_map / mapped_item | Reusable source representation, mapping origin and target item |

Validation rejects duplicate identities, missing links, assembly cycles and mapped-use
cycles, including disconnected cycles. Repeated children and diamonds are legal; no
instance tree is expanded. Roots follow input order.

This slice requires transformation item 1 directly in representation 1 and item 2 in
representation 2. Map origins must be direct source representation members; targets
must be direct members of each using representation. Occurrence placements must connect
representations directly bound to the parent/child definitions, in either endpoint order.
Indirect item-in-context and transitive shape association paths fail graph validation.
Multiple representation alternatives and placements remain explicit; none is selected
as an active alternative.

## Explicit units

Only 3D geometric contexts with global unit assignments are supported. Each must supply
exactly one length, plane-angle and solid-angle unit; no default units are invented.
SI metre/radian/steradian units with STEP prefixes from atto through exa and positive
conversion-based chains are interpreted. Conversion must preserve its unit role and
seven dimensional exponents. Iterative chains have cycle and depth checks. Offset units,
other dimensions and uninterpretable unit definitions are explicit unsupported errors.
Names such as “inch” and “degree” are labels; the numeric chain determines the scale.

`Unit::to_si(value)` converts explicitly requested quantities to **metres, radians or
steradians**. It rejects non-finite values and nonzero underflow. Scales are finite and
positive; f64 arithmetic does not promise exact decimal conversion. Raw geometry
coordinates remain uninterpreted: storing a scale does not convert the physical file.

## Limits and remaining work

Errors carry typed kinds and physical owner/attribute/source when available. Global
graph failures have a graph error without an invented owner. No partial model is
published. Default budgets are 5,000,000 logical work steps, 16,000,000 copied text
bytes and 128 unit-chain nodes. Graph validation consumes the remaining work budget.
Ordered maps and input-order vectors preserve determinism. Membership scans can be
quadratic; logical budgets are not exact RSS or wall-time limits.

Administrative metadata, uncertainty/tolerance measures, geometric placement values,
axis defaults, matrix composition/inversion, Cartesian/nonuniform transformation
operators, 2D contexts, external assemblies, other assembly relationship kinds,
AP242 occurrence variants, configuration selection and general EXPRESS rules remain
future work. Opaque representation items are not certified by product stage success.
The C ABI and C++ wrapper remain at physical documents; no product/schema operations,
mesh views or Rust layouts are exposed through ABI 1.

## Evidence and references

`corpus/product/sample.exp` is an original reduced test schema, not an ISO extract.
Tests cover independent graphs, repeated parts, millimetre/inch and degree conversion,
complex mappings, mapped reuse, ordering, budgets, invalid units/links, cycles and
deterministic mutations. `python3 scripts/check_product.py` verifies generated metadata
and separate physical/schema/product stages against reviewed fixture expectations.
See [VALIDATION.md](VALIDATION.md) for observed evidence and remaining limits.

Relationship fields were checked against STEP Tools' published reference pages for
[mapped_item](https://www.steptools.com/docs/stp_aim/html/t_mapped_item.html),
[representation_map](https://www.steptools.com/stds/stp_aim/html/t_representation_map.html),
[context_dependent_shape_representation](https://www.steptools.com/stds/stp_aim/html/t_context_dependent_shape_representation.html),
[item_defined_transformation](https://www.steptools.com/stds/stp_aim/html/t_item_defined_transformation.html),
[si_unit](https://www.steptools.com/stds/stp_aim/html/t_si_unit.html) and
[conversion_based_unit](https://www.steptools.com/stds/stp_aim/html/t_conversion_based_unit.html).
These references do not establish full standards compliance.
