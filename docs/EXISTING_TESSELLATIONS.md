# Existing tessellation import

Milestone 20 imports the triangulated tessellations that a STEP file already
carries as an owned `tessstep_mesh::Mesh`, without retessellating B-rep geometry.
`tessstep_import::import_tessellated` takes a selected root entity ID and a positive
source-length scale. Mesh positions are metres and triangle `face_ids` are the
physical STEP IDs of the tessellated faces (or of the surface set itself).

This is a bounded import profile, **not AP242 conformance**. The original reduced
[tessellated profile](../corpus/geometry/tessellated.exp) supplies generated metadata
for named-attribute decoding. Only the selected root's local reference closure is
checked. `FILE_SCHEMA`, unrelated records, representation contexts, product graphs,
placements and units are not validated or selected automatically.

## Supported input

| Contract | Supported |
| --- | --- |
| Root | `TESSELLATED_SOLID`, `TESSELLATED_SHELL`, `TRIANGULATED_SURFACE_SET`, `COMPLEX_TRIANGULATED_SURFACE_SET` |
| Solid/shell items | `TRIANGULATED_FACE`, `COMPLEX_TRIANGULATED_FACE` |
| Points | `COORDINATES_LIST`, shared by any number of faces |
| Triangles | Explicit index triples, triangle strips and triangle fans |
| Normals | None, one per face, or one per face-local point |
| Links | `TESSELLATED_FACE.geometric_link`, `TESSELLATED_SOLID.geometric_link` and `TESSELLATED_SHELL.topological_link`, retained as entity IDs |

Link targets (faces, surfaces, B-rep solids and shells) are outside the profile.
The decoder retains them through explicit physical-profile *link slots*: a link must
be `$` or a reference to a local entity, but neither the target nor its closure is
decoded or type-checked. A linked tessellation therefore imports even when the
linked B-rep uses geometry that no TessSTEP profile supports. Links are not compared
with the tessellation; a link to disagreeing geometry is accepted and reported as-is.

## Interpretation

Indices are 1-based. When `pnindex` is empty, face-local indices address the
coordinate list directly and `pnmax` must equal `npoints`. Otherwise `pnindex` has
exactly `pnmax` entries, each inside the coordinate list, and face-local index `k`
means point `pnindex[k]`. `npoints` must equal the number of coordinates. Every
triangle, strip and fan index must lie in `1..=pnmax`. A complex face needs at least
one strip or fan.

Strips alternate orientation so that every triangle keeps the strip's winding:
`(p1,p2,p3)`, `(p3,p2,p4)`, `(p3,p4,p5)`, and so on. Fans produce `(p1,p2,p3)`,
`(p1,p3,p4)`, and so on. A strip or fan triangle that repeats an index, the usual
idiom for stitching strips together, is omitted and counted by
`skipped_degenerate()`. An explicit triangle that repeats an index is rejected.

Vertex identity is `(COORDINATES_LIST entity, point index)`. Positions are never
welded by proximity. Faces that use separate coordinate lists, or duplicated points
within one list, are therefore not connected across those points. A solid encoded
that way fails closure with a message that names this rule.

Supplied normals are normalized and must have a positive dot product with the
normal implied by each triangle's winding. A normal that opposes the winding is
rejected rather than flipped, because either the winding or the normal would have
to be guessed. Without supplied normals, corners receive the faceted winding normal.
`TessellatedFace::supplied_normals` records which case applied. UVs are zero; no
surface parameterization is carried by these entities.

Every mesh passes the owned-mesh checks: finite positions, nondegenerate and unique
triangles, oriented manifold edges and vertices. `TESSELLATED_SOLID` additionally
requires one closed component with positive algebraic volume. Shells and surface sets
may be open or disconnected. Inverted solids are rejected, not reoriented. As with
B-rep meshes, closure is combinatorial and does not prove nonintersection.

Errors use the shared import error with `Profile`, `Geometry` and `Topology` stages.
The tessellation stage does not apply. `ImportLimits::max_work` bounds decoding and
adapter work, `max_records` counts faces and coordinate lists, and
`tessstep_mesh::Limits` bounds vertices, triangles and mesh validation work.

`TESSELLATED_EDGE`, `TESSELLATED_CONNECTING_EDGE`, `TESSELLATED_VERTEX`,
`TESSELLATED_WIRE`, curve and point sets, cubic Bézier faces,
`REPOSITIONED_TESSELLATED_ITEM`, presentation occurrences, representation contexts,
unit and root discovery, and the choice between B-rep and tessellated
representations of one product remain outside this profile. Profile rejection is
not an AP validity verdict.

## Example

```sh
cargo run -p tessstep-import --example tessellated -- corpus/geometry/tessellated-cube.step 1000 0.001
# Unmodified NIST tessellated export, when the external corpus is installed:
cargo run -p tessstep-import --example tessellated -- ~/step-corpus/vendor/nist-pmi/unpacked/NIST-PMI-STEP-Files/nist_ftc_08_asme1_ap242-e1-tg.stp 11436 0.001
```

```rust
use tessstep_import::{ImportLimits, import_tessellated};
let document = tessstep_model::parse(bytes, tessstep_part21::ParseLimits::default())?;
let imported = import_tessellated(
    &document,
    tessstep_part21::EntityId::new(1000).unwrap(),
    tessstep_math::LengthUnit::MILLIMETRE,
    ImportLimits::default(),
    tessstep_mesh::Limits::default(),
)?;
let mesh = imported.mesh(); // metres; face_ids are STEP face IDs
```

## Evidence

Authored fixtures cover a box that uses every encoding (explicit triangles with and
without `pnindex`, strips, fans, per-point, single and absent normals, a nonunit
normal, a stitching triangle and a face link), an open shell with face and shell
links to B-rep geometry, an open solid and an unsupported `TESSELLATED_EDGE` item.
Rust tests add typed, located rejections for count, index, normal, orientation,
degeneracy, coordinate-list identity and budget violations, record-order invariance
and 300 deterministic byte mutations.

`python3 scripts/check_geometry.py --require-external` also checks three
unmodified exporter files, hash-pinned in
[external-tessellated.json](../corpus/geometry/external-tessellated.json):

| Input | Result |
| --- | --- |
| NIST FTC-08 tessellated solid (`#11436`) | 272 faces, all linked to B-rep surfaces; 20 stitching triangles skipped; 1,636 vertices, 3,368 triangles; closed; 0.000503 m³ |
| CATIA V5 cuboid (`#32`) | 6 strip faces; 8 vertices, 12 triangles; closed; 0.000772403 m³, matching its 101.43797 × 76.14538 × 100 mm extents |
| HOOPS Exchange shell (`#92`) | 1 face; 81 vertices, 128 triangles; open with 32 boundary edges |

`cargo bench -p tessstep-import --bench tessellated` measures a generated closed
grid box; see [benchmarks](../benchmarks/README.md).
