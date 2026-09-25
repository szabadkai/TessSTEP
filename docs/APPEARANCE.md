# Mesh appearance

Milestone 19 provides explicit, STEP-independent color and opacity in
`tessstep_mesh::appearance` (`tessstep::mesh::appearance`). `Appearance::new` retains
an `Arc<Scene>`, owns an immutable material palette and assignment records, and validates
all references before publishing the result. It copies no mesh buffers and creates no
per-occurrence triangle material arrays. Different appearance layers may share the same
scene; differently colored instances continue to share the original geometry.

`MaterialId` is a nonzero appearance-local identity. `LinearRgba::new([r,g,b,a])` accepts
only finite components in [0,1]. RGB is linear-light and alpha is straight (unassociated)
opacity: zero is transparent and one opaque. Values are not clamped, premultiplied or
converted from sRGB. A material is a color/opacity record, not a lighting model. There
are no implicit materials, names used as identity, or automatic color deduplication.

## Assignments and precedence

A `Binding` connects a material to an exact `Target`: an asset, an asset-local face,
an occurrence, or a face of one occurrence's own asset. Face IDs are the mesh's existing
`face_ids`, not triangle indices; every triangle with that ID receives the assignment.
Face ID zero is ordinary, not a wildcard. A default assignment uses a distinct target
variant. Meshes imported through the current C triangle API use face ID zero throughout.

`resolve_triangle(instance_id, triangle_index)` chooses the first applicable assignment:

1. The exact occurrence-face assignment.
2. The nearest occurrence or ancestor with a whole-occurrence override.
3. The asset-face assignment.
4. The asset default.

This is TessSTEP's explicit appearance policy, **not a claim about ISO STEP style
precedence**. An occurrence override replaces the entire material and applies to its
descendants, including through group nodes without geometry. A nearer occurrence override
replaces an ancestor's. Face overrides apply only to their named occurrence, never to
its children. Asset defaults are not inherited through parent occurrences. No alpha
multiplication, color blending or partial-property merging occurs. An omitted assignment
falls through; it does not clear an inherited override.

The result is `Option<Resolved>` containing the winning material ID and original target,
including ancestor identity. `None` means unstyled. An explicitly transparent material
still has a nonzero ID and is distinct from absence. Triangle queries allocate nothing,
scan no mesh, and walk no ancestor chain. They use ordered lookups and cached inheritance.
The material can be read from the retained palette with `material(id)`.

Geometry-only `Scene::bake` does not attach appearance records to the returned mesh.
It preserves triangle order and asset-local face IDs, so the original appearance query
for each triangle remains valid for that instance's baked output, including reflections.
Applications exporting a baked mesh should explicitly pair these results; material and
occurrence identity must not be inferred from world coordinates or mesh face IDs alone.

## Validation and bounds

Duplicate material IDs and duplicate assignment targets are rejected, including identical
duplicates. Missing materials, assets, occurrences and face IDs fail explicitly. A face
assignment on a group without its own asset fails; group-wide overrides are allowed.
All assignments are validated even when another assignment would hide them. Unused
palette entries, empty palettes/assignments, and styles on existing unused assets are
permitted. Source order is preserved by `materials()` and `bindings()`; assignment order
does not affect the result. Invalid query targets and unstyled triangles are distinct.

Default construction limits are 100,000 materials, 1,000,000 bindings and 10,000,000
logical work units. Work includes palette/binding processing, scene traversal and scans
of assets needed to verify face assignments. Each such asset's face set is built once
per construction, shared across its occurrences, and discarded after validation.
Inheritance is resolved by one iterative forest traversal, so deeply nested assemblies
have no recursive construction, query or destruction path. Existing scene limits apply
when constructing the underlying scene. Ordered maps add logarithmic comparison cost;
logical work limits are not exact RSS or wall-time bounds. Failure publishes no partial
appearance and never modifies the scene.

## C and C++

`ts_appearance_create` retains the original scene independently and copies only explicit
C material/binding records. Scalar material/assignment queries expose no Rust layouts.
`ts_appearance_get_scene` returns an independent acquisition of the original scene;
its acquired mesh buffers and zero-copy views may outlive the appearance. C++17
`Appearance` owns its handle through RAII, returns typed errors and provides a `scene()`
acquisition. Construction failure cleans up all temporary ownership.

The C query returns `TS_OK` with material ID zero and zero source for an unstyled
triangle. Missing/group instances or invalid triangle indices return `TS_NOT_FOUND`.
Invalid colors or assignment semantics return `TS_INVALID_APPEARANCE`; malformed
C records/options return `TS_INVALID_ARGUMENT`. Counts/work limits return
`TS_RESOURCE_LIMIT`. No previous ABI layout or status is changed. See
[C_API.md](C_API.md) and the public header for the complete contract.

Regression tests cover precedence, inherited provenance, transparent versus unstyled
results, face zero/repeated face IDs, shared geometry, reflected baking, hidden invalid
assignments, count/work failures and 10,000-deep inheritance. A 2,000-case deterministic
mutation harness compares cached resolution against an independent parent-walking oracle
and checks arbitrary-bit colors. Native consumers cover copied inputs, ownership chains,
concurrent reads, moves, scalar layouts, error clearing and installed-package relocation.
The `appearance` benchmark separates inheritance construction from triangle queries.

STEP presentation/style adaptation, surface-side styles, texture images, vertex colors,
line/point styles, physically based material properties, shader evaluation, color-space
conversion, rendering and exporters remain future work. This module does not enable an
external STEP appearance, geometry or tessellation corpus stage. The next numbered
milestone is 20: existing tessellations.
