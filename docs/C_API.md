# Public C ABI and C++ wrapper

The C ABI and C++ wrapper are supported public interfaces. They carry the same
compatibility, documentation and release-testing obligations as the public Rust
API. This is a binding architectural requirement. The first implementation exports
ABI 1 physical-document operations, a C++17 wrapper and a shared CMake package.
Owned triangle-mesh import/read-only views, explicit assembly asset scenes and appearance layers are also implemented. Schema decoding,
general B-rep construction and curved STEP tessellation are not exposed yet.
The additive faceted and edge-based planar import entry points are described in [STEP_IMPORT.md](STEP_IMPORT.md).
The planar profile's conformance policy is a separate 16-byte `ts_import_policy`
record with a flags word (`TS_IMPORT_STRICT`), passed to
`ts_document_tessellate_planar_with_policy`. It leaves the frozen options layout
unchanged. Unknown flag bits are rejected, so future opt-in policies can be added
to the same record.

## Using the first slice

The authoritative protocol is [tessstep.h](../include/tessstep/tessstep.h); the
header-only C++ wrapper is [tessstep.hpp](../include/tessstep/tessstep.hpp).
Build/install from source (Rust 1.85+, CMake 3.20+, C11/C++17 toolchain):

```sh
cmake -S . -B target/cmake -DCMAKE_INSTALL_PREFIX=/absolute/install/path
cmake --build target/cmake --config Release
cmake --install target/cmake --config Release
```

Consumers set `CMAKE_PREFIX_PATH` to that installation, then use:

```cmake
find_package(TessSTEP 0.1 CONFIG REQUIRED)
target_link_libraries(my_app PRIVATE TessSTEP::TessSTEP)
```

Prebuilt consumers need only CMake and their C/C++ toolchain, not Cargo. This slice
supports shared linkage on 64-bit Linux (GCC/Clang), Windows (MSVC 2022), and macOS
(Apple Clang). Debug and Release consumers link the same release ABI library; no
CRT allocation ownership crosses the boundary. Static linkage is not provided.
Windows applications must make the installed `bin` directory available to the DLL
loader (for example through PATH, or by placing the DLL beside the executable).
Package relocation and compiler/platform behavior are CI gates; local verification
is macOS arm64 only. See VALIDATION.md for observed results.

```cpp
#include <tessstep/tessstep.hpp>
// bytes contains a complete physical STEP file, read by the application.
auto parsed = tessstep::Document::parse(bytes);
if (!parsed) {
    // parsed.error().code and .diagnostics own their structured error details.
    return 1;
}
auto document = std::move(parsed).value();
auto info = document.info();
auto report = document.diagnostics(); // Separate reference/trust analysis.
```

`ts_document_parse` borrows bytes only during the call, returns a complete immutable
document or a typed failure, and accepts explicit budgets initialized by
`ts_parse_options_init`. `ts_document_get_info`, `ts_document_entity_at`, and
`ts_document_record_name` inspect counts, source-order IDs and component names.
C names are borrowed without copying; C++ `record_name` explicitly returns owned
text. Reference analysis returns an independent report that survives document
release. A successful analysis can contain error diagnostics. Parse success never
means schema or CAD validity.

Each document acquisition/retain requires `ts_document_release`; every diagnostic
report requires `ts_diagnostics_release`. C++ wraps both paths in RAII, including
when C++ string/vector allocation throws. C++ document copies are disabled; moves
and destructors are nonthrowing. Queries on moved-from documents return a typed
invalid-argument result. Diagnostics and names returned by C++ are owned copies.
There are no callbacks, global last-error slots or file access. Mesh operations below
are independent of physical document parsing.

ABI 1 freezes the header's fixed-width status values and record layouts. Options
must have the exact size and ABI version initialized by the library; zero budgets
mean zero. Future incompatible records receive new names, never appended fields.
NULL, empty input, failure outputs, borrowed-text lifetimes and concurrent reads are
specified in the header. C opaque handles never expose Rust storage layouts.
Unwinding panics become internal failures; an unexpected panic payload is deliberately
forgotten to prevent a panicking destructor from escaping. Aborts and allocation
termination remain unrecoverable. Release paths have no expected panic sources.

## ABI boundary

No Rust type, layout, allocator, panic, or ownership semantics may cross the ABI
boundary. Define the protocol in a public C header: opaque handles, explicitly
specified C-compatible scalar fields, pointer/count buffers, stable status codes,
and an ABI version query such as `ts_api_version`. Any exposed record has a
C-defined layout and versioning contract; it is not an exported kernel struct.
Rust bridge types implement that contract privately.

Do not export Rust references, slices, enums, `Vec`, `String`, `Box`, trait objects,
or implementation-dependent layouts. The C++ wrapper calls only the public C ABI;
it does not inspect opaque handles or depend on Rust symbols or object layouts.
Isolate necessary unsafe bridge code in `tessstep-capi`, with SAFETY invariants
and focused tests. Kernel crates continue to forbid unsafe code.

## Allocation, ownership and lifetime

Every function documents whether each handle or buffer is borrowed, retained,
transferred, or newly created in terms of the C protocol. Acquired handles have
matching library release operations. Library-owned memory is released only by
the library; callers never use `free`, `delete`, or their allocator on it. Caller
buffers remain caller-owned. Copying a C handle value does not acquire ownership.

Input pointers have explicit validity, count and duration requirements. Output
parameters have defined failure states; a failed operation must not leave a
partially acquired object or require callers to understand Rust drop behavior.
Text has documented encoding and length. Define null/empty cases, thread-safety,
handle retention, and destruction order in the header before implementation.

C++ owning types release handles automatically, support safe moves, and have
nonthrowing destructors. Copies are disabled unless a documented retain or clone
operation implements their ownership. Failed construction and result propagation
must be exception-safe and leak-free. No manual Rust lifetime management is
required of a C or C++ consumer.

## Implemented mesh ownership and read-only views

`ts_mesh_create` imports flat XYZ double and u32 triangle-index buffers, copying the
inputs during the call. It validates mesh invariants through `tessstep-mesh`, creates
faceted unit normals, zero UVs and face id 0, then publishes an immutable opaque handle.
This imports existing triangles; it does not tessellate a STEP document. Initialize
`ts_mesh_options` with `ts_mesh_options_init`; set `require_solid=1` to require a single
closed component with positive algebraic volume. Empty/invalid geometry returns the
additive ABI-1 status `TS_INVALID_MESH` (7); budget failure returns `TS_RESOURCE_LIMIT`.
No detailed mesh diagnostics are exposed yet.

`ts_mesh_get_view` returns address-stable const scalar pointers and record counts,
without allocation or copying. Positions use 3 doubles per vertex; triangles use 3 u32
indices per triangle; normals/UVs use 9/6 doubles per triangle and retain corner order.
Face IDs use one u64 per triangle. All arrays have zero padding and their scalar type's
alignment. Boundary counts, components and algebraic volume are included. Volume on an
open mesh is not physical volume; closure does not prove geometric nonintersection.

C views borrow the handle and remain valid until its last release. Use `ts_mesh_retain`
for longer ownership and match every acquisition with `ts_mesh_release`. C++17 `Mesh`
and `MeshView` are move-only RAII wrappers. `Mesh::view()` retains a reference for the
view, which therefore survives destruction/movement of the originating mesh. Its `data()`
returns the C-defined pointer/count record with const element pointers. Extracted pointers
remain borrowed from the view and must not outlive it. Moved-from views contain null
pointers and zero counts. Concurrent immutable reads are supported while ownership lives.

```cpp
const double xyz[] = {0,0,0, 1,0,0, 0,1,0};
const uint32_t indices[] = {0,1,2};
auto imported = tessstep::Mesh::from_triangles(xyz, 3, indices, 1);
if (!imported) return 1;
auto view = imported.value().view();
if (!view) return 1;
const auto& buffers = view.value().data(); // No copy; view retains the mesh.
```

The bridge explicitly copies validated kernel data into C scalar storage once at
creation. Queries never reinterpret Rust point/vector/Vec layouts or repack buffers.
New options/views are independent frozen records (40/80 bytes on the supported 64-bit
platforms); all earlier ABI-1 layouts and statuses retain their meanings. The installed
`TessSTEP::TessSTEP` target exports the same library and header package, now with 34 C
symbols. Kernel tessellation remains Rust-only until public B-rep inputs are specified.

## Assembly asset scenes (Milestone 18)

`ts_scene_create` accepts mesh asset records and explicit occurrence records. Every
asset and occurrence ID is nonzero and scene-local. Parent zero denotes a root; asset
zero denotes a group without geometry. Parents can follow children in the input array.
Local maps use row-major 3x3 matrices, column vectors and metre-based translations.
C callers must supply identity explicitly; a zero matrix is singular. C++'s
`scene_instance` helper initializes identity. Matrices are supplied by the caller;
no STEP unit/default/placement interpretation takes place.

Scenes retain the original mesh acquisitions without copying their buffers. Callers
may release input meshes immediately after successful creation. `ts_scene_asset_mesh`
returns an independent acquisition of the same mesh, paired with `ts_mesh_release`.
A view acquired from it remains valid after the scene is released. C++ `Scene` and
`Mesh` provide move-only RAII; `MeshView` retains its own acquisition as before.
Scene queries copy scalar records in input order, including local/world matrices,
root-one depth and reflection parity. Concurrent immutable queries are supported while
a live acquisition remains. Renderers must honor reflected winding and inverse-transpose
normal transformation when drawing an asset through these placements.

`ts_scene_bake_instance` / `Scene::bake` explicitly produces one owned world-space mesh,
reversing triangle and corner-attribute order for reflections and transforming normals
with the inverse transpose. It revalidates the mesh and honors mesh limits/require_solid.
Group or missing instance targets return `TS_NOT_FOUND`; malformed graphs and numerical
placement failures return `TS_INVALID_SCENE` (8). No partial result is published.
Baking preserves asset-local face IDs; occurrence identity is separate. No subtree merge,
Boolean union, placement decoding or world-space tessellation tolerance is promised.

```cpp
auto part = tessstep::Mesh::from_triangles(xyz, 3, indices, 1);
if (!part) return 1;
auto first = tessstep::scene_instance(1, 0, 10);
auto second = tessstep::scene_instance(2, 0, 10);
second.translation[0] = 5;
auto scene = tessstep::Scene::create({{10, &part.value()}}, {first, second});
if (!scene) return 1;
auto baked = scene.value().bake(2); // Explicit copying, optional for rendering.
```

New frozen C records are 40-byte options, 16-byte asset/info, 120-byte instance and
232-byte instance-info records on supported 64-bit targets. The eight new functions
bring ABI 1 to 25 symbols; existing layouts/statuses are unchanged. The bridge retains
the kernel mesh alongside C scalar storage once per asset so shared scenes can reuse
validated data. Scene construction does not multiply that storage by occurrence count.
Default construction budgets and exact numerical/semantic limits are documented in
[ASSEMBLY_ASSETS.md](ASSEMBLY_ASSETS.md) and the public header.

## Appearance (Milestone 19)

`ts_appearance_create` / C++17 `Appearance::create` attaches an immutable color/opacity
layer to a retained scene. Material and binding records are copied; scene and geometry
buffers are shared. The caller can release original inputs immediately after success.
`ts_appearance_get_scene` / `Appearance::scene()` acquires the original scene independently;
its acquired meshes and zero-copy views can outlive both original scene and appearance.
Concurrent queries are allowed while a live acquisition remains. C++ ownership is move-only
RAII with typed errors, including `ErrorCode::invalid_appearance`.

`ts_material` contains a nonzero local ID and finite linear-light RGB/straight opacity
components in [0,1]. It does not describe a texture or lighting model. Target kinds
separate asset defaults, asset-local faces, instance overrides and instance-local faces.
Their `reserved` field must be zero; `face_id` must be zero for whole-asset/instance kinds.
For face kinds, zero is an ordinary identity; C triangle imports currently assign face
zero to every triangle. Unrecognized kind/unused-field values return `TS_INVALID_ARGUMENT`.
Missing or duplicate assignments and invalid colors return `TS_INVALID_APPEARANCE` (9).

Resolution chooses an exact instance-face assignment, then the nearest instance/ancestor
whole-instance override, then the asset face, then asset default. This explicit policy
does not claim ISO STEP style semantics. Face overrides never propagate to descendants;
whole-instance overrides do, including through group nodes. Entire materials are replaced;
opacity is not multiplied. Results preserve the winning target and ancestor provenance.

`ts_appearance_resolve_triangle` returns `TS_OK` with material ID zero and zero source
when the triangle is unstyled. An explicitly transparent color has alpha zero and a
nonzero material ID. Missing/group occurrences and out-of-range triangles return
`TS_NOT_FOUND`. All failed outputs are cleared. Scalar palette/binding enumeration follows
input order; queries allocate nothing. Geometry-only baking keeps triangle order and face
IDs, so these queries still match each corresponding baked triangle, even after reflection.
The baked `Mesh` does not itself retain a palette; exporters must pair the results explicitly.

```cpp
auto scene = tessstep::Scene::create({{10, &part.value()}},
    {tessstep::scene_instance(1, 0, 10), tessstep::scene_instance(2, 0, 10)});
if (!scene) return 1;
auto appearance = tessstep::Appearance::create(scene.value(),
    {{20, {0.2, 0.5, 0.8, 1.0}}, {30, {0.8, 0.1, 0.1, 0.5}}},
    {tessstep::style_binding(tessstep::StyleScope::asset, 10, 20),
     tessstep::style_binding(tessstep::StyleScope::instance, 2, 30)});
if (!appearance) return 1;
auto color = appearance.value().resolve_triangle(2, 0); // Material 30; instance 2.
```

Nine new functions bring the library to 34 C exports. Existing ABI-1 records, symbols
and statuses retain their contract. New frozen 64-bit layouts are 32-byte options,
40-byte material, 24-byte target, 32-byte binding, 64-byte resolved material and 16-byte
info records. No Rust color, enum, Arc, vector, allocator or error layout crosses the
boundary. Options bound material/binding counts and logical work, including face scans
and inheritance traversal. See [APPEARANCE.md](APPEARANCE.md) for limits and unsupported
STEP style adaptation, surface-side styling, textures, lighting and rendering.

## Errors and panic containment

C operations return stable typed status codes and explicit diagnostic access.
Diagnostics have documented ownership and lifetime; they never expose a Rust
error object or borrowed formatter state. Distinguish expected invalid input,
unsupported functionality, resource limits, and internal failures.

C++ maps these into typed error codes and result objects with structured diagnostic
information. If an exception convenience API is added, its typed exceptions are
constructed entirely in C++; no exception or panic may unwind through the C ABI.

Convert expected failures into results before the boundary. Where Rust unwinding
is enabled, contain unexpected panics inside each exported operation and translate
them into an internal-error status without publishing partial output or reusing
invalid state. Release paths must remain nonpanicking. Process aborts and allocator
termination are not recoverable errors; the implementation must not claim that
panic containment can recover them. Callback designs, if introduced, must also
prevent C++ exceptions from entering Rust.

## CMake package

Install a relocatable CMake package with a version file, public headers, libraries,
and the imported target `TessSTEP::TessSTEP`. The intended consumer interface is:

```cmake
find_package(TessSTEP CONFIG REQUIRED)
target_link_libraries(my_app PRIVATE TessSTEP::TessSTEP)
```

The target supplies include paths, its required C++ language level, the C ABI
library, and platform link dependencies transitively. Consumers of a prebuilt
package do not need Cargo or knowledge of Rust linkage. Publish the supported
C/C++ compiler baseline and shared/static build configurations. Verify exported
symbols, visibility, runtime dependencies, and Debug/Release configuration mapping
on Linux, Windows and macOS.

## Compatibility and acceptance evidence

Version the C ABI from its first release. Prefer additive evolution; define size
and version handling before exposing extensible option records. The C++ API and
CMake target are public compatibility surfaces, not internal implementation aids.

Before marking either interface available, compile and run standalone C and C++
consumers against the installed package, without source-tree paths or Rust tooling.
Test status/diagnostic mapping, null/empty cases, acquire/release symmetry, moves,
construction failures, destruction order, panic containment, and concurrent reads
where supported. Mesh tests must verify pointer stability, view lifetime after
parent destruction, and the absence of copies on documented zero-copy paths.
Exercise relocation and `find_package` through `TessSTEP::TessSTEP` on all supported
platforms, with sanitizer and ABI/layout checks where applicable.

The document, mesh, scene and appearance subsets are exercised by `cargo test -p tessstep-capi` and
`python3 scripts/check_capi.py`. The latter installs and relocates the package,
compiles/runs independent C11 and C++17 consumers in Debug/Release (plus a C-only
CMake project) with Rust tool
invocations blocked, and verifies the exact exported symbol set. Release packaging
runs those same consumers. Native consumer ASan/UBSan is available with
`--sanitizers`; this does not instrument the Rust library. Mesh lifetime, pointer stability, ownership, failure and zero-copy
acceptance checks run in those consumers. See [ARCHITECTURE.md](ARCHITECTURE.md) and
[CONFORMANCE.md](CONFORMANCE.md).

Milestone 5 product graphs and unit/assembly adapters are Rust-only operations. The
C ABI and C++ wrapper expose physical documents, independent meshes and explicitly placed mesh scenes; no product layouts,
placement descriptions or Rust ownership conventions are exposed through ABI 1.
