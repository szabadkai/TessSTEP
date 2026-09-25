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

#ifdef __cplusplus
}
#endif
#endif
