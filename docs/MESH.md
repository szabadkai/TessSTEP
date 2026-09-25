# Owned mesh assets

`tessstep-mesh` / `tessstep::mesh` owns immutable, STEP-independent triangle assets.
It depends only on the checked math layer. `Mesh::new(MeshData, Limits)` takes ownership
of caller-created arrays, validates them and publishes a mesh only on success.
`Mesh::from_triangles` generates faceted normals, zero UVs and zero face IDs for imported
indexed triangles. Positions use model-space metres and triangles use u32 indices.

`MeshData` separates positions/triangles from per-corner unit normals and UVs, plus one
model-local u64 face identity per triangle. Position welding never erases attribute
seams. Validated meshes expose immutable slices through `data()`; they own their buffers
and can outlive the input B-rep and shared-edge cache.

Validation checks finite positions/attributes, array lengths, index bounds, nonzero
triangle area, duplicate triangles, unit normals facing each facet, unused vertices,
edge manifoldness, opposing edge directions and connected vertex links. Open boundary
links are paths; closed links are cycles. Components are counted through shared edges.
Every invalid state has a typed `Error`; work and output limits fail without a mesh.
Ordered maps and iterative graph walks are deterministic and logically bounded, without
an exact memory/time bound. Mesh construction has no filesystem, schema or topology access.

`statistics()` reports boundary edges, connected components and algebraic signed volume.
Volume uses a translation-relative compensated sum; extreme finite coordinates can still
produce an explicit numeric failure. Open-mesh volume is not a physical volume.
`is_watertight()` means zero boundary edges after the manifold checks.
`require_solid()` also requires one component and positive signed volume. It does not
prove nonintersection, material containment or cavity nesting. No tolerance-based weld,
orientation repair or self-intersection algorithm is hidden in these checks.

The C ABI imports flat scalar triangle buffers into an owned opaque mesh and returns
zero-copy borrowed read-only views. C++ `MeshView` retains its own reference so it can
outlive the originating `Mesh`. Bridge storage is explicitly C-defined and separate
from kernel representations; Rust Vec/reference/enum layouts never cross the boundary.
See [C_API.md](C_API.md) for protocol, fixed layouts and native release tests.

`scene` adds shared assets, explicit nested occurrence transforms and checked baking.
See [ASSEMBLY_ASSETS.md](ASSEMBLY_ASSETS.md) for ownership, reflection and numerical limits.

`appearance` retains a scene and adds color/opacity with exact asset/face and inherited
occurrence assignments. Geometry buffers stay shared; see [APPEARANCE.md](APPEARANCE.md).
