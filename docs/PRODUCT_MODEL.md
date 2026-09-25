# Products, representations and assembly semantics

Milestone 5 delivers bounded local 3D product semantics over explicitly supplied,
structurally decoded schemas: products, formations, definitions, shapes, contexts,
units, uncertainty measures, reusable representations, mapped items and placed assembly
occurrences. STEP placements are evaluated with the independent checked math layer.
This is not full AP242 conformance or STEP B-rep/mesh adaptation.

## Entry points and ownership

`tessstep-product` depends only on `tessstep-math` and can be constructed without STEP.
`Model::new(parts, max_work)` checks caller-owned `Parts` and publishes an immutable
model. Typed IDs identify products, formations, definitions, shapes, representations,
occurrences and items; they are sparse identities, never allocation sizes.

`tessstep-ap242::adapt(decoded, schema_name, limits)` reads named attributes and resolved
schema memberships and returns an owned product model. The umbrella exports these APIs
as `tessstep::ap242` and `tessstep::product`. No AP inheritance hierarchy or physical
parameter offsets are hand encoded. Callers must pass the structural decoder first;
unsupported EXPRESS rules are not suppressed or evaluated by the adapter.

```rust,ignore
let decoded = tessstep::model::decode::decode(
    &document, &bindings::SCHEMA_SET, "MY_SCHEMA", Default::default(),
)?;
let model = tessstep::ap242::adapt(&decoded, "MY_SCHEMA", Default::default())?;
let instances = model.expand(root_definition, root_representation,
    &std::collections::BTreeMap::new(), Default::default())?;
```

Output owns its strings, graph records and evaluated transforms. Adapter ID numbers
preserve physical IDs. Opaque geometry items remain provenance: retain the physical
model for later geometry interpretation. No ADVANCED_FACE scan discovers geometry,
and no mesh or topology validity follows from product-stage acceptance.

## Product and representation relationships

| Input role | Interpretation |
| --- | --- |
| product → product_definition_formation → product_definition | Product identity, version and design definition |
| product_definition_shape → shape_definition_representation | Shape identity and representation binding |
| representation → context_of_items/items | Reusable representation, explicit units and opaque item identities |
| next_assembly_usage_occurrence | Distinct immediate parent/child usage |
| representation_relationship_with_transformation | Ordered endpoints with item-defined or Cartesian transformation |
| context_dependent_shape_representation | Usage-to-representation-relationship link |
| shape_representation_relationship without transformation | Transitive shape ownership association |
| representation_map / mapped_item | Reusable source, origin, target and evaluated source-to-user placement |

Products, versions, definitions, shapes and occurrences remain distinct. Repeated parts
share definition/representation records. Reference closure through representation items
resolves indirect context membership, including placement points/directions and nested
item groups. A map's source is a context boundary; its source items are not imported into
the using context by this traversal. Cyclic item references terminate through visited sets.

Untransformed shape associations extend definition-to-representation ownership in both
directions, including cyclic association graphs. Assembly placement edges do not extend
ownership. An occurrence's relationship endpoints must belong to its parent and child
shapes. Ordered item-defined endpoints must belong to the corresponding representation
contexts. Assembly definition cycles and mapped-representation cycles are rejected,
including disconnected cycles. Multiple representations and placements are preserved.

## Placement contract

All evaluated transforms operate on **metre-based coordinates**, use column vectors
and store row-major linear matrices. The adapter evaluates:

- `axis2_placement_3d`, with normalized directions, projected x reference, default +Z
  axis, and default +X reference (or +Y when Z is exactly parallel to X).
- Item-defined relationships: `rep_1_to_rep_2 = frame_2 * inverse(frame_1)`. Each frame
  origin uses its own representation context's length unit.
- 3D Cartesian operators: projected orthogonal axes, default scale 1, positive scale,
  destination-unit origin and dimensionless scale. The second axis preserves handedness;
  zero/degenerate projections fail. No silent axis repair is performed.
- A supplied `cartesian_transformation_operator_3d_non_uniform` schema extension with
  optional scale2/scale3 defaulting to scale. This supplemental role does not claim that
  every STEP AP declares such an entity.
- Mapped items: `source_to_using = target * inverse(mapping_origin)`, with origin and
  target translations converted in their respective contexts. Targets may be axis2
  placements or supported operators. Unit conversion never becomes an extra shape scale.

`Parts::relationship_transforms` and `Parts::mapped_transforms` expose immutable evaluated
maps while preserving source descriptors. Unsupported placement kinds fail explicitly.
Absent transformation remains absent, not identity. The checked math layer rejects
non-finite results, unusable inverses and nearly parallel axes at its documented numerical
threshold. These are floating-point safeguards, not exact predicates or error certificates.

## Assembly expansion

`Model::expand(root_definition, root_representation, selections, limits)` explicitly
instantiates the definition DAG into a preorder vector. Each `Instance` records a parent
index, source occurrence/definition/representation IDs, local-to-parent and local-to-world
transforms. Repeated subassemblies expand into distinct paths without copying geometry.
The chosen root has identity world placement. World composition applies the child-to-parent
map first, then the parent's world map; reversed relationship endpoints are inverted.

The caller chooses the root representation. Multiple matching placement relationships
return `AmbiguousPlacement` unless the caller selects a relationship for that occurrence.
Missing transforms return `MissingPlacement`. Same-context untransformed shape associations
can connect coordinate frames during traversal; cross-context ownership association alone
does not establish a world placement. No external resources or arbitrary alternatives are
selected. Output contains no partial expansion on failure.

Defaults: 5,000,000 work steps, 100,000 instances, 1,024 levels. Traversal is iterative;
instance and work limits stop exponential DAG expansion before unbounded output allocation.
The independent mesh scene API can consume these explicit transforms and caller-selected
mesh assets; automatic STEP geometry-to-mesh asset binding remains a separate task.

## Units and uncertainty

Each supported 3D geometric context supplies exactly one length, plane-angle and solid-angle
unit. No defaults are invented. SI metre/radian/steradian units with the STEP prefix set
atto–exa and positive conversion chains are normalized. Conversion role and all seven
explicit dimensional exponents must agree; cycles, overflow and nonzero underflow fail.
Names such as “inch” do not override numeric conversion factors.

`Unit::to_si` returns metres, radians or steradians. Placement origins and uncertainty
values are converted; opaque geometry coordinates are not. Global uncertainty assignments
retain every positive measure, its dimension, source ID, name and optional description.
Each measure uses its own unit, independently of the context unit. The adapter never
chooses a tessellation tolerance from an uncertainty value or merges competing measures.

## Budgets and conformance boundaries

Adapter defaults: 5,000,000 work steps, 16,000,000 copied text bytes, 128 unit-chain nodes.
Graph validation consumes remaining adapter work. Errors carry typed kinds, physical
owner/attribute/span where available, and math/graph failure kinds. Ordered maps and input
order preserve determinism. Closure and membership scans can be superlinear; these logical
limits are not exact RSS or wall-time bounds.

This milestone covers local immediate single-occurrence 3D assemblies. Quantified,
promissory and specified-higher usage forms are explicitly rejected: they cannot safely
be interpreted as immediate individual instances. Full AP242 occurrence/configuration
variants, external assembly resolution, 2D contexts, offset/other unit dimensions,
administrative metadata and full EXPRESS rule evaluation are outside this slice.
Geometry item evaluation and automatic mesh asset binding remain later adapter work.
C/C++ product operations are not yet exposed; existing physical/mesh/scene interfaces
and their ownership contracts are unchanged. Rust math or container layouts never cross
that ABI.

## Evidence and references

The authored `corpus/product` schema is a reduced synthetic schema, not an ISO extract.
`python3 scripts/check_product.py` verifies generated metadata, eleven reviewed physical /
schema / product outcomes, evaluated matrices, graph counts and uncertainty values.
Rust tests additionally cover reversed transforms, noncommuting nested placement,
reflections, shared representations, indirect context/shape links, ambiguity, missing
placement, numeric failures and a 2,000-level expansion. See [VALIDATION.md](VALIDATION.md).

Axis/default rules were checked against STEP Tools' published
[base_axis](https://www.steptools.com/stds/stp_aim/html/t_base_axis.html),
[first_proj_axis](https://www.steptools.com/stds/stp_aim/html/t_first_proj_axis.html) and
[second_proj_axis](https://www.steptools.com/stds/stp_aim/html/t_second_proj_axis.html)
functions. The [uncertainty reference](https://www.steptools.com/stds/stp_aim/html/t_valid_measure_value.html)
requires positive measures. The
[product structure reference](https://www.steptools.com/stds/smrl/data/resource_docs/product_structure_configuration/sys/4_schema.htm)
distinguishes immediate usages from promised or quantified use. These references inform
this bounded implementation, not a standards certification claim.
