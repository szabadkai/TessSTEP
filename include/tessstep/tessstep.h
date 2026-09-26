#ifndef TESSSTEP_H
#define TESSSTEP_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#define TS_API __declspec(dllimport)
#define TS_CALL __cdecl
#else
#define TS_API
#define TS_CALL
#endif
#ifdef __cplusplus
extern "C" {
#endif

/* ABI 1: C11, 64-bit platforms. All layouts and numeric values below are frozen.
 * Future incompatible records/functions get new names; existing records do not
 * grow. No Rust representation is part of this protocol. */
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

#ifdef __cplusplus
}
#endif
#endif
