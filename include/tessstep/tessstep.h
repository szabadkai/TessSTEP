#ifndef TESSSTEP_H
#define TESSSTEP_H

#include <stddef.h>
#include <stdint.h>

/* TESSSTEP_STATIC (defined by TessSTEP::TessSTEP_static) selects the static library. */
#if defined(_WIN32) && !defined(TESSSTEP_STATIC)
#define TS_API __declspec(dllimport)
#else
#define TS_API
#endif
#if defined(_WIN32)
#define TS_CALL __cdecl
#else
#define TS_CALL
#endif
#ifdef __cplusplus
extern "C" {
#endif

/* ABI 1: C11, 64-bit platforms. All layouts and numeric values below are frozen.
 * Future incompatible records/functions get new names; existing records do not
 * grow, except the extensible records of Milestone 22 (see ts_import_options).
 * No Rust representation is part of this protocol. */
#define TS_ABI_VERSION 1u
typedef uint32_t ts_status;
#define TS_OK 0u
#define TS_INVALID_ARGUMENT 1u
#define TS_PARSE_ERROR 2u
#define TS_UNSUPPORTED 3u
#define TS_RESOURCE_LIMIT 4u
#define TS_NOT_FOUND 5u
#define TS_INTERNAL_ERROR 6u
#define TS_INVALID_MESH 7u
#define TS_INVALID_SCENE 8u
#define TS_INVALID_APPEARANCE 9u
#define TS_INVALID_GEOMETRY 10u

typedef struct ts_document ts_document;
typedef struct ts_diagnostics ts_diagnostics;

/* UTF-8 bytes, not NUL terminated. {NULL,0} is empty. Borrowed until the last
 * owning document/report handle is released; never free or modify these bytes. */
typedef struct ts_string_view {
    const char *data;
    size_t size;
} ts_string_view;

/* Initialize with ts_parse_options_init, then adjust budgets. NULL options means
 * defaults. Zero budgets mean zero, not unlimited. Both header fields must match.
 * Logical allocation limits are not an RSS cap. Nesting is also hard-capped by
 * the parser. options and input bytes are borrowed only for the parse call. */
typedef struct ts_parse_options {
    uint32_t struct_size;
    uint32_t abi_version;
    uint64_t max_input_bytes;
    uint64_t max_token_bytes;
    uint64_t max_string_bytes;
    uint64_t max_entities;
    uint64_t max_nesting_depth;
    uint64_t max_aggregate_elements;
    uint64_t max_total_values;
    uint64_t max_symbols;
    uint64_t max_records;
    uint64_t max_sections;
} ts_parse_options;

typedef struct ts_document_info {
    uint64_t entity_count;
    uint64_t header_count;
    uint64_t data_section_count;
} ts_document_info;

typedef struct ts_entity_info {
    uint64_t id;
    uint64_t record_count; /* One for simple entities; components for complex. */
} ts_entity_info;

#define TS_SEVERITY_ERROR 1u
#define TS_SEVERITY_WARNING 2u
typedef struct ts_diagnostic {
    uint32_t severity;
    uint32_t reserved; /* Always zero. */
    uint64_t entity_id; /* Zero means no owning entity. */
    uint64_t start_offset; /* Zero-based byte offset; end is exclusive. */
    uint64_t end_offset;
    uint64_t line; /* One-based line and byte column of the start. */
    uint64_t column;
    ts_string_view code; /* Stable TS code, e.g. TS1103. */
    ts_string_view message;
} ts_diagnostic;

/* General contract:
 * - Non-NULL pointers must address valid, aligned objects of the declared type.
 *   Buffers must span the stated count (at most PTRDIFF_MAX bytes). Output storage
 *   must be writable and disjoint from inputs and other outputs. Invalid/dangling
 *   non-NULL pointers and double releases are caller errors, not detectable errors.
 * - All output pointers are required. Valid outputs are cleared before validation,
 *   including when another argument is NULL. Failure leaves zero/NULL outputs,
 *   except parse failures may return a newly owned diagnostic report.
 * - Every acquired/retained handle needs one matching release. Copying its pointer
 *   does not retain. Only this library releases its allocations. release(NULL) is
 *   a no-op. Other NULL handles return TS_INVALID_ARGUMENT.
 * - Documents and reports are immutable. Reads/retains can run concurrently while
 *   a reference stays alive. Do not release the last reference during any use,
 *   including a retain or use of borrowed text. Separate parses are independent.
 * - No callbacks, global error state, external fetching or filesystem access.
 *   Unwinding panics become TS_INTERNAL_ERROR, with no partial output. Process
 *   aborts, invalid pointers and allocator termination cannot be recovered.
 */
TS_API uint32_t TS_CALL ts_api_version(void);
TS_API ts_status TS_CALL ts_parse_options_init(ts_parse_options *out);

/* Parse a complete physical STEP buffer (original encoding bytes, not necessarily
 * UTF-8). data may be NULL only when size==0; empty input yields TS_PARSE_ERROR.
 * Success acquires one document and leaves error_report NULL. Failure leaves
 * document NULL; syntax/unsupported/budget errors acquire one error_report.
 * Parse success does NOT imply reference, schema, geometry or mesh validity. */
TS_API ts_status TS_CALL ts_document_parse(const uint8_t *data, size_t size,
    const ts_parse_options *options, ts_document **out, ts_diagnostics **error_report);
TS_API ts_status TS_CALL ts_document_retain(const ts_document *document);
TS_API void TS_CALL ts_document_release(const ts_document *document);
TS_API ts_status TS_CALL ts_document_get_info(const ts_document *document, ts_document_info *out);
/* Zero-based source-order index. Out of range returns TS_NOT_FOUND. */
TS_API ts_status TS_CALL ts_document_entity_at(const ts_document *document, size_t index, ts_entity_info *out);
/* Entity ID lookup and zero-based component index; absent IDs/components return
 * TS_NOT_FOUND. Text is borrowed from document, without copying. */
TS_API ts_status TS_CALL ts_document_record_name(const ts_document *document,
    uint64_t entity_id, size_t component, ts_string_view *out);
/* Separate reference-existence/trust analysis, not schema validation. Acquires an
 * independent report (even if empty). It outlives document. TS_OK means analysis
 * completed, not that the report contains no errors. No external resources fetched. */
TS_API ts_status TS_CALL ts_document_diagnostics(const ts_document *document, ts_diagnostics **out);
TS_API void TS_CALL ts_diagnostics_release(ts_diagnostics *report);
TS_API ts_status TS_CALL ts_diagnostics_count(const ts_diagnostics *report, size_t *out);
TS_API ts_status TS_CALL ts_diagnostics_get(const ts_diagnostics *report, size_t index, ts_diagnostic *out);

/* Owned indexed meshes. Creation copies the inputs and validates finite positions,
 * nondegenerate triangles, oriented manifold edge/vertex incidence and connectivity
 * statistics. Self-intersection and material containment are not certified.
 * This imports triangles; it does not convert a STEP document into a mesh. */
typedef struct ts_mesh ts_mesh;
typedef struct ts_mesh_options {
    uint32_t struct_size, abi_version;
    uint64_t max_vertices, max_triangles, max_work;
    uint32_t require_solid; /* 0: open/disconnected manifold meshes allowed; 1: one closed
                              component with positive algebraic volume required. */
    uint32_t reserved; /* Must be zero. */
} ts_mesh_options;
/* All buffers are immutable, borrowed without copying until the last mesh release.
 * Positions: 3*vertex_count doubles, XYZ in metres. Triangles: 3*triangle_count
 * uint32 indices, counterclockwise relative to normals. Normals: 9*triangle_count
 * doubles (unit XYZ per triangle corner). UVs: 6*triangle_count doubles (UV per corner).
 * face_ids: triangle_count uint64 model-local identities. Imported meshes receive
 * faceted normals, zero UVs and face id 0. Arrays have no padding/stride.
 * boundary_edges==0 means topologically watertight. signed_volume is algebraic,
 * in cubic metres; for open meshes it is not a physical volume. */
typedef struct ts_mesh_view {
    const double *positions;
    size_t vertex_count;
    const uint32_t *triangles;
    size_t triangle_count;
    const double *normals;
    const double *uvs;
    const uint64_t *face_ids;
    uint64_t boundary_edges, components;
    double signed_volume;
} ts_mesh_view;
TS_API ts_status TS_CALL ts_mesh_options_init(ts_mesh_options *out);
/* Inputs are borrowed for this call only. Counts are vertices/triangles, not scalar
 * counts. NULL is permitted only for zero count. Empty/invalid meshes return
 * TS_INVALID_MESH; budget exhaustion returns TS_RESOURCE_LIMIT. Output is cleared
 * on failure. Initialize options with ts_mesh_options_init; NULL selects defaults. */
TS_API ts_status TS_CALL ts_mesh_create(const double *positions, size_t vertex_count,
    const uint32_t *triangles, size_t triangle_count, const ts_mesh_options *options,
    ts_mesh **out);
TS_API ts_status TS_CALL ts_mesh_retain(const ts_mesh *mesh);
TS_API void TS_CALL ts_mesh_release(const ts_mesh *mesh);
/* Concurrent immutable reads/retains are allowed while ownership remains alive.
 * Copying the view does not retain the mesh; retain explicitly for longer use. */
TS_API ts_status TS_CALL ts_mesh_get_view(const ts_mesh *mesh, ts_mesh_view *out);

/* Planar FACETED_BREP import. Explicit entity ID and source length scale in metres.
 * Only the selected root's local reference closure is checked against a reduced
 * structural profile; this does not validate FILE_SCHEMA or unrelated records.
 * Requires planar FACE_SURFACE/ADVANCED_FACE and POLY_LOOP boundaries. No healing,
 * assembly placement, unit inference, curved geometry or full AP conformance.
 * Model/chord/UV distances are metres, angles radians. max_work is an independent
 * per-stage budget, not a whole-call time/RSS cap. Zero budgets mean zero.
 * Initialize options before edits; NULL options selects these same defaults.
 * ABI 1 freezes this new 80-byte record, without changing any previous layout. */
typedef struct ts_faceted_options {
    uint32_t struct_size, abi_version;
    double model_distance, model_angle, chord, normal_angle, uv_tolerance;
    uint64_t max_work, max_records, max_vertices, max_triangles;
} ts_faceted_options;
#define TS_IMPORT_STAGE_NONE 0u
#define TS_IMPORT_STAGE_PROFILE 1u
#define TS_IMPORT_STAGE_GEOMETRY 2u
#define TS_IMPORT_STAGE_TOPOLOGY 3u
#define TS_IMPORT_STAGE_TESSELLATION 4u
#define TS_IMPORT_STAGE_PRODUCT 5u
#define TS_IMPORT_STAGE_PRESENTATION 6u
/* All-zero on success/invalid call arguments; entity/offsets zero if unavailable.
 * Offsets are half-open bytes in the original document. No borrowed storage. */
typedef struct ts_import_error {
    uint32_t stage, reserved;
    uint64_t entity_id, start_offset, end_offset;
} ts_import_error;
TS_API ts_status TS_CALL ts_faceted_options_init(ts_faceted_options *out);
/* document/options borrowed during call. out is required, error optional; writable
 * outputs must be aligned/disjoint from each other and all inputs. Both are cleared
 * before validation. Failure returns no mesh. Success transfers one mesh acquisition
 * independent of document lifetime; release with ts_mesh_release. Mesh positions are
 * metres, triangle face_ids are original STEP face IDs. Concurrent immutable calls
 * are allowed while the document stays alive. metres_per_unit must be finite > 0. */
TS_API ts_status TS_CALL ts_document_tessellate_faceted(const ts_document *document,
    uint64_t entity_id, double metres_per_unit, const ts_faceted_options *options,
    ts_mesh **out, ts_import_error *error);

/* Planar edge-based MANIFOLD_SOLID_BREP profile. Same units, ownership, output
 * clearing, budgets and error contract as ts_document_tessellate_faceted.
 * EDGE_LOOP/ORIENTED_EDGE/EDGE_CURVE/LINE and VERTEX_POINT are supported.
 * Both derived ORIENTED_EDGE endpoint slots must be * or $ (the tolerant default
 * policy below). A single FACE_BOUND is accepted as the outer; multiple bounds
 * require one explicit FACE_OUTER_BOUND. Shared identity follows STEP
 * vertices/edges; no proximity welding or healing. */
typedef ts_faceted_options ts_planar_options;
TS_API ts_status TS_CALL ts_planar_options_init(ts_planar_options *out);
TS_API ts_status TS_CALL ts_document_tessellate_planar(const ts_document *document,
    uint64_t entity_id, double metres_per_unit, const ts_planar_options *options,
    ts_mesh **out, ts_import_error *error);

/* Profile conformance policy; additive ABI 1 16-byte record. Initialize with
 * ts_import_policy_init. flags 0 is the tolerant default used by
 * ts_document_tessellate_planar. TS_IMPORT_STRICT rejects exporter deviations the
 * default accepts without changing geometry: $ instead of * in derived
 * ORIENTED_EDGE endpoint slots. Unknown flag bits or nonzero reserved fail with
 * TS_INVALID_ARGUMENT after clearing outputs. */
#define TS_IMPORT_STRICT 1u
typedef struct ts_import_policy {
    uint32_t struct_size, abi_version, flags, reserved;
} ts_import_policy;
TS_API ts_status TS_CALL ts_import_policy_init(ts_import_policy *out);
/* ts_document_tessellate_planar with an explicit policy; NULL policy selects the
 * tolerant default. policy is borrowed during the call only. */
TS_API ts_status TS_CALL ts_document_tessellate_planar_with_policy(
    const ts_document *document, uint64_t entity_id, double metres_per_unit,
    const ts_planar_options *options, const ts_import_policy *policy,
    ts_mesh **out, ts_import_error *error);

/* Existing triangulated tessellations (Milestone 20): a selected TESSELLATED_SOLID,
 * TESSELLATED_SHELL or triangulated surface set becomes an owned mesh WITHOUT
 * retessellation. TRIANGULATED_FACE / COMPLEX_TRIANGULATED_FACE triangles, strips
 * and fans are published as given (strip/fan triangles that repeat an index are
 * skipped and counted). Vertex identity is (COORDINATES_LIST, point index), never
 * proximity; TESSELLATED_CONNECTING_EDGE joins exactly the points it declares equal
 * (equal coordinates, segments on both faces' triangles). TESSELLATED_EDGE and
 * TESSELLATED_VERTEX items add no triangles; only their counts are reported here.
 * Supplied normals must agree with winding (never flipped); solids must form one
 * closed component with positive volume. Links are retained, never decoded.
 * Budgets as ts_faceted_options (zero means zero); max_work also bounds mesh
 * validation. flags 0 is the tolerant default: without PNINDEX a PNMAX below
 * NPOINTS (at most one normal) indexes the whole list and is counted.
 * TS_IMPORT_STRICT rejects it. Unknown flags/reserved fail with TS_INVALID_ARGUMENT.
 * Additive ABI 1 records: 48-byte options, 80-byte info. */
#define TS_TESSELLATED_SOLID 1u
#define TS_TESSELLATED_SHELL 2u
#define TS_TESSELLATED_SURFACE_SET 3u
typedef struct ts_tessellated_options {
    uint32_t struct_size, abi_version;
    uint64_t max_work, max_records, max_vertices, max_triangles;
    uint32_t flags, reserved;
} ts_tessellated_options;
/* All-zero unless the import succeeds. link_id is the solid's GEOMETRIC_LINK or the
 * shell's TOPOLOGICAL_LINK (zero when absent); IDs are physical STEP IDs. */
typedef struct ts_tessellated_info {
    uint32_t kind, reserved;
    uint64_t root_id, link_id;
    uint64_t face_count, linked_face_count, edge_count, vertex_count;
    uint64_t skipped_degenerate, joined_points, pnmax_deviations;
} ts_tessellated_info;
TS_API ts_status TS_CALL ts_tessellated_options_init(ts_tessellated_options *out);
/* Same document, unit, ownership, output-clearing and error contract as
 * ts_document_tessellate_faceted; there is no tessellation stage. out is required;
 * info and error are optional and cleared before validation. Success transfers one
 * mesh acquisition (release with ts_mesh_release) whose face_ids are the physical
 * STEP IDs of the tessellated faces (or of the surface set), corner normals are
 * supplied or faceted, UVs zero. */
TS_API ts_status TS_CALL ts_document_import_tessellated(const ts_document *document,
    uint64_t entity_id, double metres_per_unit, const ts_tessellated_options *options,
    ts_mesh **out, ts_tessellated_info *info, ts_import_error *error);

/* Presentation tessellations: a selected TESSELLATED_ANNOTATION_OCCURRENCE (styling
 * a TESSELLATED_GEOMETRIC_SET) or TESSELLATED_GEOMETRIC_SET as placed graphics:
 * one polyline per TESSELLATED_CURVE_SET line strip, TESSELLATED_POINT_SET points
 * and surface-set fill triangles. REPOSITIONED_TESSELLATED_ITEM placements are
 * composed and applied; positions are metres in the containing representation's
 * coordinate system. Graphics are NOT meshes: fills need not be manifold, oriented
 * or of positive area (zero-area triangles are kept and counted), supplied normals
 * are checked for shape only and not published, styles are retained as entity IDs
 * without decoding. Vertex identity is (coordinate list, index, placement).
 * Options add max_polyline_points and max_depth (geometric-set nesting, the
 * selected set counting one) to the tessellated budgets; flags as above.
 * Additive ABI 1 records: 64-byte options and info, 24-byte item, 104-byte view. */
typedef struct ts_presentation ts_presentation;
#define TS_PRESENTATION_CURVE_SET 1u
#define TS_PRESENTATION_POINT_SET 2u
#define TS_PRESENTATION_SURFACE_SET 3u
typedef struct ts_presentation_options {
    uint32_t struct_size, abi_version;
    uint64_t max_work, max_records, max_vertices, max_triangles;
    uint64_t max_polyline_points, max_depth;
    uint32_t flags, reserved;
} ts_presentation_options;
/* occurrence_id is zero when a geometric set was selected directly. */
typedef struct ts_presentation_info {
    uint64_t root_id, occurrence_id, geometric_set_id, style_count, item_count;
    uint64_t skipped_degenerate, zero_area_triangles, pnmax_deviations;
} ts_presentation_info;
/* One leaf use, in depth-first child order; placements counts the composed
 * repositionings, including the item's own. supplied_normals is 0 or 1. */
typedef struct ts_presentation_item {
    uint64_t entity_id;
    uint32_t kind, supplied_normals;
    uint64_t placements;
} ts_presentation_item;
/* Immutable arrays borrowed without copying until the last presentation release.
 * An array is NULL exactly when its count is zero (polyline_offsets is never NULL).
 * positions: 3*vertex_count doubles. polyline_points: vertex indices; polyline i
 * spans [polyline_offsets[i], polyline_offsets[i+1]) (polyline_count+1 offsets,
 * the first zero). triangles: 3*triangle_count indices in the file's winding.
 * *_items: the physical STEP ID of each primitive's source set. */
typedef struct ts_presentation_view {
    const double *positions;
    size_t vertex_count;
    const uint32_t *polyline_points;
    size_t polyline_point_count;
    const uint64_t *polyline_offsets;
    const uint64_t *polyline_items;
    size_t polyline_count;
    const uint32_t *triangles;
    const uint64_t *triangle_items;
    size_t triangle_count;
    const uint32_t *points;
    const uint64_t *point_items;
    size_t point_count;
} ts_presentation_view;
TS_API ts_status TS_CALL ts_presentation_options_init(ts_presentation_options *out);
/* Document/unit/output/error contract of ts_document_import_tessellated; there is
 * no tessellation stage. Success transfers one presentation acquisition, independent
 * of the document, released with ts_presentation_release. */
TS_API ts_status TS_CALL ts_document_import_presentation(const ts_document *document,
    uint64_t entity_id, double metres_per_unit, const ts_presentation_options *options,
    ts_presentation **out, ts_import_error *error);
TS_API ts_status TS_CALL ts_presentation_retain(const ts_presentation *presentation);
TS_API void TS_CALL ts_presentation_release(const ts_presentation *presentation);
/* Scalar copies; outputs are cleared on failure. Concurrent immutable queries are
 * allowed while an acquisition stays alive. Out-of-range indices are TS_NOT_FOUND. */
TS_API ts_status TS_CALL ts_presentation_get_info(const ts_presentation *presentation,
    ts_presentation_info *out);
TS_API ts_status TS_CALL ts_presentation_item_at(const ts_presentation *presentation,
    size_t index, ts_presentation_item *out);
/* Style assignment entity IDs of the occurrence, in file order. */
TS_API ts_status TS_CALL ts_presentation_style_at(const ts_presentation *presentation,
    size_t index, uint64_t *out);
/* Zero-copy view; copying it does not retain. Retain explicitly for longer use. */
TS_API ts_status TS_CALL ts_presentation_get_view(const ts_presentation *presentation,
    ts_presentation_view *out);



/* Immutable assembly scenes. Assets share retained mesh buffers; instances are an
 * explicitly supplied forest, not decoded STEP placements. IDs are scene-local
 * nonzero uint64 values. Parent/asset zero means root/group respectively. */
typedef struct ts_scene ts_scene;
typedef struct ts_scene_options {
    uint32_t struct_size, abi_version;
    uint64_t max_assets, max_instances, max_depth, max_work;
} ts_scene_options;
typedef struct ts_scene_asset {
    uint64_t id;
    const ts_mesh *mesh;
} ts_scene_asset;
/* p_parent = linear * p_local + translation; row-major linear, column vectors,
 * local coordinates and translations in metres. Zero linear is singular, NOT
 * identity. Finite invertible local/world maps are required. Relative pivot
 * threshold: 64*DBL_EPSILON. Reflections and nonuniform scales are supported. */
typedef struct ts_scene_instance {
    uint64_t id, parent_id, asset_id;
    double linear[9], translation[3];
} ts_scene_instance;
typedef struct ts_scene_instance_info {
    ts_scene_instance source;
    double world_linear[9], world_translation[3];
    uint64_t depth; /* roots have depth one */
    uint32_t mirrored, reserved; /* reverse winding when rendering a mirrored use */
} ts_scene_instance_info;
typedef struct ts_scene_info { size_t asset_count, instance_count; } ts_scene_info;
TS_API ts_status TS_CALL ts_scene_options_init(ts_scene_options *out);
/* Inputs are borrowed for this call; each asset mesh must remain live throughout.
 * Successful creation retains assets independently. No mesh buffers are copied.
 * Empty scenes are allowed; NULL arrays require zero counts. Instances may precede
 * parents. Cycles, duplicate/zero IDs, missing links and unusable transforms return
 * TS_INVALID_SCENE. Budgets return TS_RESOURCE_LIMIT. Initialize options; NULL uses
 * defaults. No partial output; a valid output slot is cleared on every failure. */
TS_API ts_status TS_CALL ts_scene_create(const ts_scene_asset *assets, size_t asset_count,
    const ts_scene_instance *instances, size_t instance_count, const ts_scene_options *options,
    ts_scene **out);
TS_API ts_status TS_CALL ts_scene_retain(const ts_scene *scene);
TS_API void TS_CALL ts_scene_release(const ts_scene *scene);
TS_API ts_status TS_CALL ts_scene_get_info(const ts_scene *scene, ts_scene_info *out);
/* Scalar copies in input order; out-of-range is TS_NOT_FOUND. Outputs are cleared
 * on failure. Concurrent immutable queries are allowed with a live acquisition. */
TS_API ts_status TS_CALL ts_scene_instance_at(const ts_scene *scene, size_t index,
    ts_scene_instance_info *out);
/* Returns an independently retained mesh acquisition, released with ts_mesh_release.
 * Its zero-copy views can outlive the scene. Unknown asset ID is TS_NOT_FOUND. */
TS_API ts_status TS_CALL ts_scene_asset_mesh(const ts_scene *scene, uint64_t asset_id, ts_mesh **out);
/* Explicitly copy ONLY this occurrence's asset into world coordinates, revalidate,
 * and return an owned mesh. No merging, subtree expansion, or tolerance guarantee
 * under scaling. Reflections reverse triangle/corner order; normals use inverse
 * transpose. Face IDs stay asset-local. Missing instance/group is TS_NOT_FOUND;
 * numeric transform failures TS_INVALID_SCENE; mesh validation TS_INVALID_MESH.
 * options follows ts_mesh_create, including require_solid. */
TS_API ts_status TS_CALL ts_scene_bake_instance(const ts_scene *scene, uint64_t instance_id,
    const ts_mesh_options *options, ts_mesh **out);

/* Immutable appearance layer over one retained scene. No STEP style decoding,
 * renderer, lighting, textures or color-space conversion is implied. */
typedef struct ts_appearance ts_appearance;
typedef struct ts_appearance_options {
    uint32_t struct_size, abi_version;
    uint64_t max_materials, max_bindings, max_work;
} ts_appearance_options;
/* Nonzero appearance-local ID. Linear-light RGB and straight opacity in [0,1],
 * finite, without clamping. Alpha 0 is transparent, 1 opaque; no premultiplication. */
typedef struct ts_material { uint64_t id; double rgba[4]; } ts_material;
#define TS_STYLE_ASSET 1u
#define TS_STYLE_ASSET_FACE 2u
#define TS_STYLE_INSTANCE 3u
#define TS_STYLE_INSTANCE_FACE 4u
/* id names an asset or instance according to kind; reserved must be zero.
 * face_id must be zero for non-face kinds. For face kinds, zero is an ordinary
 * asset-local face ID, not a wildcard (triangle import currently uses face 0). */
typedef struct ts_style_target {
    uint32_t kind, reserved;
    uint64_t id, face_id;
} ts_style_target;
typedef struct ts_style_binding { ts_style_target target; uint64_t material_id; } ts_style_binding;
typedef struct ts_resolved_material { ts_material material; ts_style_target source; } ts_resolved_material;
typedef struct ts_appearance_info { size_t material_count, binding_count; } ts_appearance_info;
TS_API ts_status TS_CALL ts_appearance_options_init(ts_appearance_options *out);
/* Retains scene independently; copies material/binding records, never geometry.
 * Inputs are borrowed only for this call; NULL arrays require zero counts. Empty
 * palettes/bindings are permitted. Every binding must reference an existing target,
 * face and material, even if overridden; duplicate targets/IDs are rejected.
 * Invalid colors/identities/bindings return TS_INVALID_APPEARANCE, malformed records
 * TS_INVALID_ARGUMENT, budgets TS_RESOURCE_LIMIT. Initialize options; NULL selects
 * defaults. Required writable output is cleared on failure; no partial result. */
TS_API ts_status TS_CALL ts_appearance_create(const ts_scene *scene,
    const ts_material *materials, size_t material_count,
    const ts_style_binding *bindings, size_t binding_count,
    const ts_appearance_options *options, ts_appearance **out);
TS_API ts_status TS_CALL ts_appearance_retain(const ts_appearance *appearance);
TS_API void TS_CALL ts_appearance_release(const ts_appearance *appearance);
TS_API ts_status TS_CALL ts_appearance_get_info(const ts_appearance *appearance, ts_appearance_info *out);
/* Scalar copies in input order; out-of-range returns TS_NOT_FOUND. */
TS_API ts_status TS_CALL ts_appearance_material_at(const ts_appearance *appearance, size_t index, ts_material *out);
TS_API ts_status TS_CALL ts_appearance_binding_at(const ts_appearance *appearance, size_t index, ts_style_binding *out);
/* Precedence: exact instance face > nearest instance/ancestor override > asset
 * face > asset default. Instance-face assignments never propagate to children.
 * No fallback color is invented. TS_OK with material.id=0 and zero source means
 * unstyled; an explicitly transparent material retains a nonzero ID.
 * source identifies the winning assignment (including the original ancestor).
 * Missing/group instances or out-of-range triangles return TS_NOT_FOUND. Output
 * is cleared on failure. Allocation-free query; triangle order also matches the
 * corresponding ts_scene_bake_instance output, including mirrored instances.
 * Concurrent immutable queries require a live appearance acquisition. */
TS_API ts_status TS_CALL ts_appearance_resolve_triangle(const ts_appearance *appearance,
    uint64_t instance_id, size_t triangle, ts_resolved_material *out);
/* Returns an independently retained original scene, paired with ts_scene_release.
 * Its asset mesh acquisitions and zero-copy views may outlive this appearance. */
TS_API ts_status TS_CALL ts_appearance_get_scene(const ts_appearance *appearance, ts_scene **out);

/* Schema selection (Milestone 21). The first FILE_SCHEMA entry, borrowed from the
 * document (valid until its last release), and the protocol its name (object
 * identifier removed, compared in upper case) selects from an exact table:
 * CONFIG_CONTROL_DESIGN, the AP203 edition 2 MIM long form, AUTOMOTIVE_DESIGN and
 * the AP242 MIM long form. Every other name, including AUTOMOTIVE_DESIGN_CC*, is
 * TS_PROTOCOL_UNKNOWN. Selection is not validation: no ISO schema is bundled and
 * imports use bounded original profiles. A header without a schema entry is
 * TS_NOT_FOUND. Additive ABI 1 record: 32 bytes. */
#define TS_PROTOCOL_UNKNOWN 0u
#define TS_PROTOCOL_AP203 1u
#define TS_PROTOCOL_AP203E2 2u
#define TS_PROTOCOL_AP214 3u
#define TS_PROTOCOL_AP242 4u
typedef struct ts_schema_info {
    uint32_t protocol, reserved;
    uint64_t schema_count;
    ts_string_view declared;
} ts_schema_info;
TS_API ts_status TS_CALL ts_document_get_schema(const ts_document *document, ts_schema_info *out);

/* Assembly import (Milestone 21). Discovers every solid and shape tessellation
 * root, selects exact B-reps over paired tessellations, imports each selected root
 * in its representation context's units (a root whose context declares no units is
 * not imported; an alternative is tried only after its selection fails), links the
 * meshes to products, definitions and next assembly usage occurrences placed by
 * context dependent shape representations, and adapts surface colours of styled
 * solids, outer shells, faces and shape representations. Each B-rep root's model
 * tolerance is its context's length uncertainty, never below
 * minimum_model_tolerance_m; chord_m and angle_rad bound tessellation. A missing
 * or ambiguous occurrence placement fails with TS_INVALID_GEOMETRY at stage
 * TS_IMPORT_STAGE_PRODUCT; placement is never defaulted. Failed roots and product
 * or style records outside the bundled profiles are reported, not placed; complete
 * is 0 when an occurrence or placement record was excluded. flags accepts
 * TS_IMPORT_STRICT. Additive ABI 1 records: 72-byte options, 72-byte info, 40-byte
 * node, 48-byte product, 56-byte root, 48-byte exclusion, 96-byte style info. */
typedef struct ts_assembly ts_assembly;
typedef struct ts_assembly_options {
    uint32_t struct_size, abi_version;
    uint64_t max_work, max_records, max_instances, max_depth;
    double chord_m, angle_rad, minimum_model_tolerance_m;
    uint32_t flags, reserved;
} ts_assembly_options;
typedef struct ts_assembly_info {
    uint64_t node_count, product_count, occurrence_count, root_count, imported_root_count;
    uint64_t unplaced_count, unimported_count, excluded_count;
    uint32_t complete, reserved;
} ts_assembly_info;
/* One scene instance: a definition node (root_id zero; occurrence_id zero for a root
 * definition) or an asset leaf (root_id nonzero; its scene asset ID is root_id and
 * it is an identity child of the definition node). All IDs are STEP instance IDs
 * except instance_id, the scene instance ID. */
typedef struct ts_assembly_node {
    uint64_t instance_id, definition_id, occurrence_id, representation_id, root_id;
} ts_assembly_node;
/* PRODUCT.id and PRODUCT.name as written, borrowed until the last assembly release. */
typedef struct ts_assembly_product {
    uint64_t definition_id, product_id;
    ts_string_view id, name;
} ts_assembly_product;
#define TS_ROOT_MANIFOLD_SOLID_BREP 1u
#define TS_ROOT_BREP_WITH_VOIDS 2u
#define TS_ROOT_FACETED_BREP 3u
#define TS_ROOT_TESSELLATED_SOLID 4u
#define TS_ROOT_TESSELLATED_SHELL 5u
#define TS_ROOT_TESSELLATED_SURFACE_SET 6u
/* status is TS_OK for an imported root, else its failure status and located error. */
typedef struct ts_assembly_root {
    uint64_t root_id;
    uint32_t kind, selected, status, reserved;
    ts_import_error error;
} ts_assembly_root;
/* A product or style record left out, its status and located error; structural is
 * 1 for an occurrence or placement record. */
typedef struct ts_assembly_exclusion {
    uint64_t record_id;
    uint32_t structural, status;
    ts_import_error error;
} ts_assembly_exclusion;
/* Styled items by use. Curve, point and context-dependent styles, layers and
 * invisibility are counted, never applied. */
typedef struct ts_style_info {
    uint64_t styled_items, asset_styles, face_styles, unimported, unsupported_targets;
    uint64_t no_surface_colour, context_styles, conflicts, complex_items, layers;
    uint64_t invisibility, excluded;
} ts_style_info;
TS_API ts_status TS_CALL ts_assembly_options_init(ts_assembly_options *out);
/* Success transfers one assembly acquisition, independent of the document, released
 * with ts_assembly_release. Output/error contract of ts_document_import_tessellated. */
TS_API ts_status TS_CALL ts_document_import_assembly(const ts_document *document,
    const ts_assembly_options *options, ts_assembly **out, ts_import_error *error);
TS_API ts_status TS_CALL ts_assembly_retain(const ts_assembly *assembly);
TS_API void TS_CALL ts_assembly_release(const ts_assembly *assembly);
/* Scalar copies; outputs are cleared on failure; out-of-range is TS_NOT_FOUND.
 * Concurrent immutable queries are allowed while an acquisition stays alive. */
TS_API ts_status TS_CALL ts_assembly_get_info(const ts_assembly *assembly, ts_assembly_info *out);
TS_API ts_status TS_CALL ts_assembly_node_at(const ts_assembly *assembly, size_t index,
    ts_assembly_node *out);
TS_API ts_status TS_CALL ts_assembly_product_at(const ts_assembly *assembly, size_t index,
    ts_assembly_product *out);
TS_API ts_status TS_CALL ts_assembly_root_at(const ts_assembly *assembly, size_t index,
    ts_assembly_root *out);
TS_API ts_status TS_CALL ts_assembly_excluded_at(const ts_assembly *assembly, size_t index,
    ts_assembly_exclusion *out);
/* Imported roots that no product definition's shape representation holds. */
TS_API ts_status TS_CALL ts_assembly_unplaced_at(const ts_assembly *assembly, size_t index,
    uint64_t *out);
TS_API ts_status TS_CALL ts_assembly_get_style_info(const ts_assembly *assembly,
    ts_style_info *out);
/* Independent acquisitions of the assembly's scene and of the imported appearance
 * over the same scene, released with ts_scene_release and ts_appearance_release.
 * Materials are the distinct colours in order of first use; RGB was read as
 * sRGB-encoded and converted to linear light, opacity is one minus transparency. */
TS_API ts_status TS_CALL ts_assembly_get_scene(const ts_assembly *assembly, ts_scene **out);
TS_API ts_status TS_CALL ts_assembly_get_appearance(const ts_assembly *assembly,
    ts_appearance **out);

/* Milestone 22: import options, root discovery, representation selection, solid and
 * surface-model import, import sessions.
 *
 * Extensible records. ts_import_options, ts_solid_info and ts_surface_model_info are
 * the only ABI 1 records that may grow. struct_size names the revision a caller was
 * compiled with (its sizeof). ABI 1.0 publishes revision 1 of each (TS_*_SIZE_1); a
 * 1.x release may publish revision 2, 3, ... that only append fields. The library
 * accepts every published revision size and reads or writes exactly that many
 * bytes: an input's fields beyond its revision take their initializer defaults, an
 * output's are not written. Any other size is TS_INVALID_ARGUMENT; abi_version must
 * be 1. Every other record keeps the exact-size rule. */
#define TS_IMPORT_OPTIONS_SIZE_1 168u
/* Opt-in policies in ts_import_options.flags beside TS_IMPORT_STRICT. Only
 * ts_import_options accepts these bits; every other flags word rejects them. */
#define TS_IMPORT_REPAIR_SEW 2u /* merge coincident boundary vertices and free edges */
#define TS_IMPORT_REPAIR_ORIENT 4u /* flip faces, reverse loops and inside-out shells */
#define TS_DISCOVER_SURFACE_MODELS 8u /* discovery also reports surface-model roots */
typedef struct ts_import_options {
    uint32_t struct_size, abi_version;
    uint32_t flags, reserved; /* bits above; other bits and nonzero reserved rejected */
    uint64_t max_work, max_records; /* profile decoding, adapter and discovery budgets */
    double model_distance_m, model_angle_rad; /* B-rep model tolerance */
    double chord_m, normal_angle_rad; /* tessellation tolerance */
    uint64_t max_tessellation_work, max_evaluations; /* shared by a solid's faces */
    uint64_t max_vertices, max_triangles; /* mesh output (also presentation graphics) */
    double sewing_factor; /* sewing tolerance / model distance; finite > 0 */
    double deviation_factor; /* accepted edge/vertex deviation / model distance; >= 1 */
    uint64_t max_assembly_instances, max_assembly_depth; /* product-structure expansion */
    uint64_t max_session_decoded_entities, max_session_adapted_entities, max_session_triangles;
    uint64_t max_polyline_points, max_presentation_depth; /* session presentation import */
} ts_import_options;
/* Write the defaults of the revision struct_size names (pass sizeof *out) into
 * exactly struct_size bytes: flags 0, max_work 5,000,000, max_records 100,000, model
 * tolerance 1e-7 m and 1e-8 rad, chord 1e-4 m, normal angle 0.1 rad, tessellation
 * work 50,000,000, evaluations 10,000,000, 1,000,000 vertices, 2,000,000 triangles,
 * sewing factor 10, deviation factor 16, 100,000 assembly instances, assembly depth
 * 1,024, session budgets of 1,000,000 decoded and 1,000,000 adapted records and
 * 20,000,000 triangles, 4,000,000 polyline points and presentation nesting depth 16.
 * Each discovery pass and import stage has its own max_work; per-face
 * polygon, trimming and sampling budgets keep library defaults capped by the
 * tessellation fields. Zero budgets mean zero. NULL or an unknown size writes nothing
 * and returns TS_INVALID_ARGUMENT. NULL options in the entry points below mean these
 * defaults; every entry point validates every field of the record.
 * Curved-profile tolerance policy: an edge farther than the model distance from its
 * face surface, or a vertex from its edge curve, is accepted and counted up to
 * deviation_factor times it and rejected beyond (TS_INVALID_GEOMETRY at the
 * geometry stage); TS_IMPORT_STRICT forces a factor of 1. Repair runs only when its
 * flags ask, on solids (never on surface models); strict and repair combine. */
TS_API ts_status TS_CALL ts_import_options_init(ts_import_options *out, size_t struct_size);
/* ts_import_options_init with max_work and max_records sized for this document as
 * Rust ImportOptions::for_document sizes them (at least 512 work units and one record
 * per entity; the one-million topology record ceiling still applies) and the
 * session budgets as SessionBudgets::for_document (at least 4 decoded and 16
 * adapted records and 256 triangles per entity). A NULL document zero-fills a valid
 * record (struct_size 0) and returns TS_INVALID_ARGUMENT. */
TS_API ts_status TS_CALL ts_import_options_for_document(const ts_document *document,
    ts_import_options *out, size_t struct_size);

/* Single-root solid import: a selected MANIFOLD_SOLID_BREP or BREP_WITH_VOIDS through
 * the curved profile (planes, cylinders, cones, spheres, tori, B-spline and swept
 * surfaces; line, conic and B-spline edges; supplied or computed pcurves; see
 * docs/CURVED_IMPORT.md), or a FACETED_BREP through the faceted profile, chosen by
 * the root's physical record. Units are explicit and finite > 0: metres_per_unit
 * scales lengths, radians_per_unit plane angles (cone semi-angles and supplied
 * pcurve parameters), for example from ts_discovery_root_at. No unit inference or
 * welding; repair only when its flags ask. Representation adaptations and repairs
 * (none moves a coordinate) are counted in info; faceted roots adapt nothing.
 * TS_IMPORT_STRICT rejects $ in derived ORIENTED_EDGE endpoint slots and every edge
 * or vertex deviation. Info revision 1: 216 bytes. */
#define TS_SOLID_INFO_SIZE_1 216u
typedef struct ts_solid_info {
    uint32_t struct_size, abi_version;
    uint32_t kind, reserved; /* TS_ROOT_MANIFOLD_SOLID_BREP, _BREP_WITH_VOIDS, _FACETED_BREP */
    uint64_t root_id, face_count, shell_count; /* shells: the outer one and each cavity */
    uint64_t inferred_outer_bounds, inserted_seams, split_edges, recharted_spheres;
    uint64_t collapsed_edges, collapsed_faces, supplied_pcurves, rejected_pcurves;
    /* Tolerance policy: edge uses accepted off their face surface, edge
     * ends accepted off their curve, B-spline sides charted as regular sides; the
     * largest accepted deviation and the working tolerance every later stage used
     * (the model distance plus twice that deviation plus any sewing merge distance),
     * in metres. */
    uint64_t off_surface_edges, off_curve_vertices, uncollapsed_sides;
    double largest_deviation_m, working_tolerance_m;
    /* Repairs (zero unless asked for): merged vertices and free edges and
     * their largest distance; closed shells that shared no edge or vertex before
     * sewing; flipped faces, reversed loops, reoriented and undecided closed shells;
     * the sewing tolerance in force (zero when not sewing). */
    uint64_t merged_vertices, merged_edges;
    double max_merge_distance_m;
    uint64_t disconnected_shells, flipped_faces, reversed_loops, reoriented_shells;
    uint64_t undecided_shells;
    double sewing_tolerance_m;
} ts_solid_info;
/* Document, ownership, output-clearing and error contract of
 * ts_document_import_tessellated; success transfers one mesh acquisition whose
 * face_ids are the STEP face IDs. info is optional: when non-NULL the caller sets
 * info->struct_size to a published revision (sizeof *info); an unknown size returns
 * TS_INVALID_ARGUMENT without writing info, otherwise the rest of that revision is
 * zeroed (abi_version 1) before validation and filled on success. */
TS_API ts_status TS_CALL ts_document_import_solid(const ts_document *document,
    uint64_t entity_id, double metres_per_unit, double radians_per_unit,
    const ts_import_options *options, ts_mesh **out, ts_solid_info *info,
    ts_import_error *error);

/* Root and unit discovery with representation selection. Finds every
 * MANIFOLD_SOLID_BREP, BREP_WITH_VOIDS and FACETED_BREP (solids), then every
 * TESSELLATED_SOLID, TESSELLATED_SHELL and triangulated surface set that is directly
 * a representation item (tessellations), the shape representations containing each
 * and their context's length and plane-angle units and length uncertainty; and every
 * TESSELLATED_ANNOTATION_OCCURRENCE with the callouts and annotation planes holding
 * it and their representations. Missing or conflicting units are per-root errors,
 * never defaults. Selection groups a solid with the tessellations linked to it
 * (GEOMETRIC_LINK, TOPOLOGICAL_LINK) or related by an untransformed representation
 * relationship between representations holding only one root each, and selects the
 * solid (TS_PREFER_EXACT) or the first tessellation (TS_PREFER_TESSELLATED). Every
 * shape root is the selection or an alternative of exactly one choice; alternatives
 * are for a failed selection. Physical records are read; nothing is imported.
 * With TS_DISCOVER_SURFACE_MODELS, every SHELL_BASED_SURFACE_MODEL is reported too,
 * as roots of kind TS_ROOT_SHELL_BASED_SURFACE_MODEL after the tessellations (their
 * count is root_count - solid_count - tessellation_count); they take no part in
 * selection, so solid and tessellation indices and choices are unchanged.
 * Options: flags and max_work (each pass has its own budget); the other fields are
 * validated. Additive ABI 1 records: 48-byte info, 40-byte units, 112-byte root,
 * 120-byte presentation and 32-byte choice. */
#define TS_ROOT_SHELL_BASED_SURFACE_MODEL 7u
typedef struct ts_discovery ts_discovery;
#define TS_PREFER_EXACT 1u
#define TS_PREFER_TESSELLATED 2u
#define TS_PAIRING_NONE 0u
#define TS_PAIRING_LINK 1u
#define TS_PAIRING_RELATIONSHIP 2u
typedef struct ts_discovery_info {
    uint64_t root_count, solid_count, tessellation_count, presentation_count, choice_count;
    uint32_t preference, reserved;
} ts_discovery_info;
/* distance_uncertainty_m is valid when has_distance_uncertainty is 1: the measure
 * named DISTANCE_ACCURACY_VALUE, else the context's smallest length uncertainty. */
typedef struct ts_context_units {
    uint64_t context_id;
    double metres_per_unit, radians_per_unit, distance_uncertainty_m;
    uint32_t has_distance_uncertainty, reserved;
} ts_context_units;
/* Shape roots in order: solids (index below solid_count) in document order, then
 * tessellations, then any surface models. kind is a TS_ROOT_* value. link_id is a tessellation's
 * GEOMETRIC_LINK or TOPOLOGICAL_LINK (zero when absent and for solids). status is
 * TS_OK with units, or the unit failure with zero units and a located error.
 * representations is borrowed from the discovery until its last release, and NULL
 * exactly when representation_count is zero (as for every array below). */
typedef struct ts_discovered_root {
    uint64_t root_id;
    uint32_t kind, status;
    uint64_t link_id;
    const uint64_t *representations;
    size_t representation_count;
    ts_context_units units;
    ts_import_error error;
} ts_discovered_root;
typedef struct ts_discovered_presentation {
    uint64_t occurrence_id;
    uint32_t status, reserved;
    const uint64_t *containers; /* *_CALLOUT and ANNOTATION_PLANE records, nested too */
    size_t container_count;
    const uint64_t *representations;
    size_t representation_count;
    ts_context_units units;
    ts_import_error error;
} ts_discovered_presentation;
/* One shape: shape-root indices for ts_discovery_root_at. pairing is TS_PAIRING_NONE
 * for a shape with a single representation, else how it was recognized (LINK when
 * any member is linked). Solids' choices come first, in solid order. */
typedef struct ts_representation_choice {
    size_t selected;
    uint32_t pairing, reserved;
    const size_t *alternatives;
    size_t alternative_count;
} ts_representation_choice;
/* preference is TS_PREFER_EXACT or TS_PREFER_TESSELLATED. Output/error contract of
 * ts_document_import_tessellated: budget failures return TS_RESOURCE_LIMIT. Success
 * transfers one discovery acquisition, independent of the document, released with
 * ts_discovery_release. */
TS_API ts_status TS_CALL ts_document_discover(const ts_document *document,
    uint32_t preference, const ts_import_options *options, ts_discovery **out,
    ts_import_error *error);
TS_API ts_status TS_CALL ts_discovery_retain(const ts_discovery *discovery);
TS_API void TS_CALL ts_discovery_release(const ts_discovery *discovery);
/* Scalar copies and borrowed arrays; outputs are cleared on failure; out-of-range is
 * TS_NOT_FOUND. Concurrent immutable queries are allowed while an acquisition lives. */
TS_API ts_status TS_CALL ts_discovery_get_info(const ts_discovery *discovery,
    ts_discovery_info *out);
TS_API ts_status TS_CALL ts_discovery_root_at(const ts_discovery *discovery, size_t index,
    ts_discovered_root *out);
TS_API ts_status TS_CALL ts_discovery_presentation_at(const ts_discovery *discovery,
    size_t index, ts_discovered_presentation *out);
TS_API ts_status TS_CALL ts_discovery_choice_at(const ts_discovery *discovery, size_t index,
    ts_representation_choice *out);

/* Surface models (Milestone 22.6): a selected SHELL_BASED_SURFACE_MODEL whose
 * boundary holds OPEN_SHELLs and CLOSED_SHELLs (or ORIENTED_ forms) of curved-profile
 * faces, imported as one mesh per shell, never as a solid: each shell is validated
 * open or closed as declared (closed: every edge used twice), keeps its own copies of
 * shared vertices and edges, and is meshed as one connected component, watertight
 * when closed, with no volume or outward-orientation check. Units, tolerances,
 * strictness and the deviation policy are those of ts_document_import_solid; repair
 * flags are validated and not applied. The first failing shell fails the import.
 * Additive ABI 1 records: 136-byte info revision 1 (extensible) and 48-byte shell. */
typedef struct ts_surface_model ts_surface_model;
#define TS_SURFACE_MODEL_INFO_SIZE_1 136u
typedef struct ts_surface_model_info {
    uint32_t struct_size, abi_version;
    uint64_t root_id, shell_count, closed_shell_count, face_count, boundary_edges;
    uint64_t inferred_outer_bounds, inserted_seams, split_edges, recharted_spheres;
    uint64_t collapsed_edges, collapsed_faces, supplied_pcurves, rejected_pcurves;
    uint64_t off_surface_edges, off_curve_vertices, uncollapsed_sides;
} ts_surface_model_info;
/* One shell in boundary order: the shell entity the model references, closed 1 for a
 * declared CLOSED_SHELL, its STEP faces (borrowed from the model until its last
 * release; NULL exactly when face_count is zero), open boundary edges (zero when
 * closed) and the working tolerance its B-rep was validated with, in metres. */
typedef struct ts_surface_shell {
    uint64_t entity_id;
    uint32_t closed, reserved;
    const uint64_t *faces;
    size_t face_count;
    uint64_t boundary_edges;
    double working_tolerance_m;
} ts_surface_shell;
/* Document, unit, options, output-clearing and error contract of
 * ts_document_import_solid. Success transfers one surface-model acquisition,
 * independent of the document, released with ts_surface_model_release. */
TS_API ts_status TS_CALL ts_document_import_surface_model(const ts_document *document,
    uint64_t entity_id, double metres_per_unit, double radians_per_unit,
    const ts_import_options *options, ts_surface_model **out, ts_import_error *error);
TS_API ts_status TS_CALL ts_surface_model_retain(const ts_surface_model *model);
TS_API void TS_CALL ts_surface_model_release(const ts_surface_model *model);
/* out is required and its caller sets struct_size to a published revision; the rule
 * of ts_solid_info applies. Concurrent immutable queries are allowed. */
TS_API ts_status TS_CALL ts_surface_model_get_info(const ts_surface_model *model,
    ts_surface_model_info *out);
/* Out-of-range indices are TS_NOT_FOUND; outputs are cleared on failure. */
TS_API ts_status TS_CALL ts_surface_model_shell_at(const ts_surface_model *model,
    size_t index, ts_surface_shell *out);
/* An independent acquisition of the shell's mesh (face_ids are STEP face IDs),
 * released with ts_mesh_release; it and its views may outlive the model. */
TS_API ts_status TS_CALL ts_surface_model_shell_mesh(const ts_surface_model *model,
    size_t index, ts_mesh **out);

/* Assembly import with the extensible options (Milestone 22): ts_document_import_assembly
 * with strictness, repair and the deviation policy for every root, the import and
 * tessellation budgets and tolerances of ts_import_options (model_distance_m is the
 * floor under each root's context uncertainty), max_assembly_instances and
 * max_assembly_depth for the product-structure expansion, the representation
 * preference (TS_PREFER_*), and a
 * one-shot import session bounded by the record's session budgets. Output, error
 * and ownership contract of ts_document_import_assembly. */
TS_API ts_status TS_CALL ts_document_import_assembly_with_options(const ts_document *document,
    uint32_t preference, const ts_import_options *options, ts_assembly **out,
    ts_import_error *error);
/* Exporter encodings tolerated (and counted) unless strict: SI units whose dimensions
 * are explicit exponents or $, conversion-based units with * dimensions, bare
 * numbers for typed measures, and complex instances out of canonical order. */
typedef struct ts_tolerated_encodings {
    uint64_t explicit_si_dimensions, unset_si_dimensions, omitted_conversion_dimensions;
    uint64_t untyped_measures, unordered_complex;
} ts_tolerated_encodings;
/* Occurrences placed by a MAPPED_ITEM (or Cartesian transformation operator) of the
 * occurrence's representation rather than a context dependent shape representation,
 * and the encodings tolerated in the product closure. Additive ABI 1 records:
 * 40-byte encodings, 48-byte adaptations. */
typedef struct ts_assembly_adaptations {
    uint64_t mapped_placements;
    ts_tolerated_encodings tolerated;
} ts_assembly_adaptations;
/* Any assembly acquisition; output cleared on failure. */
TS_API ts_status TS_CALL ts_assembly_get_adaptations(const ts_assembly *assembly,
    ts_assembly_adaptations *out);

/* Per-root reports. tolerated counts the encodings tolerated in the root's context
 * units (zero when it has none); message is the root's failure text as the importer
 * words it (the budget and its configured value, a measured distance and its bound,
 * a unit problem), UTF-8 borrowed from the handle until its last release, {NULL, 0}
 * when the root has none. For an assembly: the root's import failure (index as
 * ts_assembly_root_at). For a discovery: the root's unit failure (index as
 * ts_discovery_root_at). Out-of-range is TS_NOT_FOUND; outputs are cleared on
 * failure. Additive ABI 1 record: 56 bytes. */
typedef struct ts_root_report {
    ts_tolerated_encodings tolerated;
    ts_string_view message;
} ts_root_report;
TS_API ts_status TS_CALL ts_assembly_root_report_at(const ts_assembly *assembly,
    size_t index, ts_root_report *out);
TS_API ts_status TS_CALL ts_discovery_root_report_at(const ts_discovery *discovery,
    size_t index, ts_root_report *out);

/* Import sessions (Milestone 22.8). A session shares one document between imports:
 * each discovery pass runs once per session, ts_import_session_prepare decodes the
 * reference closures of several roots together once, and every import adapts its
 * root from that shared decode. Every result equals the corresponding ts_document_*
 * call's with the same options; a shared decode is kept only when it succeeds within
 * one root's max_work, otherwise each root decodes alone. Beside every per-root
 * budget, the session budgets of the options bound what the session accumulates:
 * decoded records, records handed to adapters and triangles; an exhausted budget
 * fails the root with TS_RESOURCE_LIMIT. Size them with
 * ts_import_options_for_document.
 * Ownership: creation retains the document (it may be released immediately) and
 * fixes the options; the session is uniquely owned (no retain) and released once
 * with ts_import_session_release, after every call on it has returned. Results
 * (discoveries, meshes, assemblies) are independent acquisitions that outlive it. A
 * session holds the shared decodes of prepared roots until each is imported.
 * Threads: calls on one session are serialized by the library (safe, not parallel);
 * use one session per thread for parallel imports. After TS_INTERNAL_ERROR (a
 * contained panic) every further call except release fails the same way.
 * Messages: the session keeps the failure text of its most recent import or
 * discovery call (ts_import_session_last_message). This is state of the handle, not
 * global or per-thread state; document-level calls keep none. Additive ABI 1 record:
 * 72-byte statistics. */
typedef struct ts_import_session ts_import_session;
typedef struct ts_session_statistics {
    uint64_t discoveries, shared_decodes, shared_fallbacks, shared_roots, root_decodes;
    uint64_t decoded_entities, adapted_entities, imports, triangles;
} ts_session_statistics;
/* options is copied; NULL means the ts_import_options_init defaults. */
TS_API ts_status TS_CALL ts_import_session_create(const ts_document *document,
    const ts_import_options *options, ts_import_session **out);
TS_API void TS_CALL ts_import_session_release(ts_import_session *session);
/* ts_document_discover with the session's options and cached passes. */
TS_API ts_status TS_CALL ts_import_session_discover(ts_import_session *session,
    uint32_t preference, ts_discovery **out, ts_import_error *error);
/* Decode the closures of these roots together, grouped by the profile their import
 * uses (FACETED_BREP, other solids, tessellations and annotation occurrences); other
 * entities are ignored. *shared is the number of roots a shared decode now serves
 * (zero for groups of fewer than two new roots or a failed shared decode). roots may
 * be NULL only when root_count is zero; zero IDs return TS_INVALID_ARGUMENT. */
TS_API ts_status TS_CALL ts_import_session_prepare(ts_import_session *session,
    const uint64_t *roots, size_t root_count, size_t *shared);
/* ts_document_import_solid with the session's options. */
TS_API ts_status TS_CALL ts_import_session_import_solid(ts_import_session *session,
    uint64_t entity_id, double metres_per_unit, double radians_per_unit, ts_mesh **out,
    ts_solid_info *info, ts_import_error *error);
/* ts_document_import_tessellated with the session's options: max_work, max_records
 * and the strict flag, and max_vertices / max_triangles as mesh limits (validation
 * work bounded by the tessellation work). */
TS_API ts_status TS_CALL ts_import_session_import_tessellated(ts_import_session *session,
    uint64_t entity_id, double metres_per_unit, ts_mesh **out, ts_tessellated_info *info,
    ts_import_error *error);
/* ts_document_import_presentation with the session's options: max_work, max_records
 * and the strict flag, max_vertices, max_triangles, max_polyline_points and
 * max_presentation_depth. */
TS_API ts_status TS_CALL ts_import_session_import_presentation(ts_import_session *session,
    uint64_t entity_id, double metres_per_unit, ts_presentation **out, ts_import_error *error);
/* ts_document_import_surface_model with the session's document and options. Surface
 * models share no decode and are not counted in the session budgets. */
TS_API ts_status TS_CALL ts_import_session_import_surface_model(ts_import_session *session,
    uint64_t entity_id, double metres_per_unit, double radians_per_unit,
    ts_surface_model **out, ts_import_error *error);
/* ts_document_import_assembly_with_options with the session's options and passes. */
TS_API ts_status TS_CALL ts_import_session_import_assembly(ts_import_session *session,
    uint32_t preference, ts_assembly **out, ts_import_error *error);
TS_API ts_status TS_CALL ts_import_session_get_statistics(const ts_import_session *session,
    ts_session_statistics *out);
/* The failure text of the session's most recent discover, prepare or import call:
 * the importer's message for a located failure (for example
 * "schema decoding work budget exceeded (max_work 5000000)"), empty after a success
 * or an argument error. UTF-8, at most capacity - 1 bytes copied and NUL-terminated
 * when capacity
 * is nonzero (buffer may be NULL only when capacity is zero); *length receives the
 * full length without the NUL, so a caller retries with length + 1 bytes when
 * *length >= capacity. length is required. */
TS_API ts_status TS_CALL ts_import_session_last_message(const ts_import_session *session,
    char *buffer, size_t capacity, size_t *length);

#ifdef __cplusplus
}
#endif
#endif
