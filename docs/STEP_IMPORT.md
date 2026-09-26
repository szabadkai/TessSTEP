# Planar STEP solid import

Two explicit import profiles convert a selected STEP solid into a validated B-rep
and owned solid triangle mesh. Rust, C and C++ expose both pipelines. Mesh positions
are metres; triangle `face_ids` identify the original STEP face entities.

These are bounded geometry profiles, **not AP203/AP214/AP242 conformance**. The
original reduced [faceted](../corpus/geometry/faceted.exp) and
[edge-based](../corpus/geometry/planar.exp) EXPRESS profiles supply generated metadata
for named-attribute decoding. Only the selected root's local reference closure is
checked. `FILE_SCHEMA`, unrelated records, product graphs, representation contexts
and units are not validated or automatically selected. Callers supply a root entity
ID and a positive source-length scale.

## Supported input

| Contract | Faceted profile | Edge-based planar profile |
| --- | --- | --- |
| Root | `FACETED_BREP` | `MANIFOLD_SOLID_BREP` |
| Shell / faces | One `CLOSED_SHELL`; planar `FACE_SURFACE` / `ADVANCED_FACE` | Same |
| Boundaries | `POLY_LOOP` point lists | `EDGE_LOOP`, `ORIENTED_EDGE`, `EDGE_CURVE`, `VERTEX_POINT` |
| Curves | Polygon segments | Declared `LINE` / `VECTOR` / `DIRECTION` |
| Outer bound | Explicit `FACE_OUTER_BOUND` | A single plain `FACE_BOUND` is sufficient; multiple bounds require one explicit outer |
| Shared identity | Cartesian-point IDs and point-pair edges | Vertex-point and edge-curve IDs |

Both profiles interpret face/bound orientation, plane placements and optional
placement directions. The structural decoder's complex-mapping restrictions apply.
Coincident distinct topology entities are never welded by proximity.

The edge-based adapter retains the declared line origin and direction, converts
vector magnitude to metres, projects both endpoints to obtain line trim parameters,
and checks endpoint agreement against model tolerance. `EDGE_CURVE.same_sense`
must agree with parameter direction. Canonical increasing ranges preserve shared
edges; coedge orientation composes the edge sense, oriented-edge direction, bound
orientation and face sense. Plane projection builds linear pcurves with affine
parameter correspondence. Off-line/off-plane endpoints and contradictory directions
fail rather than replacing the line with an invented segment.

Both physical `ORIENTED_EDGE` endpoint slots must be `*`. By default the importer
also accepts `$` there, a common exporter deviation that carries no geometry: the
slots are derived, so their values are never read. `ImportOptions::strict` (C:
`TS_IMPORT_STRICT` in a `ts_import_policy` passed to
`ts_document_tessellate_planar_with_policy`; C++: `ImportPolicy`) rejects `$` in those
slots. Explicit vertex references in them are rejected in both modes. An explicit
physical-profile mapping recognizes exactly those named slots. The adapter derives their meaning
from `EDGE_ELEMENT` and `ORIENTATION`. Ordinary schema decoding still rejects
unsupported derived attributes: no general EXPRESS DERIVE evaluation or suppression
of schema diagnostics is claimed.

The independent topology validator, UV loop checks, shared-edge sampler, triangulator
and manifold solid checks run before a mesh is published. Successful solid meshes
have one closed manifold component and positive algebraic volume. These checks do
not prove nonintersection or complete CAD validity. Unsupported geometry, malformed
winding and missing references produce typed errors without a partial mesh.
An entity type outside the reduced profile is reported by name (every component of
a complex instance, marking those outside the profile), as outside the import
profile rather than as unknown to STEP.

Curved edges/surfaces, `BREP_WITH_VOIDS`, enclosed cavity shells, multiple bounds
without an explicit outer, assembly placement, automatic units, STEP appearance and
healing remain outside these profiles. Profile rejection is not an AP validity verdict.

## Edge-based example

```sh
cargo run -p tessstep-import --example planar -- corpus/geometry/planar-box.step 1000 0.001
# Unmodified ST-DEVELOPER export, when the external corpus is installed:
cargo run -p tessstep-import --example planar -- ~/step-corpus/vendor/foxtrot/examples/cuboid.step 121 1
```

The external cuboid measures 0.0508 × 0.0254 × 0.0762 metres. It produces 8 shared
vertices, 12 triangles and volume 0.000098322384 m³. Its original bytes are hash-pinned
in [external-planar.json](../corpus/geometry/external-planar.json); upstream revision
and license are recorded in [sources.json](../corpus/sources.json). No external model
is redistributed in the repository or release assets.

Use `import_planar_solid` in place of `import_faceted_solid` in the Rust example
below. In C, use `ts_planar_options_init` and `ts_document_tessellate_planar`; in C++,
use `default_planar_options()` and `Document::tessellate_planar`. The options alias
has the same frozen scalar layout, units and lifetime contract as the faceted API.

## Rust

```rust
use tessstep::{import, math, part21, parse};
use std::{fs::File, io::BufReader};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let document = parse(BufReader::new(File::open("corpus/geometry/box.step")?),
    part21::ParseLimits::default())?;
let model_tolerance = math::ModelTolerance::new(
    math::Length::metres(1e-8)?, math::Angle::radians(1e-8)?)?;
let solid = import::import_faceted_solid(&document,
    part21::EntityId::new(1000).unwrap(), math::LengthUnit::MILLIMETRE,
    model_tolerance, import::ImportOptions::default())?;
let tolerance = math::TessellationTolerance::new(
    math::Length::metres(1e-6)?, math::Angle::radians(0.1)?)?;
let mesh = solid.tessellate(tolerance, import::TessellationOptions::default())?;
assert_eq!(mesh.data().triangles.len(), 12);
# Ok(())
# }
```

`ImportedSolid` owns geometry independently of the input document. `brep()` exposes
its immutable normalized geometry; `face_entities()` maps model-local face handles
to physical STEP IDs. `tessellate()` returns an independent owned mesh. Errors retain
stage, category, source entity and source span where available; topology errors may
identify the selected root while their message gives the model-local defect.

A runnable file example emits JSON with distinct profile, geometry and tessellation
outcomes:

```sh
cargo run -p tessstep-import --example faceted -- corpus/geometry/box.step 1000 0.001
```

## C and C++

Use `ts_faceted_options_init` and `ts_document_tessellate_faceted` in C. Success
transfers one `ts_mesh` acquisition, released with `ts_mesh_release`. Failure clears
the mesh output and optionally fills a value-only `ts_import_error` with stage,
entity and byte offsets. `TS_INVALID_GEOMETRY` is a new additive status value;
unsupported input, missing entities and resource exhaustion retain distinct statuses.
Existing ABI 1 layouts and function signatures are unchanged.

```cpp
// bytes contains the contents of corpus/geometry/box.step.
auto parsed = tessstep::Document::parse(bytes);
if (!parsed) return 1;
auto options = tessstep::default_faceted_options();
auto result = parsed.value().tessellate_faceted(1000, 0.001, &options);
if (!result) {
    // result.error().code and .import_failure.{stage,entity_id,start_offset,end_offset}
    return 2;
}
auto view = result.value().view();
if (!view) return 3;
// view owns a retained reference; its read-only buffers survive both objects above.
const auto& data = view.value().data();
```

The C initializer supplies documented numerical defaults: model distance/angle
`1e-8`, chord `1e-5`, normal angle `0.1`, and UV tolerance `1e-8`. Distances and
plane UVs are metres; angles are radians. The source unit scale is a required
separate argument. Zero budgets mean zero. `max_work` limits stages independently,
not total wall-clock time or memory; underlying hard limits still apply.

## Evidence and limits

`python3 scripts/check_geometry.py` verifies generated profile reproducibility and
writes [per-file stage results](../reports/geometry/index.html). Authored fixtures
cover a 10 × 20 × 30 mm box, a rectangular through-hole, missing references,
nonplanarity, duplicate points, open topology and an unsupported curved surface.
The corresponding Rust tests verify independent volume, extents, outward normals,
face provenance, orientation/default semantics, record-order invariance, identity
preservation, budgets and 500 deterministic byte mutations.

When available, four external adversarial faceted fixtures are additionally checked
for rejection/unsupported outcomes. The unmodified exporter cuboid is tested through
the Rust pipeline and relocated C++ consumers, including dimensions, volume, normals
and mesh lifetimes. This establishes one reviewed exporter case, not general CAD
coverage. The full external corpus run imports every solid root with these
profiles and reports per-root outcomes by stage and error category; see
[corpus testing](corpus-testing.md#solid-import-stage). Those are observations,
not reviewed expectations.
Installed C11/C++17 consumers exercise file parsing, conversion, errors, units and
mesh/view lifetime after document release in relocated Debug/Release packages.


The edge-based suite adds a box, a through-hole, off-line endpoint, incorrect edge
sense, `$` derived-slot marker (accepted by default, rejected when strict), open shell
and unsupported curve cases. Independent
Rust tests cover reversed edge/bound/face combinations, shifted line origins,
nonunit vector magnitudes, source-face identity, no proximity welding, order
invariance and 300 deterministic byte mutations. `check_geometry.py --require-external`
fails if the pinned cuboid is unavailable or changed. CI runs this after acquiring
the external corpus and also runs installed C++ consumers against it. Temporary
consumer-test copies of external data are never placed in the SDK or release archive.
