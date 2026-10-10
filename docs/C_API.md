# Public C ABI and C++ wrapper

The C ABI and C++ wrapper are supported public interfaces. They carry the same
compatibility, documentation and release-testing obligations as the public Rust
API. This is a binding architectural requirement. The first implementation exports
ABI 1 physical-document operations, a C++17 wrapper and a shared CMake package.
Owned triangle-mesh import/read-only views, explicit assembly asset scenes and appearance layers are also implemented, as are
existing-tessellation and presentation-graphics import from STEP documents (Milestone 20),
schema selection, assembly import and imported appearance (Milestone 21), and
extensible import options with the tolerance policy and opt-in repair, root and unit
discovery, representation selection, single-root curved solid and surface-model
import, import sessions, failure text and a static library target (Milestone 22.10).
The export set of ABI 1.0 is frozen at 95 functions ([ABI 1 freeze](#abi-1-freeze));
ABI 1.1 adds three: input-sized parse budgets, the parse conformance policy and
deviation counts ([ABI 1.1](#abi-11-parse-budgets-and-exporter-slips)).
General schema decoding is not exposed.
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
find_package(TessSTEP 1.0 CONFIG REQUIRED)
target_link_libraries(my_app PRIVATE TessSTEP::TessSTEP)
```

Prebuilt consumers need only CMake and their C/C++ toolchain, not Cargo. The package
supports shared linkage on 64-bit Linux (GCC/Clang), Windows (MSVC 2022), and macOS
(Apple Clang). Debug and Release consumers link the same release ABI library; no
CRT allocation ownership crosses the boundary. `TessSTEP::TessSTEP_static` links the
same ABI statically (see [static linkage](#static-linkage)).
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
`ts_parse_options_init`. Budget failures name the budget and its configured value in
the diagnostic message. Import option initializers keep their documented defaults;
for large documents, `ts_import_options_for_document` sizes `max_work` and
`max_records` as the Rust `ImportOptions::for_document` rule does (at least 512 work
units and one record per entity); for the per-profile option records, apply that
rule from `ts_document_info.entity_count`, see [STEP_IMPORT.md](STEP_IMPORT.md). `ts_document_get_info`, `ts_document_entity_at`, and
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
There are no callbacks, global or per-thread last-error slots or file access: failure
text is state of an explicit handle ([diagnostic text](#diagnostic-text)). Mesh
operations below are independent of physical document parsing.

ABI 1 freezes the header's fixed-width status values and record layouts. Options
must have the exact size and ABI version initialized by the library; zero budgets
mean zero. Future incompatible records receive new names, never appended fields.
The three extensible records of Milestone 22 (`ts_import_options`, `ts_solid_info`,
`ts_surface_model_info`) are the only exception: they may grow by published revisions
in ABI 1.x (see below).
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

## Existing tessellations and presentation graphics (Milestone 20)

`ts_document_import_tessellated` / C++ `Document::import_tessellated` converts a
selected `TESSELLATED_SOLID`, `TESSELLATED_SHELL` or triangulated surface set into an
owned `ts_mesh` without retessellation. Rules are those of
[EXISTING_TESSELLATIONS.md](EXISTING_TESSELLATIONS.md): coordinate-index identity,
connecting edges joining only the points they declare, normals that must agree with
winding, and closure for solids. The mesh acquisition is independent of the document;
its `face_ids` are the physical STEP IDs of the tessellated faces. The optional
80-byte `ts_tessellated_info` reports kind, root and link IDs and face, linked-face,
edge, vertex, skipped, joined and tolerated-deviation counts. It is cleared, like the
optional `ts_import_error`, before validation and on every failure. Edge polylines,
vertex items and per-face links stay Rust-only.

`ts_document_import_presentation` / `Document::import_presentation` returns a
retained `ts_presentation` handle for a tessellated annotation occurrence or geometric
set: placed polylines, points and fill triangles in metres. `ts_presentation_get_view`
returns a zero-copy 104-byte view of immutable scalar arrays (positions, polyline
point indices with `polyline_count + 1` offsets, triangles, points and the source-set
ID of every primitive). An array pointer is NULL exactly when its count is zero, except
the offsets, which always hold at least one zero. Views borrow the handle until its
last release; C++ `PresentationView` retains its own acquisition, so it survives the
originating `Presentation`. `ts_presentation_get_info`, `ts_presentation_item_at` and
`ts_presentation_style_at` return scalar copies, and out-of-range indices are
`TS_NOT_FOUND`. Concurrent immutable queries are allowed while an acquisition lives.
Graphics carry no mesh-validity claim, and styles are entity IDs only.

Both entry points take new frozen options records: 48-byte `ts_tessellated_options`
and 64-byte `ts_presentation_options`, the latter adding polyline-point and nesting
budgets. Each carries its own `flags` word; `TS_IMPORT_STRICT` rejects the tolerated
`pnmax` deviation, and unknown flags or nonzero reserved fields return
`TS_INVALID_ARGUMENT` after clearing outputs. Status mapping and `ts_import_error`
records follow the faceted/planar contract, without a tessellation stage.

```cpp
auto parsed = tessstep::Document::parse(bytes);
if (!parsed) return 1;
auto solid = parsed.value().import_tessellated(1000, 0.001);
if (!solid) return 1; // solid.error().import_failure locates the failing entity
auto mesh_view = solid.value().mesh.view();
auto graphics = parsed.value().import_presentation(300, 0.001);
if (!graphics) return 1;
auto view = graphics.value().view(); // Zero-copy; retains the graphics.
for (size_t i = 0; i < view.value().data().polyline_count; ++i) { /* draw a polyline */ }
```

Ten new functions bring the library to 50 C exports; the other new frozen layouts are
the 80-byte tessellated info, 64-byte presentation info, 24-byte item and 104-byte
view records on supported 64-bit targets. Existing ABI-1 records, symbols and statuses
keep their contract. Discovery and representation selection remained Rust-only until
Milestone 22.10.

## Schema selection, assembly import and imported appearance (Milestone 21)

`ts_document_get_schema` reports the first FILE_SCHEMA entry, borrowed from the
document, the number of entries, and the protocol the name selects from an exact
table: `CONFIG_CONTROL_DESIGN` (`TS_PROTOCOL_AP203`), the AP203 edition 2 MIM long
form (`TS_PROTOCOL_AP203E2`), `AUTOMOTIVE_DESIGN` (`TS_PROTOCOL_AP214`) and the AP242
MIM long form (`TS_PROTOCOL_AP242`). Every other name, including the
`AUTOMOTIVE_DESIGN_CC*` conformance-class schemas, is `TS_PROTOCOL_UNKNOWN`; a header
without a schema is `TS_NOT_FOUND`. Selection is not validation. No ISO schema is
bundled, whole-file AP validation is a corpus-runner stage built from separately
fetched schemas ([corpus-testing.md](corpus-testing.md#ap-schema-stage-milestone-21)),
and the library reports its own bounded-profile outcomes instead.

`ts_document_import_assembly` runs `tessstep_import::import_assembly` and
`import_appearance`: it imports every selected shape root in its context units,
links the meshes to products, definitions and next assembly usage occurrences placed
by context dependent shape representations, and adapts surface colours. Success
transfers one `ts_assembly` acquisition, independent of the document:

- `ts_assembly_get_info` counts nodes, products, occurrences, attempted and imported
  roots, unplaced and unimported roots and excluded records, and `complete` is 0 when
  an occurrence or placement record was excluded.
- `ts_assembly_node_at` maps each scene instance to its definition, occurrence and
  representation; an asset leaf carries its root, whose ID is also its asset ID.
- `ts_assembly_product_at` returns `PRODUCT.id` and `.name` views borrowed from the
  assembly; `ts_assembly_root_at` returns each root's kind, selection and status with
  its located `ts_import_error`; `ts_assembly_excluded_at` returns product and style
  records outside the bundled profiles with their status and located error;
  `ts_assembly_unplaced_at` lists imported roots no product shape holds.
- `ts_assembly_get_style_info` counts styled items by use.
- `ts_assembly_get_scene` and `ts_assembly_get_appearance` return independent scene
  and appearance acquisitions (the appearance shares the scene), so the existing
  scene, mesh-view and triangle-resolution queries apply unchanged.

A missing or ambiguous occurrence placement fails with `TS_INVALID_GEOMETRY` at
`TS_IMPORT_STAGE_PRODUCT` (new stage values 5, product, and 6, presentation); failed
roots are reported, not placed. The 72-byte `ts_assembly_options` holds work, record,
instance and depth budgets, the chord and angle tolerance, the minimum model tolerance
and `flags` (`TS_IMPORT_STRICT`); zero or non-finite tolerances, unknown flags and
nonzero reserved fields return `TS_INVALID_ARGUMENT` after clearing outputs. Curved
roots use the default tolerance policy (Milestone 22.3: deviations up to 16 times the
model tolerance are accepted and counted, see
[curved import](CURVED_IMPORT.md#tolerance-policy)); `TS_IMPORT_STRICT` keeps
rejecting every deviation. The multiple is not yet a C option.

```cpp
auto parsed = tessstep::Document::parse(bytes);
if (!parsed) return 1;
auto schema = parsed.value().schema(); // schema.value().protocol, owned declared name
auto assembly = parsed.value().import_assembly();
if (!assembly) return 1; // import_failure.stage == ImportStage::product for placements
auto scene = assembly.value().scene();          // placed occurrences, shared meshes
auto appearance = assembly.value().appearance(); // imported colours over that scene
auto first = assembly.value().product_at(0);     // owned id and name strings
```

Fourteen new functions bring the library to 64 C exports; the new frozen layouts are
the 32-byte schema info, 72-byte assembly options and info, 40-byte node, 48-byte
product, 56-byte root, 48-byte exclusion and 96-byte style info records on supported
64-bit targets. Existing ABI-1 records, symbols and statuses keep their contract.
Curved B-rep import of a single root remained Rust-only until Milestone 22.10.

## Import options, discovery and solid import (Milestone 22.10)

**Extensible import options.** `ts_import_options` carries what the whole import path
shares:

- `flags`: `TS_IMPORT_STRICT`, the opt-in repairs `TS_IMPORT_REPAIR_SEW` (merge
  coincident boundary vertices and free edges) and `TS_IMPORT_REPAIR_ORIENT` (flip
  faces into a consistent orientation, reverse loops that run against their surface,
  reverse inside-out closed shells), and `TS_DISCOVER_SURFACE_MODELS`. Other bits and
  a nonzero `reserved` are rejected; only this record accepts the last three bits.
- the profile decoding, adapter and discovery budgets `max_work` / `max_records`;
- the B-rep model tolerance (`model_distance_m`, `model_angle_rad`) and the
  tessellation tolerance (`chord_m`, `normal_angle_rad`);
- the budgets a solid's faces share (`max_tessellation_work`, `max_evaluations`) and
  the mesh limits (`max_vertices`, `max_triangles`, also used for presentation
  graphics). Per-face polygon, trimming and sampling budgets keep the library
  defaults, capped by these fields, so lowering any field binds and zero means zero;
- `sewing_factor` (sewing tolerance as a multiple of the model distance, default 10)
  and `deviation_factor` (the 22.3 tolerance policy, default 16, at least 1: an edge
  farther than the model distance from its face surface, or a vertex from its edge
  curve, is accepted and counted up to that multiple and rejected beyond it with
  `TS_INVALID_GEOMETRY` at the geometry stage; `TS_IMPORT_STRICT` forces 1,
  [curved import](CURVED_IMPORT.md#tolerance-policy));
- `max_assembly_instances` / `max_assembly_depth` (product-structure expansion),
  the session budgets `max_session_decoded_entities`, `max_session_adapted_entities`
  and `max_session_triangles`, and the presentation limits `max_polyline_points` /
  `max_presentation_depth` of session presentation import.

`ts_import_options_init(&options, sizeof options)` writes the Rust defaults
(`ImportOptions`, `TessellationOptions`, the assembly tolerance and 1e-7 m minimum
model tolerance, `SessionBudgets`, `PresentationLimits`; the values are in the
header), and `ts_import_options_for_document` also sizes `max_work` (512 units per
entity), `max_records` (one per entity) and the session budgets as the Rust
`for_document` rules do. The unit test `default_options_are_the_rust_defaults` keeps
the initialized record equal to the Rust defaults, tessellation budgets included.
Every entry point validates every field.

**Extensible records.** `ts_import_options` (168 bytes), `ts_solid_info` (216 bytes)
and `ts_surface_model_info` (136 bytes) are the only records that may grow.
`struct_size` names the revision a caller was compiled with (its `sizeof`). ABI 1.0
publishes revision 1 of each (`TS_IMPORT_OPTIONS_SIZE_1`, `TS_SOLID_INFO_SIZE_1`,
`TS_SURFACE_MODEL_INFO_SIZE_1`); a 1.x release may publish later revisions that only
append fields. The library accepts each published size and reads or writes exactly
that many bytes: an input's later fields take their defaults, an output's are not
written, so a consumer built against an older header keeps working. Initializers take
the size rather than assume the library's, so they never write past an older
caller's record. Any other size returns `TS_INVALID_ARGUMENT` and writes nothing; a
newer consumer on an older library is rejected the same way, never silently ignored.
Outputs (`ts_solid_info`, `ts_surface_model_info`) follow the same rule: the caller
sets `struct_size` and the library writes only that revision. No release ever shipped
the draft sizes of the development passes (96 and 152 bytes for the options, 104 for
the solid info); they are not revisions and are rejected.

**Single-root solid import.** `ts_document_import_solid` / C++
`Document::import_solid` imports a selected `MANIFOLD_SOLID_BREP` or `BREP_WITH_VOIDS`
through `tessstep_import::import_brep_solid` (the curved profile of
[CURVED_IMPORT.md](CURVED_IMPORT.md)) or a `FACETED_BREP` through the faceted profile,
chosen from the root's physical record, and tessellates it to an owned `ts_mesh` whose
`face_ids` are the STEP face IDs. Units stay explicit arguments: `metres_per_unit` and
`radians_per_unit` (cone semi-angles and supplied pcurve parameters), both finite and
positive; tolerances, strictness, repair and the deviation policy come from the
options. The optional `ts_solid_info` reports the root's kind, face and shell counts
(outer shell plus cavities), the representation adaptations of `Adaptations`
(inferred outer bounds, inserted seams, split edges, re-charted spheres, collapsed
edges and faces, supplied and rejected pcurves, edges off their surfaces, vertices
off their curves, uncollapsed B-spline sides), the largest accepted deviation and the
working tolerance every later stage used (`ImportedSolid::largest_deviation`,
`brep().model_tolerance()`), and the `RepairReport` (merged vertices and edges and
their largest distance, disconnected shells, flipped faces, reversed loops,
reoriented and undecided shells, sewing tolerance). None moves a coordinate. Output
clearing, ownership, status mapping and the located `ts_import_error` follow
`ts_document_import_tessellated`. C++ `SolidImport` holds the `Mesh` and the info; an
overload takes a `DiscoveredRoot` and uses its discovered units, or returns the typed
reason they are missing.

**Discovery and representation selection.** `ts_document_discover` / C++
`Document::discover` runs `discover_solids`, `discover_tessellations`,
`select_representations` with the given preference (`TS_PREFER_EXACT` or
`TS_PREFER_TESSELLATED`), `discover_presentations` and, with
`TS_DISCOVER_SURFACE_MODELS`, `discover_surface_models`, each pass with its own
`max_work`, and transfers one retained, read-only `ts_discovery` acquisition,
independent of the document:

- `ts_discovery_get_info` counts shape roots (solids, then tessellations, then any
  surface models), solids, tessellations, annotation occurrences and choices, and
  repeats the preference.
- `ts_discovery_root_at` returns a root's ID, `TS_ROOT_*` kind, tessellation link, its
  containing representations as a zero-copy array borrowed from the discovery, and
  either its `ts_context_units` (context ID, metres and radians per unit, optional
  length uncertainty in metres) with `TS_OK`, or the per-root unit failure status
  with a located `ts_import_error`; units are never defaulted.
  `ts_discovery_root_report_at` adds the encodings tolerated in the root's context
  units and the text of a unit failure ([diagnostic text](#diagnostic-text)).
- `ts_discovery_presentation_at` returns an annotation occurrence with its containing
  callouts and annotation planes and representations (zero-copy) and units or error.
- `ts_discovery_choice_at` returns one shape's selected shape-root index, its
  alternative indices (zero-copy) and the pairing (`TS_PAIRING_NONE`, `_LINK`,
  `_RELATIONSHIP`); an alternative is meant for a failed selection.

Surface-model roots (`TS_ROOT_SHELL_BASED_SURFACE_MODEL`) follow every solid and
tessellation and take no part in selection, so solid and tessellation indices, counts
and choices do not depend on the flag; their count is `root_count - solid_count -
tessellation_count`. Arrays are NULL exactly when their count is zero and stay valid
until the last release of the discovery; concurrent immutable queries are allowed.
C++ `Discovery` is move-only RAII and returns owned copies (`DiscoveredRoot`,
`DiscoveredPresentation`, `RepresentationChoice`), with each root's units as a typed
`Result<ContextUnits>` whose error carries the unit failure's text in `message`.

```cpp
auto parsed = tessstep::Document::parse(bytes);
if (!parsed) return 1;
auto options = parsed.value().import_options(); // budgets sized for this document
auto discovery = parsed.value().discover(tessstep::Preference::exact, &options.value());
if (!discovery) return 1;
for (size_t i = 0; i < discovery.value().info().value().choice_count; ++i) {
    auto choice = discovery.value().choice_at(i).value();
    auto root = discovery.value().root_at(choice.selected).value();
    if (root.kind == tessstep::RootKind::manifold_solid_brep) {
        auto solid = parsed.value().import_solid(root, &options.value());
        if (!solid) { /* solid.error(): typed code and located import_failure; try
                         choice.alternatives */ }
    }
}
```

### Static linkage

The installed package also provides `TessSTEP::TessSTEP_static`, an imported static
library (`libtessstep_capi.a`; `tessstep_capi.lib` on Windows) of the same ABI. Its
interface defines `TESSSTEP_STATIC`, which removes `__declspec(dllimport)` from the
header on Windows, and declares the system libraries the embedded Rust standard
library needs, exactly as rustc reports them (`--print native-static-libs`) for the
package's target when the package is configured. The configure step prints the list
(`TessSTEP static link dependencies`); on macOS it is `c;m` (libSystem, which rustc
also names, is always linked by the Apple driver and is omitted to avoid a duplicate
library warning). MSVC `/defaultlib:` directives are already embedded in the objects.
`TessSTEP::TessSTEP` keeps its meaning (shared); a separate target rather than a
switch keeps existing consumers' linkage unchanged and lets one build tree use both.
`find_package(TessSTEP 1.0 CONFIG REQUIRED COMPONENTS static)` requires the archive.

```cmake
find_package(TessSTEP 1.0 CONFIG REQUIRED COMPONENTS static)
target_link_libraries(my_app PRIVATE TessSTEP::TessSTEP_static)
```

The archive embeds the Rust standard library, so link at most one Rust static library
into a final binary, or use the shared target. On MSVC it uses the release DLL CRT
(`/MD`): Release consumers match it; Debug (`/MDd`) consumers link both CRTs (linker
warning LNK4098) and, as with the shared library, no CRT object crosses the
boundary. The installed-consumer gate links and runs the C and C++ consumers
statically in Release, against a copy of the package without the shared library, and
on Linux and macOS checks that the archive defines exactly the ABI's C symbols.
Debug static consumers and sanitizers on static consumers are not tested.

## Surface models, sessions, assembly options and failure text (Milestone 22.10)

**Surface models (22.6).** `ts_document_import_surface_model` / C++
`Document::import_surface_model` imports a `SHELL_BASED_SURFACE_MODEL` with the units,
tolerances, strictness and deviation policy of solid import, meshes every shell, and
transfers a retained, read-only `ts_surface_model`: `ts_surface_model_get_info` fills
the extensible `ts_surface_model_info` (shell, closed-shell, face and boundary-edge
counts and the eleven adaptation counters), `ts_surface_model_shell_at` returns each
shell's entity, declared closure, zero-copy face IDs, open boundary edges and working
tolerance, and `ts_surface_model_shell_mesh` an independent mesh acquisition. Shells
are meshes, never solids: closed shells are watertight, open shells keep their
boundary, and no volume or orientation is checked. Surface models are never repaired:
the repair flags are validated with the record and not applied, as the Rust importer
ignores `ImportOptions::repair` for them.

**Import sessions (22.8).** `ImportSession<'a>` borrows its document and is `Send` but
not `Sync`. `ts_import_session_create` therefore returns a uniquely owned handle that
retains the document (the caller may release it at once) and holds the Rust session
behind a mutex, created with the options' import settings and session budgets; the
`'static` borrow it needs is private to the handle, which drops the session before the
document. Calls on one handle are serialized, so concurrent calls are safe but not
parallel; a contained panic poisons the handle, and every later call except release
returns `TS_INTERNAL_ERROR`. The session covers every session-capable import:
`ts_import_session_discover` (cached passes, plus surface models when flagged),
`ts_import_session_prepare` (groups the given roots by the profile their import uses
and decodes each group together), `ts_import_session_import_solid`,
`ts_import_session_import_tessellated` (mesh limits from `max_vertices` /
`max_triangles`), `ts_import_session_import_presentation` (with `max_polyline_points`
and `max_presentation_depth`) and `ts_import_session_import_assembly`.
`ts_import_session_import_surface_model` runs the free import on the session's document
and options for a uniform API and its failure text; the Rust session has no surface
models, so it shares no decode and is not counted in the session budgets.
`ts_import_session_get_statistics` returns the Rust `SessionStatistics`. Results are
owned and independent; no Rust session, decode or borrowed record crosses the
boundary. C++ `ImportSession` (from `Document::import_session`) is move-only RAII over
it.

**Assembly import with the extensible options.**
`ts_document_import_assembly_with_options` / C++ `Document::import_assembly(Preference,
const ImportOptions*)` runs the assembly import through a one-shot session with
explicit session budgets, strictness, repair and the deviation policy for every root,
the record's tolerances (`model_distance_m` is the floor under each root's context
uncertainty), budgets and expansion limits, and the representation preference.
`ts_assembly_get_adaptations` (any assembly, also from the frozen-options entry point)
reports the 22.7 mapped-item placements and the tolerated exporter encodings of the
product closure; `ts_assembly_root_report_at` adds each root's tolerated context
encodings and failure text. The frozen `ts_tolerated_encodings` record keeps its five
22.7 counts: the reversed item defined transformations accepted since 1.0 (HOOPS
Exchange and CATIA V4 layouts) are counted in Rust only
(`ToleratedEncodings::reversed_transform_items`), and `TS_IMPORT_STRICT` rejects them
like the others.

### Diagnostic text

The Rust importers word every failure: the budget and its configured value (22.1,
22.2), a measured distance and its bound (22.3), the entity outside a profile, a unit
problem. `ts_import_error` locates a failure (stage, entity, byte offsets) but is a
frozen 32-byte record without text. The contract has no global error state ("No
callbacks, global error state" in the header), so the suggested per-thread "last
import diagnostic" was not added: hidden thread-local state would break that promise,
tie results to the calling thread, and race for callers that hand work between
threads. Text is instead state of an explicit handle, with the handle's lifetime:

- **Sessions.** `ts_import_session_last_message` copies the failure text of the
  session's most recent discover, prepare or import call into a caller buffer (UTF-8,
  NUL-terminated, truncated to the capacity) and reports the full length; it is empty
  after a success or an argument error. Calls are serialized per session, so "most
  recent" is well defined. C++ `ImportSession` fills `Error::message` on every failed
  call and exposes `last_message()`.
- **Per-root reports.** `ts_assembly_root_report_at` and `ts_discovery_root_report_at`
  return a `ts_root_report`: the encodings tolerated in the root's context units and
  the root's failure text (import failure in an assembly, unit failure in a
  discovery), borrowed from the handle. C++ `AssemblyRoot` and `DiscoveredRoot` carry
  `tolerated`, and their failures carry the text (`AssemblyRoot::message`, the units'
  `Error::message`).

Document-level calls keep no text. A caller that needs it imports through a session,
which gives the same result as the document-level call with the same options.

```cpp
auto parsed = tessstep::Document::parse(bytes);
if (!parsed) return 1;
auto options = parsed.value().import_options().value();
options.flags |= TS_IMPORT_REPAIR_SEW | TS_IMPORT_REPAIR_ORIENT | TS_DISCOVER_SURFACE_MODELS;
auto session = parsed.value().import_session(&options);
if (!session) return 1;
auto found = session.value().discover();
if (!found) return 1;
for (size_t i = 0; i < found.value().info().value().root_count; ++i) {
    auto root = found.value().root_at(i).value();
    if (root.kind == tessstep::RootKind::shell_based_surface_model) {
        auto model = session.value().import_surface_model(root.root_id,
            root.units.value().metres_per_unit, root.units.value().radians_per_unit);
    } else if (root.kind == tessstep::RootKind::manifold_solid_brep) {
        auto solid = session.value().import_solid(root);
        if (!solid) { /* solid.error().message: for example the budget and its value */ }
    }
}
```

Milestone 22.10 adds 31 functions to the 64 of Milestone 21, for 95 C exports, and
these layouts on supported 64-bit targets: the extensible 168-byte import options,
216-byte solid info and 136-byte surface-model info (revision 1 each), and the fixed
48-byte discovery info, 40-byte context units, 112-byte discovered root, 120-byte
discovered presentation, 32-byte choice, 48-byte surface shell, 40-byte tolerated
encodings, 48-byte assembly adaptations, 56-byte root report and 72-byte session
statistics.

## ABI 1.1: parse budgets and exporter slips

ABI 1.1 adds three functions, recorded as `1.1` in [abi/exports.txt](../abi/exports.txt)
(98 exports), and no record:

- `ts_parse_options_for_input(input_bytes, &options)` fills a `ts_parse_options` sized
  from the input length as the Rust `ParseLimits::for_input` sizes it (see
  [PART21.md](PART21.md#budgets)); `ts_parse_options_init` keeps its defaults. C++:
  `tessstep::parse_options_for_input(size)`.
- `ts_document_parse_with_policy(data, size, options, policy, &document, &report)`
  takes the existing 16-byte `ts_import_policy`. A NULL policy or flags 0 selects the
  tolerant default of the Rust parser: string backslashes that start no directive
  are literal and `$` may stand for FILE_DESCRIPTION and FILE_NAME strings, each kind
  counted and reported as one warning (`TS1006`, `TS1011`) by
  `ts_document_diagnostics`. `TS_IMPORT_STRICT` rejects both. C++:
  `Document::parse(bytes, options, policy)`.
- `ts_document_deviation_count(document, kind, &count)` returns how many slips of
  `TS_DEVIATION_LITERAL_BACKSLASH` (1) or `TS_DEVIATION_NULL_HEADER_STRING` (2) the
  parse accepted; an unknown kind is `TS_NOT_FOUND`, so later kinds can be added
  without a record. C++: `Document::deviation_count(tessstep::Deviation)`.

`ts_document_parse` stays strict. Its ABI 1.0 contract rejects both slips, and an
ABI 1.0 caller could observe a document where it was promised a parse error, so under
the freeze policy only the new entry point and its flags word may accept them. The
Rust `tessstep_model::parse` is tolerant by default (`parse_with` takes
`Conformance::Strict`), so the C default differs from the Rust one until ABI 2. The
warnings are existing codes with the existing warning severity, produced only for
documents the new entry point accepted. The installed C and C++ consumers check the
sized budgets, both policies, the counts and the warnings on an authored input.

## ABI 1 freeze

The ABI 1.0 export set was frozen on 2026-10-10. The record is
[abi/exports.txt](../abi/exports.txt): every exported C function, each with the ABI
version that added it (95 are `1.0`; the three of
[ABI 1.1](#abi-11-parse-budgets-and-exporter-slips) are `1.1`). Three checks hold it:

- `cargo test -p tessstep-capi --test abi` requires the bridge's `#[unsafe(no_mangle)]`
  functions, the declarations in `tessstep.h` and the record to agree exactly, and
  pins the size and an FNV-1a digest of the `1.0` entries, so changing the 1.0 set
  needs a deliberate edit of both the record and the test;
- `scripts/check_capi.py` (and release packaging) requires the installed shared
  library to export exactly the recorded symbols and nothing else, and the static
  archive to define exactly those `ts_` symbols;
- the installed C consumer asserts the record layouts with `_Static_assert`, and the
  Rust contract tests assert them with `size_of` / `offset_of`.

Policy within ABI 1:

- **Frozen:** every recorded symbol and its signature and contract; every record
  layout and published revision size; the status, stage, kind, flag, pairing,
  preference, protocol and style values; the extensible-record rule; the shared and
  static CMake targets and their package layout.
- **1.x may only add:** new functions (appended to the record with their version),
  new records, new flag bits in flags words that reject unknown bits, new revisions
  of the three extensible records that only append fields, and new status, kind or
  stage values that only new functions or new opt-in flags produce, so an ABI 1.0
  caller never receives one unasked (as `TS_ROOT_SHELL_BASED_SURFACE_MODEL` appears
  only with `TS_DISCOVER_SURFACE_MODELS`).
- **Anything else needs ABI 2:** removing or renaming a symbol, changing a signature,
  a record layout or a published revision, changing the meaning of a status, value
  or default in a way an ABI 1 consumer can observe, or growing a fixed record.

The C++ wrapper is header-only over the frozen C ABI; its source compatibility follows
the same policy (members and overloads may be added).

## Post-1.0

Left for after the freeze, each additive under the policy above:

- **Document-level failure text.** Only sessions and per-root reports carry text; a
  document-level call reports the located `ts_import_error` only.
- **Annotation units text.** `ts_discovered_presentation` unit failures carry a located
  error but no text (only shape roots have `ts_discovery_root_report_at`).
- **Surface models in sessions and assemblies.** The Rust session and assembly import
  do not handle surface models; the C session imports them without sharing.
- **Assembly and scene limits.** The frozen `ts_assembly_options` copies its instance
  and depth budgets into the scene limits; the extensible entry point keeps the Rust
  scene limits. Neither exposes the scene limits themselves.
- **Debug static consumers.** The static target is tested in Release only.

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
package do not need Cargo or knowledge of Rust linkage. `TessSTEP::TessSTEP_static`
is the static configuration ([static linkage](#static-linkage)). Publish the supported
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

The document, mesh, scene, appearance, solid-import, tessellation, presentation,
assembly, import-option, discovery, surface-model, session and diagnostic-text subsets,
and the export record ([ABI 1 freeze](#abi-1-freeze)), are exercised by `cargo test -p tessstep-capi` and
`python3 scripts/check_capi.py`. The latter installs and relocates the package,
compiles/runs independent C11 and C++17 consumers in Debug/Release (plus a C-only
CMake project, and a Release build linked with `TessSTEP::TessSTEP_static`) with Rust tool
invocations blocked, and verifies the exact exported symbol set. Release packaging
runs those same consumers. Native consumer ASan/UBSan is available with
`--sanitizers`; this does not instrument the Rust library. Mesh and presentation lifetime, pointer stability, ownership, failure and zero-copy
acceptance checks run in those consumers. See [ARCHITECTURE.md](ARCHITECTURE.md) and
[CONFORMANCE.md](CONFORMANCE.md).

Milestone 5 product graphs and unit/assembly adapters are Rust-only operations. The
C ABI and C++ wrapper expose physical documents, independent meshes and explicitly placed mesh scenes; no product layouts,
placement descriptions or Rust ownership conventions are exposed through ABI 1.
