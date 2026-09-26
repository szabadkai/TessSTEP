# Existing tessellation import

Milestone 20 imports the tessellations that a STEP file already carries, without
retessellating B-rep geometry. It has two outputs:

- **Shape tessellations** (`TESSELLATED_SOLID`, `TESSELLATED_SHELL`, triangulated
  surface sets) become an owned, validated `tessstep_mesh::Mesh`, through
  `tessstep_import::import_tessellated`.
- **Presentation tessellations** (`TESSELLATED_ANNOTATION_OCCURRENCE`,
  `TESSELLATED_GEOMETRIC_SET`) become placed graphics (polylines, points and fill
  triangles), through `tessstep_import::import_presentation`. They are not meshes.

Both take a selected root entity ID and a positive source-length scale; positions are
metres. `discover_tessellations` and `discover_presentations` find roots and their
context units, and `select_representations` chooses between the B-rep and tessellated
representations of one shape. Both imports are available in Rust, C and C++.

This is a bounded import profile, **not AP242 conformance**. The original reduced
[tessellated profile](../corpus/geometry/tessellated.exp) supplies generated metadata
for named-attribute decoding. Only the selected root's local reference closure is
checked. `FILE_SCHEMA`, unrelated records, product graphs and placements are not
validated or applied.

## Shape tessellations

| Contract | Supported |
| --- | --- |
| Root | `TESSELLATED_SOLID`, `TESSELLATED_SHELL`, `TRIANGULATED_SURFACE_SET`, `COMPLEX_TRIANGULATED_SURFACE_SET` |
| Solid/shell items | `TRIANGULATED_FACE`, `COMPLEX_TRIANGULATED_FACE`, `TESSELLATED_EDGE`, `TESSELLATED_CONNECTING_EDGE`, `TESSELLATED_VERTEX` |
| Points | `COORDINATES_LIST`, shared by any number of items |
| Triangles | Explicit index triples, triangle strips and triangle fans |
| Normals | None, one per face, or one per face-local point |
| Links | `GEOMETRIC_LINK` of faces, edges and solids and `TOPOLOGICAL_LINK` of shells and vertices, retained as entity IDs |

Link targets (faces, surfaces, curves, vertex points, B-rep solids and shells) are
outside the profile. The decoder retains them through explicit physical-profile *link
slots*: a link must be `$` or a reference to a local entity, but neither the target
nor its closure is decoded or type-checked. A linked tessellation therefore imports
even when the linked B-rep uses geometry that no TessSTEP profile supports. Links are
not compared with the tessellation; a link to disagreeing geometry is accepted and
reported as-is.

### Faces and identity

Indices are 1-based. When `pnindex` is empty, face-local indices address the
coordinate list directly. Otherwise `pnindex` has exactly `pnmax` entries, each inside
the coordinate list, and face-local index `k` means point `pnindex[k]`. `npoints` must
equal the number of coordinates. Every triangle, strip and fan index must lie in
`1..=pnmax`. A complex face needs at least one strip or fan.

Without `pnindex` the standard requires `pnmax = npoints`. Some exporters write a
smaller `pnmax` while indexing the whole list (975 annotation fill sets in three
HOOPS Exchange exports of SOLIDWORKS MBD models in the corpus, and no others). Because that value is then
redundant, the tolerant default accepts `pnmax < npoints` in this case, bounds
indices by `npoints`, and counts each such face in `pnmax_deviations()`. Per-point
normals would be ambiguous, so a face with more than one normal is still rejected.
`ImportOptions::strict` (C `TS_IMPORT_STRICT`) rejects every mismatch.

Strips alternate orientation so that every triangle keeps the strip's winding:
`(p1,p2,p3)`, `(p3,p2,p4)`, `(p3,p4,p5)`, and so on. Fans produce `(p1,p2,p3)`,
`(p1,p3,p4)`, and so on. A strip or fan triangle that repeats an index, the usual
idiom for stitching strips together, is omitted and counted by
`skipped_degenerate()`. An explicit triangle that repeats an index is rejected.

Vertex identity is `(COORDINATES_LIST entity, point index)`. Positions are never
welded by proximity. Faces that use separate coordinate lists, or duplicated points
within one list, are therefore not connected across those points unless a connecting
edge says so (below). A solid encoded that way without connecting edges fails closure
with a message that names this rule.

### Edges, vertices and connecting edges

`TESSELLATED_EDGE` and `TESSELLATED_VERTEX` items add no triangles. Each edge's line
strip and each vertex's point index must lie inside its own coordinate list, and a line
strip may not repeat a consecutive point. `ImportedTessellation::edges()` returns each
strip's points in metres and, per point, the mesh vertex with the same coordinate
identity (or `None`: a point in an unrelated list is never matched by proximity).
`vertices()` does the same for vertex items.

A `TESSELLATED_CONNECTING_EDGE` also names two faces of the same shell or solid and a
face-local strip for each (`line_strip_face1`, `line_strip_face2`, face-local indices
as for triangles). It declares that the *i*-th point of its line strip and of both
face strips are one point. The importer joins exactly those coordinate identities
before creating mesh vertices, so faces with separate coordinate lists close through
their connecting edges. Joined identities must have bit-identical coordinates, each
face strip must have the line strip's length, and every strip segment must be a
triangle edge of its face. A free boundary written as a connecting edge to an
unrelated face therefore fails with a located error rather than implying closure.
`joined_points()` counts identities joined to a different identity; `smooth` is
reported as `Some(bool)` or `None` for `.U.`.

### Normals, validity and errors

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
The tessellation stage does not apply. Entity types outside the profile are named in
the diagnostic. `ImportOptions::max_work` bounds decoding and adapter work,
`max_records` counts faces, edges, vertices and coordinate lists, and
`tessstep_mesh::Limits` bounds vertices, triangles and mesh validation work.

## Presentation tessellations

| Contract | Supported |
| --- | --- |
| Root | `TESSELLATED_ANNOTATION_OCCURRENCE` whose styled item is a `TESSELLATED_GEOMETRIC_SET`, or the geometric set itself |
| Children | `TESSELLATED_CURVE_SET`, `TESSELLATED_POINT_SET`, `TRIANGULATED_SURFACE_SET`, `COMPLEX_TRIANGULATED_SURFACE_SET`, nested `TESSELLATED_GEOMETRIC_SET` |
| Placement | `REPOSITIONED_TESSELLATED_ITEM` complex instances on any set, with an `AXIS2_PLACEMENT_3D` |
| Styles | The occurrence's style assignments, retained as entity IDs |

Children are visited depth first in child order; a child shared by two sets is
emitted once per use. Each line strip of a curve set becomes one polyline, each
point-set entry one point and each surface-set triangle one fill triangle. Every
primitive records the physical ID of its source set, and `items()` lists each leaf use
with its kind and the number of placements composed for it.

`REPOSITIONED_TESSELLATED_ITEM` locations are composed from the root down, with the
ISO 10303-42 axis defaults and the reference direction projected orthogonal to the
axis; explicit parallel axes are rejected. Positions are metres in the coordinate
system of the representation that contains the root (for example a draughting
model). Vertex identity is `(coordinate list, point index, placement)`, so one list
used under two placements yields two vertices.

Indices, counts, strips and fans are checked as for shape tessellations, including the
tolerated `pnmax` deviation, and a line strip may not repeat a consecutive point.
Graphics are deliberately **not** meshes, because real annotation data would fail mesh
rules without being wrong as graphics:

- **Normals are checked for count and shape only, and not published.** Of 4,320
  annotation fill sets in the corpus, 734 carry a normal perpendicular to or opposing
  their winding. In repositioned CATIA exports the normal is written in the model frame
  while the points are in the local frame.
- **Fill triangles are kept in the file's winding**, with no orientation, manifoldness
  or uniqueness claim.
- **Zero-area triangles are kept and counted** (`zero_area_triangles()`); 293 fill sets
  in the corpus contain them.

Styles, fonts, colours, text semantics, view and plane associations and PMI meaning
are not interpreted. Occurrences combined with `OVER_RIDING_STYLED_ITEM` in one complex
instance are outside the profile. `PresentationLimits` bounds vertices, triangles,
polyline points and nesting depth; `ImportOptions` bounds records and work.

## Discovery and representation selection

`discover_tessellations` finds every `TESSELLATED_SOLID` and `TESSELLATED_SHELL`, and
every triangulated surface set that is directly an item of a representation. It
reports each root's physical link and the units of its representations, following the
same rules as `discover_solids`: missing or conflicting units are per-root errors,
never defaults. Surface sets reached only through geometric sets are graphics.

`discover_presentations` finds every `TESSELLATED_ANNOTATION_OCCURRENCE`, the
`*_CALLOUT` and `ANNOTATION_PLANE` records that contain it (planes may contain
callouts), and the representations whose items include the occurrence or one of
those containers. Draughting models, including characterized complex instances, are
decoded with the bounded context profile. The standard derives
`CHARACTERIZED_OBJECT`'s attributes, which some exporters write as `*` and others as
values. The tolerant default accepts either through an explicit profile slot; strict
requires `*`.

`select_representations` groups the roots that describe one shape and selects one:

1. A tessellated solid whose `GEOMETRIC_LINK` names a solid root, or a tessellated
   shell whose `TOPOLOGICAL_LINK` names a solid root's outer shell, pairs with it.
2. Otherwise a `(SHAPE_)REPRESENTATION_RELATIONSHIP` without transformation pairs a
   representation holding exactly one tessellated root with one holding exactly one
   solid root. Ambiguous representations are never paired, and transformed
   relationships place representations; they are not alternatives.

`RepresentationPreference::Exact` selects the B-rep and lists the tessellations as
alternatives; `Tessellated` selects the first tessellation. Every discovered root
appears exactly once. Selection imports nothing; callers may fall back to an
alternative when the selected root fails. Linking roots to product definitions remains
with the product-graph work.

## C and C++

`ts_document_import_tessellated` returns an owned `ts_mesh` plus an optional
`ts_tessellated_info` record (kind, link, face/edge/vertex counts, skipped, joined and
tolerated-deviation counts). `ts_document_import_presentation` returns a retained
`ts_presentation` handle with scalar info, item and style queries and a zero-copy
`ts_presentation_view`. C++ wraps both as `Document::import_tessellated` (returning
`TessellatedImport{Mesh, TessellatedInfo}`) and `Document::import_presentation`
(returning a move-only `Presentation` whose `PresentationView` retains the graphics).
Edge polylines, vertex items and per-face links are Rust-only; C reports their counts.
Discovery and representation selection are Rust-only. See [C_API.md](C_API.md).

## Outside the profile

Cubic Bézier faces, `TESSELLATED_WIRE`, `REPOSITIONED_TESSELLATED_ITEM` on solids and
shells, style decoding and presentation semantics are outside this profile. So are
product-definition linking of discovered roots and appearance binding of annotation
styles. Profile rejection is not an AP validity verdict.

## Examples

```sh
cargo run -p tessstep-import --example tessellated -- corpus/geometry/tessellated-connected.step 1000 0.001
cargo run -p tessstep-import --example presentation -- corpus/geometry/presentation.step 300 0.001
cargo run -p tessstep-import --example survey -- corpus/geometry/tessellated-selection.step 0.001
# Unmodified exporter files, when the external corpus is installed:
cargo run -p tessstep-import --example tessellated -- ~/step-corpus/vendor/nist-pmi/unpacked/NIST-PMI-STEP-Files/nist_ftc_08_asme1_ap242-e1-tg.stp 11436 0.001
cargo run -p tessstep-import --example presentation -- ~/step-corpus/vendor/nist-pmi/unpacked/NIST-PMI-STEP-Files/nist_ctc_02_asme1_ap242-e2.stp 1023 0.001
```

```rust
use tessstep_import::*;
let document = tessstep_model::parse(bytes, tessstep_part21::ParseLimits::default())?;
for root in discover_tessellations(&document, ImportOptions::default())? {
    let unit = root.units.as_ref().map_err(Clone::clone)?.length;
    let imported = import_tessellated(
        &document,
        root.entity,
        unit,
        ImportOptions::default(),
        tessstep_mesh::Limits::default(),
    )?;
    let mesh = imported.mesh(); // metres; face_ids are STEP face IDs
}
for root in discover_presentations(&document, ImportOptions::default())? {
    let unit = root.units.as_ref().map_err(Clone::clone)?.length;
    let graphics = import_presentation(
        &document,
        root.entity,
        unit,
        ImportOptions::default(),
        PresentationLimits::default(),
    )?;
    for i in 0..graphics.polyline_count() {
        let _points = graphics.polyline(i); // indices into graphics.positions()
    }
}
```

## Evidence

Authored fixtures cover:

- a box that uses every face encoding (explicit triangles with and without
  `pnindex`, strips, fans, per-point, single and absent normals, a nonunit normal, a
  stitching triangle and a face link);
- an open shell with face and shell links to B-rep geometry, and an open solid;
- an open shell with a boundary edge, a corner vertex and a vertex in its own list;
- a box whose six faces each have their own coordinate list and close only through
  12 connecting edges;
- a shell with a cubic Bézier face outside the profile;
- a representation-selection file (a linked pair, a relationship pair and an
  unpaired surface set);
- an annotation in two draughting models, one characterized: a callout styling a
  repositioned geometric set with curve, fill, point and nested repositioned sets.

Rust tests add typed, located rejections for count, index, normal, orientation,
degeneracy, connecting-edge, placement, child-type and budget violations, strict-mode
behaviour, record-order invariance and 300 deterministic byte mutations per profile.
C ABI contract tests and the installed C11/C++17 consumers cover layouts, ownership,
output clearing, retained zero-copy views and failure statuses. A `presentation_import`
fuzz target joins `tessellated_import`.

`python3 scripts/check_geometry.py --require-external` also checks unmodified exporter
files, hash-pinned in
[external-tessellated.json](../corpus/geometry/external-tessellated.json) and
[external-presentation.json](../corpus/geometry/external-presentation.json):

| Input | Result |
| --- | --- |
| NIST FTC-08 tessellated solid (`#11436`) | 272 faces, all linked to B-rep surfaces; 20 stitching triangles skipped; 1,636 vertices, 3,368 triangles; closed; 0.000503 m³ |
| CATIA V5 cuboid (`#32`) | 6 strip faces; 8 vertices, 12 triangles; closed; 0.000772403 m³, matching its 101.43797 × 76.14538 × 100 mm extents |
| HOOPS Exchange shell (`#92`) | 1 face; 81 vertices, 128 triangles; open with 32 boundary edges |
| NIST CTC-02 annotation `Linear Size.1` (`#1023`) | 4 repositioned sets; 5 polylines (44 points), 211 fill triangles, 1 stitching triangle skipped, 2 zero-area triangles; strict accepts |
| HOOPS Exchange / SOLIDWORKS MBD annotation (`#26`) | 4 polylines (128 points), 595 fill triangles; 6 tolerated `pnmax` deviations; strict rejects |

The corpus runner surveys every shape tessellation root and annotation occurrence; see
[corpus testing](corpus-testing.md#solid-import-stage) and
[VALIDATION.md](VALIDATION.md) for the observed outcomes.
`cargo bench -p tessstep-import --bench tessellated` measures a generated closed grid
box and a generated repositioned annotation; see [benchmarks](../benchmarks/README.md).
