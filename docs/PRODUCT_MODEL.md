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
  origin uses its own representation context's length unit. Frame 1 must belong to
  rep_1's context and frame 2 to rep_2's (AP242 WR2). A pair written the other way
  round, with transform_item_1 only in rep_2's context and transform_item_2 in rep_1's,
  is a located graph error naming its relationship; `adapt_with_policy` with
  `Policy::accept_reversed_transform_items` exchanges the two items instead (see
  [reversed item transformations](#reversed-item-transformations-hoops-exchange)).
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

An occurrence without any placement relationship may instead be placed by a mapped item
(Milestone 22.7), the encoding CATIA- and Creo-style exporters write: a representation
bound to the occurrence's own shape holds a mapped item whose map's source is a
representation of the child, and a representation in the parent instance's frame holds
the same item. That holder's evaluated `source_to_using` map is the child-to-parent
placement, and the child instance uses the map's source representation. Two distinct
qualifying items return `AmbiguousPlacement`. A mapped item held only by the occurrence's
representation places nothing, since that representation's frame is not known to be the
parent's, and the occurrence keeps failing with `MissingPlacement`.

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

## Linking imported shapes (Milestone 21)

`tessstep_import::link_assembly` decodes the document's product records with the
bundled original [product profile](../corpus/geometry/product.exp), which matches the
physical layouts AP203, AP214 and AP242 exporters write: next assembly usage
occurrences, context dependent shape representations, shape definition representations
of product definition shapes and untransformed shape representation relationships are
roots; representation items and associated documents are retained as links, never
decoded. The records are adapted by this crate's adapter (a linked item is an opaque
identity) and each root definition is expanded with `Model::expand`. A shape root is
placed wherever an expanded representation, or one associated with it in the same
context, holds it. A combined closure outside the profile is decoded record by record;
excluded records are reported, and those that are occurrences or placements mark the
structure incomplete. The solid-angle unit of a context is optional, and the tolerant
default accepts `NAMED_UNIT(*)` on conversion-based units (the dimensions follow from
the unit role; strict mode rejects it). External document references are not linked.
Evidence: `tests/assembly.rs`, the product stage of the corpus runner and
`external-assembly.json`.

## Mapped items, operators and tolerated encodings (Milestone 22.7)

The linker also decodes the MAPPED_ITEMs held by occurrence shape representations
(shape definition representations of a next assembly usage occurrence's product
definition shape), each together with the record that binds it; every other mapped
item, including those of draughting models and presentation representations, stays an
opaque link. `Model::expand` then places such occurrences as described above; the
number of occurrence nodes placed this way is `LinkedAssembly::mapped_placements`. An
item-defined, mapped or operator placement is evaluated with the checked Milestone 5
rules: target times inverse mapping origin, each frame in its own context's length unit.

`CARTESIAN_TRANSFORMATION_OPERATOR_3D` is accepted as a relationship's transformation
operator and as a mapping target, in its AP physical layout (item name, the functionally
defined transformation's name and description, axis1, axis2, local origin, scale,
axis3). Its evaluation is an orthonormal frame, possibly mirrored by the second axis,
times one positive uniform scale, which the Milestone 18 scene places as an explicit
affine transform with its reflection flag. Every other operator is a typed rejection
naming it: `CARTESIAN_TRANSFORMATION_OPERATOR_2D`, the supplemental non-uniform operator
and other functionally defined transformations are outside the profile, and a
non-positive scale fails in the adapter. An occurrence whose placement record was
excluded reports the exclusion with its `MissingPlacement`.

Exporters write some unit and measure encodings that Part 21 or the schema reject but
whose meaning is unambiguous. The tolerant default accepts them in the context
(discovery) and product profiles and counts them in `ToleratedEncodings`, reported for
the product records (`LinkedAssembly::tolerated`), each discovered context
(`ContextUnits::tolerated`) and each imported root (`RootImport::tolerated`):

| Count | Encoding | Meaning kept |
| --- | --- | --- |
| `explicit_si_dimensions` | `NAMED_UNIT(#exponents)` on an SI unit instead of `*` (XStep, PSStep, older OpenCASCADE) | exponents checked against the unit role |
| `unset_si_dimensions` | `$` instead of `*` on an SI unit | dimensions follow from the unit name |
| `omitted_conversion_dimensions` | `NAMED_UNIT(*)` on a conversion-based unit (NIST, Creo) | dimensions follow from the unit role |
| `untyped_measures` | a bare number where a typed `LENGTH_MEASURE(…)` is required (debis CATIA V4) | the measure's unit gives the dimension |
| `unordered_complex` | complex instance components out of alphabetical order (XStep units) | components are matched by name |

`ImportOptions::strict` rejects each of them; the affected product records are then
excluded with their typed decode error and the roots of an affected context get a unit
error. Synthetic five-parameter `PRODUCT_DEFINITION`s (the `AttributeCount` rejections
of the generated `dodgy-step-files` inputs) stay rejected in both modes.

## Reversed item transformations (HOOPS Exchange)

HOOPS Exchange's STEP writer (2020.1 and 2024.8 seen, including Shapr3D exports) and the
CATIA V4 STEP interface (CATIA Solutions V4 2.2) place every occurrence with a
`REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION` whose rep_1 is the child and rep_2 the
parent, but whose `ITEM_DEFINED_TRANSFORMATION` lists the parent's placement as
transform_item_1 and the child's frame as transform_item_2. In the Shapr3D layout every product also has its own geometric context, one identity
`AXIS2_PLACEMENT_3D` is an item of every shape representation, the solids sit in a B-rep
representation related to the product's shape representation, and a
`DRAUGHTING_MODEL` maps each shape representation (14 such mapped items in a 13-occurrence
file; they are presentation, never placements).

The written order breaks AP242 WR2 of representation_relationship_with_transformation,
and read literally the frames would belong to the wrong representations. The adapter
assigns the items by membership instead: when transform_item_1 is outside rep_1's
context (item closure) and inside rep_2's, and transform_item_2 is inside rep_1's, that is
the only consistent reading and the pair is exchanged, so the child is mapped onto the
parent's placement (`rep_1_to_rep_2 = parent_frame * inverse(child_frame)`). The linker
enables this unless strict and counts each exchanged pair as
`ToleratedEncodings::reversed_transform_items` (Rust only; the frozen C record
`ts_tolerated_encodings` keeps its five counts). Strict mode, and `adapt` without the
policy, reject the relationship with a located graph error. A pair whose items fit
neither order is still a graph error. Evidence: `assembly-reversed-items.step`
(`reversed_item_transformations_are_read_by_membership_and_counted`: the world boxes
equal the conformant nested fixture's, and the literal reading would move the second
block 40 mm), the adapter test `product_adapter_rejects_cycles_and_misbound_placements`,
and the corpus files `bug33410.step` (HOOPS Exchange 2020.1) and `trj3_s1-ct-203/214.stp`
(CATIA V4, non-identity placements whose membership reading arranges the CAx-IF S1 model
as the NX and Pro/ENGINEER exports of the same model do).

## Surface models in assemblies (Milestone 23.10)

Shape roots are solids and shape tessellations unless the caller asks for surface
models: `AssemblyOptions::surface_models` (off by default) makes `link_assembly` and
`import_assembly` treat every `SHELL_BASED_SURFACE_MODEL` that discovery finds
([CURVED_IMPORT.md](CURVED_IMPORT.md#surface-models)) as a shape root. It is placed by
the rules above, wherever an expanded representation (typically a
`MANIFOLD_SURFACE_SHAPE_REPRESENTATION`) or one associated with it holds it, so
occurrences, mapped items and operators place it as they place a solid. A surface
model with no placing representation is listed in `LinkedAssembly::unplaced`, and one
in a placed representation without supplied shells in `unimported`, as for solids.
With the option off, surface models are neither imported, placed nor listed, and every
earlier result is unchanged.

`import_assembly` imports each surface model with its context units and the model
tolerance a solid in that context gets, through the session's shared curved-profile
decodes (surface models are prepared together, apart from the solids). Each declared
shell is validated as declared and meshed as a shell, never as a solid: no volume or
orientation is checked, and an open shell keeps its boundary. Every shell becomes its
own scene asset: `ShapeAsset::shell` names the shell, its `AssetId` is the shell's
entity ID (`ShapeAsset::asset`), and its leaves carry `AssemblyNode::shell` beside
`root`, the surface model. The first shell that fails to validate or mesh fails its
root; `RootImport` reports the root with `ShapeRootKind::SurfaceModel`, always
selected (representation selection does not pair surface models), and its failure,
located and with its text. A shell that an earlier surface model already supplied fails the
later root (`Unsupported` at the product stage, naming both); no corpus file lists a
shell twice.

The opt-in repair stage (`ImportOptions::repair`) applies to surface models here, and
only here: the single-root `import_surface_model` keeps ignoring it. Sewing plans
merges inside one declared shell only, since `plan_sewing` takes candidate vertices
used by one shell and pairs free edges per shell, and an edge or vertex two shells
share is never free; face orientation is made consistent per shell; and the
volume-sign reorientation, which applies to solid shells, does not run. Orientation
repair leaves a surface model that validates as declared unchanged; sewing may still
merge coincident boundary edges of an open shell. Sewn, a disconnected open shell can
close, while it keeps its declared open closure (`SurfaceShell::closed`).

**Bodies are the consumer's choice.** `ImportedAssembly::asset_class` reports each
asset as `Solid` (a B-rep, faceted or tessellated solid), `ClosedShell` (a shell mesh
without boundary) or `OpenShell`, read from the mesh. Which of them become printable
bodies is the consumer's policy, not the importer's. A surface model can be a
construction, cosmetic or thread surface in the same space as a solid: in PP-31465 a
SOLIDWORKS thread surface body lying in the same space as its solid filled a hole
when HOOPS Exchange imported it and it was printed. Keeping closed shells only, or
surface models only from documents without solids, are such policies; PreForm decided
on the second (2026-10-19), which `ImportedAssembly::has_solid_roots` (any solid root
of the document, imported or not; `ShapeRootKind::is_solid`) states. In the corpus,
35 real inputs import solids beside surface models and gain only open shells (95),
and 11 NIST files of the PreForm collection gain 29 open construction surfaces beside
their solids ([VALIDATION.md](VALIDATION.md)). Evidence (`tests/assembly.rs`, fixtures
from [`assembly_fixtures.py`](../corpus/geometry/assembly_fixtures.py) and
[`surface_fixtures.py`](../corpus/geometry/surface_fixtures.py)):
`surface_models_join_an_assembly_only_on_request` (a block and an open trough placed
twice, default off),
`a_closed_surface_model_imports_as_a_watertight_shell_asset_not_a_solid` (a closed
cylinder shell part and an unplaced open model),
`surface_model_failures_are_reported_per_root_with_their_text` and
`opt_in_repair_sews_each_declared_shell_of_a_surface_model_on_its_own`; the merge
counts in `sewing_merges_within_one_declared_shell_only`; and the session's sharing in
`tests/session.rs`.

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
