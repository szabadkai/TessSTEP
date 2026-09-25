# Assembly mesh assets

Milestone 18 provides STEP-independent assembly scenes in `tessstep_mesh::scene`
(`tessstep::mesh::scene`). A scene owns a forest of explicit occurrences and shares
validated immutable `Arc<Mesh>` assets. Repeated parts retain the same buffers;
scene construction neither tessellates nor copies their vertices. An occurrence has
its own nonzero identity, optional parent, optional asset and local affine transform.
Nodes without an asset represent assembly groups. Different occurrences may use the
same asset; coincident geometry and face IDs are never used to infer shared identity.

`Scene::new` accepts parents in any input order and preserves asset/instance order.
Sparse ordered indexes resolve IDs independently of their numeric magnitude. Missing
links, duplicate/zero IDs, cycles anywhere in the forest, unusable transforms and
resource exhaustion return typed errors without a partial scene. Empty scenes and
unused assets are allowed. Every occurrence has at most one parent: callers explicitly
instantiate repeated subassemblies; this API does not expand a definition DAG.

## Placement and ownership

Positions and translations use metres. Local maps act on column vectors:
`p_parent = linear * p_local + translation`. World placement composes parent placement
after local placement. No STEP axes, defaults, units, representation relationships or
product graph alternatives are interpreted. The Milestone 5 product adapter now supplies checked matrices and bounded definition-DAG
expansion. Binding its representation identities to mesh assets remains caller-controlled.

`instances()` returns immutable source records, world transforms, depth (roots are
one), and a reflection flag. Renderers using the shared mesh must reverse winding for
mirrored instances and transform normals with the inverse transpose. Nonuniform scale,
shear and reflections are accepted when both local and world transforms pass scaled
pivot checks. The inverse transpose is cached once per occurrence. A scaled elimination
computes handedness without relying on a potentially overflowing determinant.

`bake(instance_id, mesh_limits)` explicitly copies only that instance's asset into
world coordinates and revalidates the mesh. Reflections swap triangle corners 1 and 2
and the corresponding UV/normal corners; normals use the normalized inverse transpose.
Asset-local face IDs are preserved: pair them with the occurrence ID for scene provenance.
The returned mesh has independent ownership. Group nodes fail with `NoAsset`; baking
does not merge descendants, weld coincident parts or claim union/cavity semantics.

A scene validates its placement graph, not all transformed vertex buffers. Baking may
fail when large translations lose distinguishable vertices, arithmetic overflows, or
transformed smooth normals no longer agree with transformed facets. Numerical pivot
thresholds are safeguards, not exact predicates or condition-number certificates.
Scaling an asset also scales approximation error; no scene-world tessellation tolerance
is promised and assets are not automatically retessellated.

## Bounds and public interfaces

Default construction limits are 100,000 assets, 1,000,000 occurrences, depth 1,024 and
10,000,000 logical work units, with the math layer's default numerical tolerance.
Validation uses iterative breadth-first traversal, including disconnected components;
there is no recursive destruction chain. Graph storage scales with assets/occurrences,
not repeated vertex counts. Ordered map operations have logarithmic comparison cost;
logical budgets do not cap exact RSS or wall time. Baking charges positions and corner
transforms before allocating, then passes the remaining work to mesh validation.

The C ABI retains each original mesh handle; independently acquired assets and their
views can outlive the scene. C++17 `Scene` provides RAII, typed results and `asset_mesh`
returning an owning `Mesh`, whose zero-copy `MeshView` retains its own acquisition.
Baking returns a new owned mesh. All C records use explicit scalar layouts; Rust
`Arc`, containers, math types and errors remain private. See [C_API.md](C_API.md).

Independent tests cover repeated buffer identity, unordered parents, noncommuting
nested placement, reflections, shear/nonuniform scales, corner provenance, positive
solid volume, 10,000-deep forests, disconnected cycles, numerical/resource failures and
2,000 bounded graph mutations. The `scenes` benchmark exercises shared assembly creation
and explicit baking separately. Installed C/C++ consumers cover layouts, retention,
concurrent reads, moves, failure cleanup and relocation.

Milestone 19 adds [explicit appearance](APPEARANCE.md) as a separate retained layer.
automatic STEP-to-mesh asset binding,
scene flattening/Boolean unions, world-space error-driven remeshing, external references
and industrial numerical certification remain future work. No external STEP geometry/tessellation stage is enabled by these APIs.
