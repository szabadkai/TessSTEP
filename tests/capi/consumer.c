#include <tessstep/tessstep.h>
#include <assert.h>
#include <string.h>

/* Frozen ABI 1 layouts on the supported 64-bit targets. Independent of Rust. */
_Static_assert(sizeof(ts_status) == 4, "status width");
_Static_assert(sizeof(ts_parse_options) == 88, "options layout");
_Static_assert(offsetof(ts_parse_options, max_input_bytes) == 8, "options offset");
_Static_assert(offsetof(ts_parse_options, max_sections) == 80, "options tail");
_Static_assert(sizeof(ts_string_view) == 16, "text layout");
_Static_assert(sizeof(ts_document_info) == 24, "document layout");
_Static_assert(sizeof(ts_entity_info) == 16, "entity layout");
_Static_assert(sizeof(ts_diagnostic) == 80, "diagnostic layout");
_Static_assert(offsetof(ts_diagnostic, code) == 48, "diagnostic code offset");
_Static_assert(offsetof(ts_diagnostic, message) == 64, "diagnostic message offset");
#define HEADER "ISO-10303-21;HEADER;FILE_DESCRIPTION(('test'),'2;1');" \
    "FILE_NAME('','',('a'),('o'),'','','');FILE_SCHEMA(('EXAMPLE'));ENDSEC;DATA;"
#define END "ENDSEC;END-ISO-10303-21;"
static const char input[] = HEADER "#9=ITEM(#42);#42=(A()B());" END;
static const char missing[] = HEADER "#9=ITEM(#42);" END;
static const char unsupported[] = HEADER "#1=&SCOPE #2=A(); ENDSCOPE A();" END;
static int text_is(ts_string_view view, const char *expected) {
    return view.size == strlen(expected) && memcmp(view.data, expected, view.size) == 0;
}
int main(void) {
    ts_document *doc = NULL;
    ts_diagnostics *report = NULL;
    ts_parse_options options;
    ts_document_info info;
    ts_entity_info entity;
    ts_string_view name, again;
    ts_diagnostic diagnostic;
    size_t count = 99;
    assert(ts_api_version() == TS_ABI_VERSION);
    assert(ts_parse_options_init(NULL) == TS_INVALID_ARGUMENT);
    assert(ts_parse_options_init(&options) == TS_OK);
    assert(options.struct_size == sizeof(options) && options.abi_version == TS_ABI_VERSION);
    assert(options.max_input_bytes == 268435456 && options.max_entities == 1000000);
    assert(ts_document_parse((const uint8_t*)input, strlen(input), &options, &doc, &report) == TS_OK);
    assert(doc && !report);
    assert(ts_document_get_info(doc, &info) == TS_OK && info.entity_count == 2 && info.header_count == 3 && info.data_section_count == 1);
    assert(ts_document_entity_at(doc, 0, &entity) == TS_OK && entity.id == 9 && entity.record_count == 1);
    assert(ts_document_entity_at(doc, 1, &entity) == TS_OK && entity.id == 42 && entity.record_count == 2);
    assert(ts_document_entity_at(doc, 2, &entity) == TS_NOT_FOUND && entity.id == 0 && entity.record_count == 0);
    assert(ts_document_record_name(doc, 42, 1, &name) == TS_OK && text_is(name, "B"));
    assert(ts_document_retain(doc) == TS_OK);
    ts_document_release(doc);
    assert(ts_document_record_name(doc, 42, 1, &again) == TS_OK && again.data == name.data);
    assert(ts_document_record_name(doc, 0, 0, &again) == TS_NOT_FOUND && !again.data && !again.size);
    assert(ts_document_record_name(doc, 42, 2, &again) == TS_NOT_FOUND);
    assert(ts_document_diagnostics(doc, &report) == TS_OK);
    ts_document_release(doc);
    doc = NULL;
    assert(ts_diagnostics_count(report, &count) == TS_OK && count == 0);
    assert(ts_diagnostics_get(report, 0, &diagnostic) == TS_NOT_FOUND && !diagnostic.message.data);
    ts_diagnostics_release(report);
    report = NULL;

    assert(ts_document_parse((const uint8_t*)missing, strlen(missing), NULL, &doc, &report) == TS_OK);
    assert(ts_document_diagnostics(doc, &report) == TS_OK);
    ts_document_release(doc);
    doc = NULL;
    assert(ts_diagnostics_count(report, &count) == TS_OK && count == 1);
    assert(ts_diagnostics_get(report, 0, &diagnostic) == TS_OK);
    assert(text_is(diagnostic.code, "TS1103") && diagnostic.entity_id == 9 && diagnostic.severity == TS_SEVERITY_ERROR);
    assert(diagnostic.end_offset > diagnostic.start_offset && diagnostic.line == 1 && diagnostic.column > 0 && diagnostic.reserved == 0);
    ts_diagnostics_release(report);
    report = NULL;

    assert(ts_document_parse(NULL, 0, NULL, &doc, &report) == TS_PARSE_ERROR && !doc && report);
    assert(ts_diagnostics_get(report, 0, &diagnostic) == TS_OK && diagnostic.message.size);
    ts_diagnostics_release(report);
    report = NULL;
    assert(ts_document_parse((const uint8_t*)unsupported, strlen(unsupported), NULL, &doc, &report) == TS_UNSUPPORTED && !doc && report);
    ts_diagnostics_release(report);
    report = NULL;
    options.max_entities = 0;
    assert(ts_document_parse((const uint8_t*)input, strlen(input), &options, &doc, &report) == TS_RESOURCE_LIMIT && !doc && report);
    assert(ts_diagnostics_get(report, 0, &diagnostic) == TS_OK && text_is(diagnostic.code, "TS1201"));
    ts_diagnostics_release(report);
    report = NULL;
    options.abi_version = 99;
    assert(ts_document_parse(NULL, 0, &options, &doc, &report) == TS_INVALID_ARGUMENT && !doc && !report);
    ts_parse_options_init(&options);
    options.struct_size--;
    assert(ts_document_parse(NULL, 0, &options, &doc, &report) == TS_INVALID_ARGUMENT);
    assert(ts_document_parse(NULL, 1, NULL, &doc, &report) == TS_INVALID_ARGUMENT && !doc && !report);
    assert(ts_document_parse(NULL, 0, NULL, NULL, &report) == TS_INVALID_ARGUMENT && !report);
    assert(ts_document_parse(NULL, 0, NULL, &doc, NULL) == TS_INVALID_ARGUMENT && !doc);
    assert(ts_document_get_info(NULL, &info) == TS_INVALID_ARGUMENT && info.entity_count == 0);
    assert(ts_document_get_info(NULL, NULL) == TS_INVALID_ARGUMENT);
    assert(ts_document_entity_at(NULL, 0, &entity) == TS_INVALID_ARGUMENT && entity.id == 0);
    assert(ts_document_record_name(NULL, 1, 0, &name) == TS_INVALID_ARGUMENT && !name.data);
    assert(ts_document_diagnostics(NULL, &report) == TS_INVALID_ARGUMENT && !report);
    assert(ts_diagnostics_count(NULL, &count) == TS_INVALID_ARGUMENT && count == 0);
    assert(ts_diagnostics_get(NULL, 0, &diagnostic) == TS_INVALID_ARGUMENT && !diagnostic.code.data);
    assert(ts_document_retain(NULL) == TS_INVALID_ARGUMENT);
    ts_document_release(NULL);
    ts_diagnostics_release(NULL);
    return 0;
}
