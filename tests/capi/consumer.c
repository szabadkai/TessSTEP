#include <tessstep/tessstep.h>
#include <assert.h>
#include <string.h>
#include <stdio.h>

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
/* Two exporter slips: string backslashes that start no directive, and $ for the
 * FILE_NAME authorisation. */
static const char slips[] = "ISO-10303-21;HEADER;FILE_DESCRIPTION(('slips'),'2;1');"
    "FILE_NAME('C:\\parts\\bracket.stp','',('a'),('o'),'','',$);FILE_SCHEMA(('EXAMPLE'));"
    "ENDSEC;DATA;#1=ITEM('Alloys\\Steel');" END;
static int text_is(ts_string_view view, const char *expected) {
    return view.size == strlen(expected) && memcmp(view.data, expected, view.size) == 0;
}
/* ABI 1.1: input-sized budgets, the parse conformance policy and deviation counts. */
static void parse_policy_contract(void) {
    ts_parse_options defaults, sized;
    assert(ts_parse_options_init(&defaults) == TS_OK);
    assert(ts_parse_options_for_input(1, NULL) == TS_INVALID_ARGUMENT);
    assert(ts_parse_options_for_input(0, &sized) == TS_OK);
    assert(memcmp(&sized, &defaults, sizeof sized) == 0);
    assert(ts_parse_options_for_input(700000000u, &sized) == TS_OK);
    assert(sized.struct_size == sizeof sized && sized.abi_version == TS_ABI_VERSION);
    assert(sized.max_input_bytes == 700000000u && sized.max_entities == 100000000u);
    assert(sized.max_records == 233333333u && sized.max_total_values == 350000000u);
    assert(sized.max_aggregate_elements == 350000000u && sized.max_symbols == defaults.max_symbols);
    assert(sized.max_nesting_depth == defaults.max_nesting_depth && sized.max_sections == defaults.max_sections);
    assert(sized.max_token_bytes == defaults.max_token_bytes && sized.max_string_bytes == defaults.max_string_bytes);

    const uint8_t *bytes = (const uint8_t *)slips;
    size_t size = strlen(slips);
    ts_document *doc = NULL;
    ts_diagnostics *report = NULL;
    ts_diagnostic diagnostic;
    size_t count = 0;
    uint64_t deviations = 99;
    /* The ABI 1.0 entry point stays strict. */
    assert(ts_document_parse(bytes, size, NULL, &doc, &report) == TS_PARSE_ERROR && !doc && report);
    assert(ts_diagnostics_get(report, 0, &diagnostic) == TS_OK && text_is(diagnostic.code, "TS1006"));
    ts_diagnostics_release(report);
    report = NULL;
    ts_import_policy policy;
    assert(ts_import_policy_init(&policy) == TS_OK);
    policy.flags = TS_IMPORT_STRICT;
    assert(ts_document_parse_with_policy(bytes, size, &sized, &policy, &doc, &report) == TS_PARSE_ERROR && !doc && report);
    ts_diagnostics_release(report);
    report = NULL;
    policy.flags = 2u;
    doc = (ts_document *)&policy;
    report = (ts_diagnostics *)&policy;
    assert(ts_document_parse_with_policy(bytes, size, NULL, &policy, &doc, &report) == TS_INVALID_ARGUMENT && !doc && !report);
    policy.flags = 0;
    assert(ts_document_parse_with_policy(bytes, size, &sized, &policy, &doc, &report) == TS_OK && doc && !report);
    ts_document_release(doc);
    doc = NULL;
    assert(ts_document_parse_with_policy(bytes, size, NULL, NULL, &doc, &report) == TS_OK && doc && !report);
    assert(ts_document_deviation_count(doc, TS_DEVIATION_LITERAL_BACKSLASH, &deviations) == TS_OK && deviations == 3);
    assert(ts_document_deviation_count(doc, TS_DEVIATION_NULL_HEADER_STRING, &deviations) == TS_OK && deviations == 1);
    assert(ts_document_deviation_count(doc, 99u, &deviations) == TS_NOT_FOUND && deviations == 0);
    assert(ts_document_deviation_count(NULL, TS_DEVIATION_LITERAL_BACKSLASH, &deviations) == TS_INVALID_ARGUMENT);
    assert(ts_document_deviation_count(doc, TS_DEVIATION_LITERAL_BACKSLASH, NULL) == TS_INVALID_ARGUMENT);
    /* One warning per accepted kind, at its first occurrence. */
    assert(ts_document_diagnostics(doc, &report) == TS_OK);
    assert(ts_diagnostics_count(report, &count) == TS_OK && count == 2);
    assert(ts_diagnostics_get(report, 0, &diagnostic) == TS_OK && text_is(diagnostic.code, "TS1006"));
    assert(diagnostic.severity == TS_SEVERITY_WARNING && diagnostic.end_offset == diagnostic.start_offset + 1);
    assert(bytes[diagnostic.start_offset] == '\\');
    assert(ts_diagnostics_get(report, 1, &diagnostic) == TS_OK && text_is(diagnostic.code, "TS1011"));
    assert(diagnostic.severity == TS_SEVERITY_WARNING && bytes[diagnostic.start_offset] == '$');
    ts_diagnostics_release(report);
    report = NULL;
    ts_document_release(doc);
    doc = NULL;
    /* A strict parse has no deviations. */
    assert(ts_document_parse((const uint8_t *)input, strlen(input), NULL, &doc, &report) == TS_OK);
    assert(ts_document_deviation_count(doc, TS_DEVIATION_LITERAL_BACKSLASH, &deviations) == TS_OK && deviations == 0);
    ts_document_release(doc);
}
static void mesh_contract(void) {
    _Static_assert(sizeof(ts_mesh_options) == 40, "mesh options layout");
    _Static_assert(sizeof(ts_mesh_view) == 80, "mesh view layout");
    _Static_assert(offsetof(ts_mesh_view, face_ids) == 48, "mesh face offset");
    _Static_assert(offsetof(ts_mesh_view, signed_volume) == 72, "mesh volume offset");
    double xyz[] = {0,0,0, 1,0,0, 0,1,0, 0,0,1};
    const uint32_t triangles[] = {0,2,1, 0,1,3, 1,2,3, 2,0,3};
    ts_mesh *mesh = NULL;
    ts_mesh_options options;
    ts_mesh_view view, again;
    assert(ts_mesh_options_init(&options) == TS_OK);
    assert(options.struct_size == 40 && options.abi_version == 1);
    options.require_solid = 1;
    assert(ts_mesh_create(xyz,4,triangles,4,&options,&mesh) == TS_OK);
    assert(ts_mesh_get_view(mesh,&view) == TS_OK);
    assert(view.vertex_count == 4 && view.triangle_count == 4 && view.boundary_edges == 0 && view.components == 1);
    assert(view.signed_volume > 0.1666 && view.signed_volume < 0.1667);
    xyz[0] = 8;
    assert(view.positions[0] == 0 && view.triangles[1] == 2 && view.normals[2] == -1 && view.uvs[0] == 0 && view.face_ids[0] == 0);
    assert(ts_mesh_retain(mesh) == TS_OK);
    ts_mesh_release(mesh);
    assert(ts_mesh_get_view(mesh,&again) == TS_OK && again.positions == view.positions);
    ts_mesh_release(mesh);
    assert(ts_mesh_get_view(NULL,&view) == TS_INVALID_ARGUMENT && !view.positions && view.vertex_count == 0);
    xyz[0] = 0;
    assert(ts_mesh_create(xyz,4,triangles,3,&options,&mesh) == TS_INVALID_MESH && !mesh);
    options.max_vertices = 0;
    assert(ts_mesh_create(xyz,4,triangles,4,&options,&mesh) == TS_RESOURCE_LIMIT && !mesh);
    assert(ts_mesh_retain(NULL) == TS_INVALID_ARGUMENT);
    ts_mesh_release(NULL);
}
static void scene_contract(void) {
    _Static_assert(sizeof(ts_scene_options) == 40, "scene options layout");
    _Static_assert(sizeof(ts_scene_asset) == 16, "asset layout");
    _Static_assert(sizeof(ts_scene_instance) == 120, "instance layout");
    _Static_assert(offsetof(ts_scene_instance, linear) == 24, "local linear offset");
    _Static_assert(sizeof(ts_scene_instance_info) == 232, "instance info layout");
    _Static_assert(offsetof(ts_scene_instance_info, world_linear) == 120, "world linear offset");
    _Static_assert(offsetof(ts_scene_instance_info, mirrored) == 224, "mirrored offset");
    _Static_assert(sizeof(ts_scene_info) == 16, "scene info layout");
    const double xyz[] = {0,0,0, 1,0,0, 0,1,0};
    const uint32_t tri[] = {0,1,2};
    ts_mesh *mesh = NULL, *acquired = NULL, *baked = NULL;
    ts_scene *scene = NULL;
    ts_scene_asset asset;
    ts_mesh_view original, view;
    ts_scene_info info;
    ts_scene_instance_info instance;
    ts_scene_options options;
    ts_scene_instance nodes[] = {
        {7,42,9, {-2,0,0, 0,3,0, 0,0,4}, {1,0,0}},
        {42,0,0, {1,0,0, 0,1,0, 0,0,1}, {10,0,0}}
    };
    assert(ts_mesh_create(xyz,3,tri,1,NULL,&mesh)==TS_OK);
    assert(ts_mesh_get_view(mesh,&original)==TS_OK);
    asset.id=9; asset.mesh=mesh;
    assert(ts_scene_options_init(&options)==TS_OK && options.struct_size==sizeof(options));
    assert(ts_scene_options_init(NULL)==TS_INVALID_ARGUMENT);
    assert(ts_scene_create(&asset,1,nodes,2,&options,&scene)==TS_OK);
    ts_mesh_release(mesh); mesh=NULL;
    assert(ts_scene_retain(scene)==TS_OK); ts_scene_release(scene);
    assert(ts_scene_get_info(scene,&info)==TS_OK && info.asset_count==1 && info.instance_count==2);
    assert(ts_scene_instance_at(scene,0,&instance)==TS_OK && instance.source.id==7);
    assert(instance.depth==2 && instance.mirrored==1 && instance.world_translation[0]==11);
    assert(ts_scene_instance_at(scene,2,&instance)==TS_NOT_FOUND && instance.source.id==0);
    assert(ts_scene_asset_mesh(scene,9,&acquired)==TS_OK);
    assert(ts_mesh_get_view(acquired,&view)==TS_OK && view.positions==original.positions);
    assert(ts_scene_bake_instance(scene,7,NULL,&baked)==TS_OK);
    assert(ts_mesh_get_view(baked,&view)==TS_OK && view.positions[0]==11 && view.positions[3]==9);
    assert(view.triangles[1]==2 && view.normals[2]==1);
    ts_mesh_release(baked); baked=NULL;
    assert(ts_scene_bake_instance(scene,42,NULL,&baked)==TS_NOT_FOUND && !baked);
    assert(ts_scene_asset_mesh(scene,88,&baked)==TS_NOT_FOUND && !baked);
    ts_scene_release(scene); scene=NULL;
    assert(ts_mesh_get_view(acquired,&view)==TS_OK && view.positions==original.positions);
    asset.mesh=acquired;
    nodes[1].parent_id=7;
    assert(ts_scene_create(&asset,1,nodes,2,NULL,&scene)==TS_INVALID_SCENE && !scene);
    nodes[1].parent_id=0;
    options.max_instances=0;
    assert(ts_scene_create(&asset,1,nodes,2,&options,&scene)==TS_RESOURCE_LIMIT && !scene);
    assert(ts_scene_create(NULL,1,nodes,2,NULL,&scene)==TS_INVALID_ARGUMENT && !scene);
    assert(ts_scene_get_info(NULL,&info)==TS_INVALID_ARGUMENT && info.asset_count==0);
    assert(ts_scene_retain(NULL)==TS_INVALID_ARGUMENT);
    ts_scene_release(NULL);
    ts_mesh_release(acquired);
}
static void appearance_contract(void) {
    _Static_assert(sizeof(ts_appearance_options)==32,"appearance options layout");
    _Static_assert(sizeof(ts_material)==40,"material layout");
    _Static_assert(sizeof(ts_style_target)==24,"style target layout");
    _Static_assert(sizeof(ts_style_binding)==32,"binding layout");
    _Static_assert(sizeof(ts_resolved_material)==64,"resolved layout");
    _Static_assert(offsetof(ts_resolved_material,source)==40,"resolved source offset");
    _Static_assert(sizeof(ts_appearance_info)==16,"appearance info layout");
    const double xyz[]={0,0,0,1,0,0,0,1,0}; const uint32_t tri[]={0,1,2};
    ts_mesh *mesh=NULL; ts_scene *scene=NULL,*retained=NULL; ts_appearance *appearance=NULL,*bad=NULL;
    ts_mesh_view original,view; ts_resolved_material resolved; ts_appearance_info info;
    ts_appearance_options options; ts_style_binding copied; ts_material color;
    ts_material materials[]={{1,{1,0,0,1}},{2,{0,1,0,1}},{3,{0,0,1,0}}};
    ts_style_binding bindings[]={{{TS_STYLE_ASSET,0,1,0},1},
        {{TS_STYLE_INSTANCE,0,42,0},2},{{TS_STYLE_INSTANCE_FACE,0,7,0},3}};
    ts_scene_instance nodes[]={
        {7,42,1,{1,0,0,0,1,0,0,0,1},{0,0,0}},
        {8,42,1,{1,0,0,0,1,0,0,0,1},{0,0,0}},
        {9,0,1,{1,0,0,0,1,0,0,0,1},{0,0,0}},
        {10,0,2,{1,0,0,0,1,0,0,0,1},{0,0,0}},
        {42,0,0,{1,0,0,0,1,0,0,0,1},{0,0,0}}};
    assert(ts_mesh_create(xyz,3,tri,1,NULL,&mesh)==TS_OK);
    assert(ts_mesh_get_view(mesh,&original)==TS_OK);
    ts_scene_asset assets[]={{1,mesh},{2,mesh}};
    assert(ts_scene_create(assets,2,nodes,5,NULL,&scene)==TS_OK);
    ts_mesh_release(mesh); mesh=NULL;
    assert(ts_appearance_options_init(&options)==TS_OK && options.struct_size==sizeof(options));
    assert(ts_appearance_options_init(NULL)==TS_INVALID_ARGUMENT);
    assert(ts_appearance_create(scene,materials,3,bindings,3,&options,&appearance)==TS_OK);
    ts_scene_release(scene);
    assert(ts_appearance_retain(appearance)==TS_OK);ts_appearance_release(appearance);
    materials[2].rgba[3]=1;
    assert(ts_appearance_resolve_triangle(appearance,7,0,&resolved)==TS_OK && resolved.material.id==3);
    assert(resolved.material.rgba[3]==0 && resolved.source.kind==TS_STYLE_INSTANCE_FACE && resolved.source.id==7);
    assert(ts_appearance_resolve_triangle(appearance,8,0,&resolved)==TS_OK && resolved.material.id==2);
    assert(resolved.source.kind==TS_STYLE_INSTANCE && resolved.source.id==42);
    assert(ts_appearance_resolve_triangle(appearance,9,0,&resolved)==TS_OK && resolved.material.id==1);
    assert(ts_appearance_resolve_triangle(appearance,10,0,&resolved)==TS_OK && resolved.material.id==0 && resolved.source.kind==0);
    assert(ts_appearance_resolve_triangle(appearance,42,0,&resolved)==TS_NOT_FOUND && resolved.material.id==0);
    assert(ts_appearance_resolve_triangle(appearance,7,1,&resolved)==TS_NOT_FOUND && resolved.source.id==0);
    assert(ts_appearance_get_info(appearance,&info)==TS_OK && info.material_count==3 && info.binding_count==3);
    assert(ts_appearance_material_at(appearance,2,&color)==TS_OK && color.id==3 && color.rgba[3]==0);
    assert(ts_appearance_binding_at(appearance,2,&copied)==TS_OK && copied.material_id==3);
    assert(ts_appearance_material_at(appearance,3,&color)==TS_NOT_FOUND && color.id==0);
    assert(ts_appearance_binding_at(appearance,3,&copied)==TS_NOT_FOUND && copied.material_id==0);
    assert(ts_appearance_get_scene(appearance,&retained)==TS_OK && retained==scene);
    ts_appearance_release(appearance); appearance=NULL;
    assert(ts_scene_asset_mesh(retained,1,&mesh)==TS_OK);
    assert(ts_mesh_get_view(mesh,&view)==TS_OK && view.positions==original.positions);
    ts_mesh_release(mesh);
    materials[0].rgba[0]=2;
    assert(ts_appearance_create(retained,materials,3,bindings,3,NULL,&bad)==TS_INVALID_APPEARANCE && !bad);
    materials[0].rgba[0]=1;bindings[0].target.reserved=1;
    assert(ts_appearance_create(retained,materials,3,bindings,3,NULL,&bad)==TS_INVALID_ARGUMENT && !bad);
    bindings[0].target.reserved=0;options.max_work=0;
    assert(ts_appearance_create(retained,materials,3,bindings,3,&options,&bad)==TS_RESOURCE_LIMIT && !bad);
    assert(ts_appearance_create(retained,NULL,1,NULL,0,NULL,&bad)==TS_INVALID_ARGUMENT && !bad);
    assert(ts_appearance_get_info(NULL,&info)==TS_INVALID_ARGUMENT && info.material_count==0);
    assert(ts_appearance_get_scene(NULL,&scene)==TS_INVALID_ARGUMENT && !scene);
    assert(ts_appearance_retain(NULL)==TS_INVALID_ARGUMENT);ts_appearance_release(NULL);
    ts_scene_release(retained);
}
static void solid_import_contract(int planar) {
    _Static_assert(sizeof(ts_faceted_options) == 80, "faceted options layout");
    _Static_assert(offsetof(ts_faceted_options, max_work) == 48, "faceted budgets offset");
    _Static_assert(sizeof(ts_import_error) == 32, "import error layout");
    ts_status (TS_CALL *convert)(const ts_document*, uint64_t, double, const ts_faceted_options*, ts_mesh**, ts_import_error*) =
        planar ? ts_document_tessellate_planar : ts_document_tessellate_faceted;
    FILE *file = fopen(planar ? "planar.step" : "faceted.step", "rb");
    assert(file);
    uint8_t bytes[16384];
    size_t count = fread(bytes,1,sizeof(bytes),file);
    assert(!ferror(file) && count < sizeof(bytes));
    fclose(file);
    ts_document *doc = NULL;
    ts_diagnostics *report = NULL;
    assert(ts_document_parse(bytes,count,NULL,&doc,&report) == TS_OK);
    ts_diagnostics_release(report);
    ts_faceted_options options;
    assert((planar ? ts_planar_options_init(&options) : ts_faceted_options_init(&options)) == TS_OK);
    ts_mesh *mesh = NULL;
    ts_import_error error;
    assert(convert(doc,1000,0.001,&options,&mesh,&error) == TS_OK);
    assert(error.stage == TS_IMPORT_STAGE_NONE);
    ts_mesh_view view;
    assert(ts_mesh_get_view(mesh,&view) == TS_OK);
    assert(view.vertex_count == 8 && view.triangle_count == 12 && view.boundary_edges == 0);
    assert(view.signed_volume > 0.000005999 && view.signed_volume < 0.000006001);
    assert(view.face_ids[0] == 106);
    ts_mesh *failed = mesh;
    options.max_work = 0;
    assert(convert(doc,1000,0.001,&options,&failed,&error) == TS_RESOURCE_LIMIT);
    assert(failed == NULL && error.stage == TS_IMPORT_STAGE_PROFILE);
    assert(convert(doc,1000,0.,NULL,&failed,&error) == TS_INVALID_ARGUMENT);
    assert(failed == NULL && error.stage == TS_IMPORT_STAGE_NONE);
    assert(convert(doc,1,0.001,NULL,&failed,&error) == TS_UNSUPPORTED);
    assert(error.entity_id == 1 && error.end_offset > error.start_offset);
    assert(convert(doc,9999,0.001,NULL,&failed,NULL) == TS_NOT_FOUND);
    assert(convert(NULL,1000,0.001,NULL,&failed,&error) == TS_INVALID_ARGUMENT);
    ts_document_release(doc);
    assert(ts_mesh_get_view(mesh,&view) == TS_OK && view.vertex_count == 8);
    ts_mesh_release(mesh);
}
static void import_policy_contract(void) {
    _Static_assert(sizeof(ts_import_policy) == 16, "import policy layout");
    FILE *file = fopen("placeholder.step", "rb");
    assert(file);
    uint8_t bytes[16384];
    size_t count = fread(bytes,1,sizeof(bytes),file);
    assert(!ferror(file) && count < sizeof(bytes));
    fclose(file);
    ts_document *doc = NULL;
    ts_diagnostics *report = NULL;
    assert(ts_document_parse(bytes,count,NULL,&doc,&report) == TS_OK);
    ts_diagnostics_release(report);
    ts_import_policy policy;
    assert(ts_import_policy_init(NULL) == TS_INVALID_ARGUMENT);
    assert(ts_import_policy_init(&policy) == TS_OK);
    assert(policy.struct_size == sizeof(policy) && policy.abi_version == TS_ABI_VERSION && policy.flags == 0);
    ts_mesh *mesh = NULL, *failed = NULL;
    ts_import_error error;
    assert(ts_document_tessellate_planar(doc,1000,0.001,NULL,&mesh,&error) == TS_OK);
    ts_mesh_release(mesh);
    assert(ts_document_tessellate_planar_with_policy(doc,1000,0.001,NULL,&policy,&mesh,&error) == TS_OK);
    policy.flags = TS_IMPORT_STRICT;
    failed = mesh;
    assert(ts_document_tessellate_planar_with_policy(doc,1000,0.001,NULL,&policy,&failed,&error) == TS_INVALID_GEOMETRY);
    assert(failed == NULL && error.stage == TS_IMPORT_STAGE_PROFILE && error.entity_id != 0);
    policy.flags = 2u;
    failed = mesh;
    assert(ts_document_tessellate_planar_with_policy(doc,1000,0.001,NULL,&policy,&failed,&error) == TS_INVALID_ARGUMENT);
    assert(failed == NULL && error.stage == TS_IMPORT_STAGE_NONE);
    ts_document_release(doc);
    ts_mesh_view view;
    assert(ts_mesh_get_view(mesh,&view) == TS_OK && view.vertex_count == 8);
    ts_mesh_release(mesh);
}
static ts_document *load(const char *name) {
    static uint8_t bytes[16384];
    FILE *file = fopen(name, "rb");
    assert(file);
    size_t count = fread(bytes,1,sizeof(bytes),file);
    assert(!ferror(file) && count < sizeof(bytes));
    fclose(file);
    ts_document *doc = NULL;
    ts_diagnostics *report = NULL;
    assert(ts_document_parse(bytes,count,NULL,&doc,&report) == TS_OK);
    ts_diagnostics_release(report);
    return doc;
}
static void tessellated_contract(void) {
    _Static_assert(sizeof(ts_tessellated_options) == 48, "tessellated options layout");
    _Static_assert(offsetof(ts_tessellated_options, flags) == 40, "tessellated flags offset");
    _Static_assert(sizeof(ts_tessellated_info) == 80, "tessellated info layout");
    _Static_assert(offsetof(ts_tessellated_info, pnmax_deviations) == 72, "tessellated info tail");
    ts_document *doc = load("tessellated.step");
    ts_tessellated_options options;
    assert(ts_tessellated_options_init(NULL) == TS_INVALID_ARGUMENT);
    assert(ts_tessellated_options_init(&options) == TS_OK);
    assert(options.struct_size == sizeof(options) && options.abi_version == TS_ABI_VERSION && options.flags == 0);
    ts_mesh *mesh = NULL, *failed = NULL;
    ts_tessellated_info info;
    ts_import_error error;
    assert(ts_document_import_tessellated(doc,1000,0.001,&options,&mesh,&info,&error) == TS_OK);
    assert(info.kind == TS_TESSELLATED_SOLID && info.root_id == 1000 && info.link_id == 0);
    assert(info.face_count == 6 && info.edge_count == 13 && info.vertex_count == 8 && info.joined_points == 24);
    assert(error.stage == TS_IMPORT_STAGE_NONE);
    options.max_records = 3;
    failed = mesh;
    assert(ts_document_import_tessellated(doc,1000,0.001,&options,&failed,&info,&error) == TS_RESOURCE_LIMIT);
    assert(!failed && info.kind == 0 && error.stage == TS_IMPORT_STAGE_GEOMETRY);
    assert(ts_tessellated_options_init(&options) == TS_OK);
    options.flags = 2u;
    assert(ts_document_import_tessellated(doc,1000,0.001,&options,&failed,NULL,NULL) == TS_INVALID_ARGUMENT && !failed);
    assert(ts_document_import_tessellated(doc,10,0.001,NULL,&failed,&info,&error) == TS_UNSUPPORTED);
    assert(!failed && error.entity_id == 10 && error.stage == TS_IMPORT_STAGE_GEOMETRY);
    assert(ts_document_import_tessellated(NULL,1000,0.001,NULL,&failed,&info,&error) == TS_INVALID_ARGUMENT);
    ts_document_release(doc);
    ts_mesh_view view;
    assert(ts_mesh_get_view(mesh,&view) == TS_OK);
    assert(view.vertex_count == 8 && view.triangle_count == 12 && view.boundary_edges == 0 && view.components == 1);
    assert(view.signed_volume > 0.000005999 && view.signed_volume < 0.000006001);
    assert(view.face_ids[0] == 100);
    ts_mesh_release(mesh);
}
static void assembly_contract(void) {
    _Static_assert(sizeof(ts_schema_info) == 32, "schema info layout");
    _Static_assert(sizeof(ts_assembly_options) == 72, "assembly options layout");
    _Static_assert(offsetof(ts_assembly_options, flags) == 64, "assembly flags offset");
    _Static_assert(sizeof(ts_assembly_info) == 72, "assembly info layout");
    _Static_assert(sizeof(ts_assembly_node) == 40, "assembly node layout");
    _Static_assert(sizeof(ts_assembly_product) == 48, "assembly product layout");
    _Static_assert(sizeof(ts_assembly_root) == 56, "assembly root layout");
    _Static_assert(offsetof(ts_assembly_root, error) == 24, "assembly root error offset");
    _Static_assert(sizeof(ts_assembly_exclusion) == 48, "assembly exclusion layout");
    _Static_assert(sizeof(ts_style_info) == 96, "style info layout");
    ts_document *doc = load("assembly.step");
    ts_schema_info schema;
    assert(ts_document_get_schema(doc,&schema) == TS_OK && schema.protocol == TS_PROTOCOL_AP214);
    assert(schema.schema_count == 1 && schema.declared.size > 17);
    ts_assembly_options options;
    assert(ts_assembly_options_init(&options) == TS_OK && options.struct_size == sizeof(options));
    ts_assembly *assembly = NULL, *failed = NULL;
    ts_import_error error;
    assert(ts_document_import_assembly(doc,&options,&assembly,&error) == TS_OK);
    options.chord_m = 0;
    assert(ts_document_import_assembly(doc,&options,&failed,&error) == TS_INVALID_ARGUMENT && !failed);
    ts_document_release(doc);
    ts_assembly_info info;
    assert(ts_assembly_get_info(assembly,&info) == TS_OK);
    assert(info.node_count == 11 && info.product_count == 3 && info.occurrence_count == 4);
    assert(info.root_count == 1 && info.imported_root_count == 1 && info.complete == 1);
    ts_assembly_root root;
    assert(ts_assembly_root_at(assembly,0,&root) == TS_OK && root.kind == TS_ROOT_FACETED_BREP && root.status == TS_OK);
    assert(ts_assembly_root_at(assembly,1,&root) == TS_NOT_FOUND && root.root_id == 0);
    ts_assembly_product product;
    assert(ts_assembly_product_at(assembly,0,&product) == TS_OK && product.name.size > 0);
    size_t leaves = 0;
    for (size_t i = 0; i < info.node_count; ++i) {
        ts_assembly_node node;
        assert(ts_assembly_node_at(assembly,i,&node) == TS_OK);
        leaves += node.root_id != 0;
    }
    assert(leaves == 4);
    ts_scene *scene = NULL;
    assert(ts_assembly_get_scene(assembly,&scene) == TS_OK);
    ts_assembly_release(assembly);
    ts_scene_info scene_info;
    assert(ts_scene_get_info(scene,&scene_info) == TS_OK && scene_info.asset_count == 1 && scene_info.instance_count == 11);
    ts_scene_release(scene);

    doc = load("styled.step");
    assert(ts_document_import_assembly(doc,NULL,&assembly,NULL) == TS_OK);
    ts_document_release(doc);
    ts_style_info style;
    assert(ts_assembly_get_style_info(assembly,&style) == TS_OK);
    assert(style.styled_items == 3 && style.asset_styles == 1 && style.face_styles == 1 && style.layers == 1);
    ts_appearance *appearance = NULL;
    assert(ts_assembly_get_appearance(assembly,&appearance) == TS_OK);
    ts_assembly_release(assembly);
    ts_appearance_info appearance_info;
    assert(ts_appearance_get_info(appearance,&appearance_info) == TS_OK);
    assert(appearance_info.material_count == 2 && appearance_info.binding_count == 2);
    ts_appearance_release(appearance);

    doc = load("missing-placement.step");
    assert(ts_document_import_assembly(doc,NULL,&failed,&error) == TS_INVALID_GEOMETRY && !failed);
    assert(error.stage == TS_IMPORT_STAGE_PRODUCT && error.entity_id != 0);
    ts_document_release(doc);
}
static void presentation_contract(void) {
    _Static_assert(sizeof(ts_presentation_options) == 64, "presentation options layout");
    _Static_assert(sizeof(ts_presentation_info) == 64, "presentation info layout");
    _Static_assert(sizeof(ts_presentation_item) == 24, "presentation item layout");
    _Static_assert(sizeof(ts_presentation_view) == 104, "presentation view layout");
    _Static_assert(offsetof(ts_presentation_view, point_count) == 96, "presentation view tail");
    ts_document *doc = load("presentation.step");
    ts_presentation_options options;
    assert(ts_presentation_options_init(&options) == TS_OK);
    assert(options.struct_size == sizeof(options) && options.max_depth == 16);
    ts_presentation *presentation = NULL, *failed = NULL;
    ts_import_error error;
    assert(ts_document_import_presentation(doc,300,0.001,&options,&presentation,&error) == TS_OK);
    assert(ts_document_import_presentation(doc,110,0.001,NULL,&failed,&error) == TS_UNSUPPORTED);
    assert(!failed && error.entity_id == 110 && error.stage == TS_IMPORT_STAGE_GEOMETRY);
    options.max_depth = 0;
    assert(ts_document_import_presentation(doc,300,0.001,&options,&failed,NULL) == TS_RESOURCE_LIMIT && !failed);
    ts_document_release(doc);
    ts_presentation_info info;
    assert(ts_presentation_get_info(presentation,&info) == TS_OK);
    assert(info.occurrence_id == 300 && info.geometric_set_id == 100 && info.style_count == 1 && info.item_count == 4);
    assert(info.skipped_degenerate == 1 && info.zero_area_triangles == 1);
    uint64_t style = 0;
    assert(ts_presentation_style_at(presentation,0,&style) == TS_OK && style == 203);
    assert(ts_presentation_style_at(presentation,1,&style) == TS_NOT_FOUND && style == 0);
    ts_presentation_item item;
    assert(ts_presentation_item_at(presentation,3,&item) == TS_OK);
    assert(item.entity_id == 150 && item.kind == TS_PRESENTATION_CURVE_SET && item.placements == 2);
    ts_presentation_view view, again;
    assert(ts_presentation_get_view(presentation,&view) == TS_OK);
    assert(view.vertex_count == 12 && view.polyline_count == 3 && view.polyline_point_count == 9);
    assert(view.polyline_offsets[0] == 0 && view.polyline_offsets[3] == 9 && view.polyline_items[2] == 150);
    assert(view.triangle_count == 3 && view.triangle_items[0] == 120 && view.point_count == 1);
    assert(view.positions[0] == 0.1 && view.points[0] < view.vertex_count);
    assert(ts_presentation_retain(presentation) == TS_OK);
    ts_presentation_release(presentation);
    assert(ts_presentation_get_view(presentation,&again) == TS_OK && again.positions == view.positions);
    ts_presentation_release(presentation);
    assert(ts_presentation_get_view(NULL,&view) == TS_INVALID_ARGUMENT && !view.positions && view.vertex_count == 0);
    assert(ts_presentation_retain(NULL) == TS_INVALID_ARGUMENT);
    ts_presentation_release(NULL);
}
static void import_options_contract(void) {
    _Static_assert(sizeof(ts_import_options) == TS_IMPORT_OPTIONS_SIZE_1, "import options revision 1");
    _Static_assert(sizeof(ts_import_options) == 168, "import options layout");
    _Static_assert(offsetof(ts_import_options, max_work) == 16, "import budgets offset");
    _Static_assert(offsetof(ts_import_options, model_distance_m) == 32, "model tolerance offset");
    _Static_assert(offsetof(ts_import_options, max_tessellation_work) == 64, "tessellation budgets offset");
    _Static_assert(offsetof(ts_import_options, max_triangles) == 88, "import options tail");
    ts_import_options options;
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    assert(options.struct_size == sizeof options && options.abi_version == TS_ABI_VERSION && options.flags == 0);
    assert(options.max_work == 5000000 && options.max_records == 100000);
    assert(options.model_distance_m == 1e-7 && options.chord_m == 1e-4 && options.normal_angle_rad == 0.1);
    assert(options.max_tessellation_work == 50000000 && options.max_vertices == 1000000);
    /* Unknown revision sizes write nothing. */
    options.max_work = 7;
    assert(ts_import_options_init(&options, sizeof options - 8) == TS_INVALID_ARGUMENT && options.max_work == 7);
    assert(ts_import_options_init(&options, sizeof options + 8) == TS_INVALID_ARGUMENT && options.max_work == 7);
    assert(ts_import_options_init(NULL, sizeof options) == TS_INVALID_ARGUMENT);
    ts_document *doc = load("cylinder.step");
    assert(ts_import_options_for_document(doc, &options, sizeof options) == TS_OK);
    assert(options.struct_size == sizeof options && options.max_work == 5000000 && options.max_records == 100000);
    ts_document_release(doc);
    assert(ts_import_options_for_document(NULL, &options, sizeof options) == TS_INVALID_ARGUMENT);
    assert(options.struct_size == 0 && options.max_work == 0);
}
static void solid_contract(void) {
    _Static_assert(sizeof(ts_solid_info) == TS_SOLID_INFO_SIZE_1 && TS_SOLID_INFO_SIZE_1 == 216, "solid info revision 1");
    _Static_assert(offsetof(ts_solid_info, root_id) == 16, "solid info root offset");
    _Static_assert(offsetof(ts_solid_info, inferred_outer_bounds) == 40, "solid adaptations offset");
    ts_document *doc = load("cylinder.step");
    ts_import_options options;
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    options.chord_m = 1e-6;
    ts_mesh *mesh = NULL, *failed = NULL;
    ts_solid_info info;
    info.struct_size = (uint32_t)sizeof info;
    ts_import_error error;
    assert(ts_document_import_solid(doc,1000,0.001,1.0,&options,&mesh,&info,&error) == TS_OK);
    assert(error.stage == TS_IMPORT_STAGE_NONE && info.abi_version == TS_ABI_VERSION);
    assert(info.kind == TS_ROOT_MANIFOLD_SOLID_BREP && info.root_id == 1000 && info.face_count == 3);
    assert(info.shell_count == 1 && info.inserted_seams == 1 && info.split_edges == 0);
    options.max_vertices = 0;
    failed = mesh;
    assert(ts_document_import_solid(doc,1000,0.001,1.0,&options,&failed,&info,&error) == TS_RESOURCE_LIMIT);
    assert(!failed && info.struct_size == sizeof info && info.kind == 0 && error.stage == TS_IMPORT_STAGE_TESSELLATION);
    options.max_vertices = 1000000;
    options.flags = 32u;
    assert(ts_document_import_solid(doc,1000,0.001,1.0,&options,&failed,&info,&error) == TS_INVALID_ARGUMENT);
    assert(!failed && error.stage == TS_IMPORT_STAGE_NONE);
    info.struct_size = 8;
    info.root_id = 5;
    assert(ts_document_import_solid(doc,1000,0.001,1.0,NULL,&failed,&info,&error) == TS_INVALID_ARGUMENT);
    assert(!failed && info.root_id == 5);
    assert(ts_document_import_solid(doc,1000,0.,1.0,NULL,&failed,NULL,&error) == TS_INVALID_ARGUMENT && !failed);
    assert(ts_document_import_solid(doc,9999,0.001,1.0,NULL,&failed,NULL,&error) == TS_NOT_FOUND);
    assert(!failed && error.entity_id == 9999);
    assert(ts_document_import_solid(NULL,1000,0.001,1.0,NULL,&failed,NULL,&error) == TS_INVALID_ARGUMENT);
    ts_document_release(doc);
    ts_mesh_view view;
    assert(ts_mesh_get_view(mesh,&view) == TS_OK);
    assert(view.vertex_count == 512 && view.boundary_edges == 0 && view.components == 1);
    const double exact = 3.141592653589793 * 2e-6;
    assert(view.signed_volume > exact * (1 - 2e-4) && view.signed_volume < exact * (1 + 2e-4));
    for (size_t i = 0; i < view.triangle_count; ++i)
        assert(view.face_ids[i] == 28 || view.face_ids[i] == 37 || view.face_ids[i] == 46);
    ts_mesh_release(mesh);

    /* The plane-angle unit scales the 30 degree cone semi-angle. */
    doc = load("cone.step");
    info.struct_size = (uint32_t)sizeof info;
    assert(ts_document_import_solid(doc,1000,0.001,0.017453292519943295,NULL,&mesh,&info,NULL) == TS_OK);
    assert(info.inserted_seams == 1);
    ts_mesh_release(mesh);
    assert(ts_document_import_solid(doc,1000,0.001,1.0,NULL,&failed,NULL,&error) != TS_OK);
    assert(!failed && error.stage == TS_IMPORT_STAGE_GEOMETRY);
    ts_document_release(doc);

    /* A BREP_WITH_VOIDS cavity is a second, inward component. */
    doc = load("cavity.step");
    assert(ts_document_import_solid(doc,1000,0.001,1.0,NULL,&mesh,&info,NULL) == TS_OK);
    assert(info.kind == TS_ROOT_BREP_WITH_VOIDS && info.shell_count == 2);
    ts_document_release(doc);
    assert(ts_mesh_get_view(mesh,&view) == TS_OK && view.components == 2 && view.boundary_edges == 0);
    ts_mesh_release(mesh);
}
static void discovery_contract(void) {
    _Static_assert(sizeof(ts_discovery_info) == 48, "discovery info layout");
    _Static_assert(sizeof(ts_context_units) == 40, "context units layout");
    _Static_assert(sizeof(ts_discovered_root) == 112, "discovered root layout");
    _Static_assert(offsetof(ts_discovered_root, units) == 40, "discovered root units offset");
    _Static_assert(offsetof(ts_discovered_root, error) == 80, "discovered root error offset");
    _Static_assert(sizeof(ts_discovered_presentation) == 120, "discovered presentation layout");
    _Static_assert(offsetof(ts_discovered_presentation, units) == 48, "presentation units offset");
    _Static_assert(sizeof(ts_representation_choice) == 32, "choice layout");
    ts_document *doc = load("selection.step");
    ts_discovery *discovery = NULL, *failed = NULL;
    ts_import_error error;
    assert(ts_document_discover(doc,TS_PREFER_EXACT,NULL,&discovery,&error) == TS_OK);
    ts_discovery_info info;
    assert(ts_discovery_get_info(discovery,&info) == TS_OK);
    assert(info.root_count == 5 && info.solid_count == 2 && info.tessellation_count == 3);
    assert(info.presentation_count == 0 && info.choice_count == 3 && info.preference == TS_PREFER_EXACT);
    const uint64_t ids[] = {1000, 11000, 21000, 31000, 40002};
    const uint32_t kinds[] = {TS_ROOT_MANIFOLD_SOLID_BREP, TS_ROOT_FACETED_BREP, TS_ROOT_TESSELLATED_SOLID,
        TS_ROOT_TESSELLATED_SHELL, TS_ROOT_TESSELLATED_SURFACE_SET};
    ts_discovered_root root;
    for (size_t i = 0; i < info.root_count; ++i) {
        assert(ts_discovery_root_at(discovery,i,&root) == TS_OK);
        assert(root.root_id == ids[i] && root.kind == kinds[i] && root.status == TS_OK);
        assert(root.units.metres_per_unit == 0.001 && root.units.radians_per_unit == 1.0);
        assert(root.units.has_distance_uncertainty == 1 && root.units.distance_uncertainty_m > 0.99e-8);
        assert(root.representation_count == 1 && root.representations[0] != 0);
    }
    assert(ts_discovery_root_at(discovery,2,&root) == TS_OK && root.link_id == 1000);
    assert(ts_discovery_root_at(discovery,5,&root) == TS_NOT_FOUND && !root.representations && root.root_id == 0);
    ts_representation_choice choice;
    assert(ts_discovery_choice_at(discovery,0,&choice) == TS_OK && choice.selected == 0);
    assert(choice.pairing == TS_PAIRING_LINK && choice.alternative_count == 1 && choice.alternatives[0] == 2);
    assert(ts_discovery_choice_at(discovery,1,&choice) == TS_OK && choice.pairing == TS_PAIRING_RELATIONSHIP);
    assert(ts_discovery_choice_at(discovery,2,&choice) == TS_OK && choice.selected == 4);
    assert(choice.pairing == TS_PAIRING_NONE && choice.alternative_count == 0 && !choice.alternatives);
    assert(ts_discovery_choice_at(discovery,3,&choice) == TS_NOT_FOUND && choice.selected == 0);
    /* Import the selected solid and its tessellated alternative with discovered units. */
    ts_mesh *mesh = NULL;
    ts_mesh_view view;
    assert(ts_discovery_root_at(discovery,0,&root) == TS_OK);
    assert(ts_document_import_solid(doc,root.root_id,root.units.metres_per_unit,root.units.radians_per_unit,
        NULL,&mesh,NULL,NULL) == TS_OK);
    assert(ts_mesh_get_view(mesh,&view) == TS_OK && view.signed_volume > 0.000005999 && view.signed_volume < 0.000006001);
    ts_mesh_release(mesh);
    assert(ts_discovery_root_at(discovery,2,&root) == TS_OK);
    assert(ts_document_import_tessellated(doc,root.root_id,root.units.metres_per_unit,NULL,&mesh,NULL,NULL) == TS_OK);
    ts_mesh_release(mesh);
    ts_import_options options;
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    options.max_work = 10;
    failed = discovery;
    assert(ts_document_discover(doc,TS_PREFER_EXACT,&options,&failed,&error) == TS_RESOURCE_LIMIT);
    assert(!failed && error.stage == TS_IMPORT_STAGE_PROFILE);
    assert(ts_document_discover(doc,0,NULL,&failed,&error) == TS_INVALID_ARGUMENT && !failed);
    ts_discovery *tessellated = NULL;
    assert(ts_document_discover(doc,TS_PREFER_TESSELLATED,NULL,&tessellated,NULL) == TS_OK);
    ts_document_release(doc);
    assert(ts_discovery_choice_at(tessellated,0,&choice) == TS_OK && choice.selected == 2);
    assert(choice.alternative_count == 1 && choice.alternatives[0] == 0);
    ts_discovery_release(tessellated);
    /* The discovery and its borrowed arrays outlive the document. */
    assert(ts_discovery_retain(discovery) == TS_OK);
    ts_discovery_release(discovery);
    assert(ts_discovery_root_at(discovery,4,&root) == TS_OK && root.representations[0] != 0);
    ts_discovery_release(discovery);

    doc = load("presentation.step");
    assert(ts_document_discover(doc,TS_PREFER_EXACT,NULL,&discovery,NULL) == TS_OK);
    ts_document_release(doc);
    ts_discovered_presentation presentation;
    assert(ts_discovery_presentation_at(discovery,0,&presentation) == TS_OK);
    assert(presentation.occurrence_id == 300 && presentation.status == TS_OK);
    assert(presentation.container_count == 1 && presentation.containers[0] == 310);
    assert(presentation.representation_count == 2 && presentation.units.metres_per_unit == 0.001);
    assert(ts_discovery_presentation_at(discovery,1,&presentation) == TS_OK);
    assert(presentation.occurrence_id == 301 && presentation.status == TS_UNSUPPORTED);
    assert(presentation.error.stage == TS_IMPORT_STAGE_PROFILE && presentation.error.entity_id == 301);
    assert(!presentation.containers && presentation.units.metres_per_unit == 0);
    ts_discovery_release(discovery);
    assert(ts_discovery_get_info(NULL,&info) == TS_INVALID_ARGUMENT && info.root_count == 0);
    assert(ts_discovery_retain(NULL) == TS_INVALID_ARGUMENT);
    ts_discovery_release(NULL);
}
static ts_status solid_with(const ts_document *doc, const ts_import_options *options,
    ts_solid_info *info, ts_import_error *error, double *volume) {
    ts_mesh *mesh = NULL;
    info->struct_size = (uint32_t)sizeof *info;
    ts_status status = ts_document_import_solid(doc,1000,0.001,1.0,options,&mesh,info,error);
    if (status == TS_OK) {
        ts_mesh_view view;
        assert(ts_mesh_get_view(mesh,&view) == TS_OK);
        *volume = view.signed_volume;
    }
    ts_mesh_release(mesh);
    return status;
}
static void repair_and_deviation_contract(void) {
    _Static_assert(offsetof(ts_import_options, sewing_factor) == 96, "options repair offset");
    _Static_assert(offsetof(ts_import_options, max_session_triangles) == 144, "options session offset");
    _Static_assert(offsetof(ts_import_options, max_presentation_depth) == 160, "options tail");
    _Static_assert(offsetof(ts_solid_info, off_surface_edges) == 104, "solid policy offset");
    _Static_assert(offsetof(ts_solid_info, sewing_tolerance_m) == 208, "solid info tail");
    ts_import_options options, draft;
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    assert(options.sewing_factor == 10 && options.deviation_factor == 16);
    assert(options.max_assembly_instances == 100000 && options.max_session_triangles == 20000000);
    assert(options.max_polyline_points == 4000000 && options.max_presentation_depth == 16);
    /* Sizes of unreleased drafts are not revisions. */
    memset(&draft, 0xff, sizeof draft);
    assert(ts_import_options_init(&draft, 96) == TS_INVALID_ARGUMENT && draft.max_work == UINT64_MAX);
    assert(ts_import_options_init(&draft, 152) == TS_INVALID_ARGUMENT);
    ts_solid_info info;
    ts_import_error error;
    double volume = 0;
    ts_document *doc = load("duplicated.step");
    /* Topology defects are rejected unless repair is asked for. */
    assert(solid_with(doc,&options,&info,&error,&volume) == TS_INVALID_GEOMETRY);
    assert(error.stage == TS_IMPORT_STAGE_TOPOLOGY && info.merged_vertices == 0);
    options.flags = TS_IMPORT_REPAIR_SEW | TS_IMPORT_REPAIR_ORIENT;
    assert(solid_with(doc,&options,&info,&error,&volume) == TS_OK);
    assert(info.struct_size == TS_SOLID_INFO_SIZE_1 && info.merged_vertices == 1 && info.merged_edges == 3);
    assert(info.sewing_tolerance_m > 0.99e-6 && info.sewing_tolerance_m < 1.01e-6);
    assert(info.max_merge_distance_m > 4.9e-8 && info.max_merge_distance_m < 5.1e-8);
    assert(info.working_tolerance_m > 1.4e-7 && info.working_tolerance_m < 1.6e-7);
    assert(volume > 5.99e-6 && volume < 6.01e-6);
    ts_document_release(doc);
    /* The curved tolerance policy: 3 um off the surface within 16 x 1 um is counted;
     * a factor of 1 or strict import rejects it at the geometry stage. */
    doc = load("near.step");
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    options.model_distance_m = 1e-6;
    assert(solid_with(doc,&options,&info,&error,&volume) == TS_OK);
    assert(info.off_surface_edges == 1 && info.off_curve_vertices == 0 && info.inserted_seams == 1);
    assert(info.largest_deviation_m > 2.99e-6 && info.largest_deviation_m < 3.01e-6);
    assert(info.working_tolerance_m > 6.99e-6 && info.working_tolerance_m < 7.01e-6);
    options.deviation_factor = 1;
    assert(solid_with(doc,&options,&info,&error,&volume) == TS_INVALID_GEOMETRY);
    assert(error.stage == TS_IMPORT_STAGE_GEOMETRY && error.entity_id != 0);
    options.deviation_factor = 0.5;
    assert(solid_with(doc,&options,&info,&error,&volume) == TS_INVALID_ARGUMENT);
    options.deviation_factor = 16;
    options.flags = TS_IMPORT_STRICT;
    assert(solid_with(doc,&options,&info,&error,&volume) == TS_INVALID_GEOMETRY);
    ts_document_release(doc);
}
static void surface_model_contract(void) {
    _Static_assert(sizeof(ts_surface_model_info) == TS_SURFACE_MODEL_INFO_SIZE_1, "surface model info");
    _Static_assert(TS_SURFACE_MODEL_INFO_SIZE_1 == 136, "surface model info revision 1");
    _Static_assert(sizeof(ts_surface_shell) == 48, "surface shell layout");
    _Static_assert(offsetof(ts_surface_shell, faces) == 16, "surface shell faces offset");
    ts_document *doc = load("surface-open.step");
    ts_surface_model *model = NULL, *failed = NULL;
    ts_import_error error;
    assert(ts_document_import_surface_model(doc,1000,0.001,1.0,NULL,&model,&error) == TS_OK);
    failed = model;
    assert(ts_document_import_surface_model(doc,9999,0.001,1.0,NULL,&failed,&error) == TS_NOT_FOUND);
    assert(!failed && error.entity_id == 9999);
    ts_import_options options;
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    options.flags = TS_DISCOVER_SURFACE_MODELS;
    ts_discovery *discovery = NULL;
    assert(ts_document_discover(doc,TS_PREFER_EXACT,&options,&discovery,NULL) == TS_OK);
    ts_document_release(doc);
    ts_discovery_info found;
    assert(ts_discovery_get_info(discovery,&found) == TS_OK && found.root_count == 2 && found.choice_count == 0);
    ts_discovered_root root;
    assert(ts_discovery_root_at(discovery,1,&root) == TS_OK && root.root_id == 1001);
    assert(root.kind == TS_ROOT_SHELL_BASED_SURFACE_MODEL && root.status == TS_OK && root.units.metres_per_unit == 0.001);
    ts_discovery_release(discovery);
    ts_surface_model_info info;
    info.struct_size = (uint32_t)sizeof info;
    assert(ts_surface_model_get_info(model,&info) == TS_OK);
    assert(info.root_id == 1000 && info.shell_count == 1 && info.closed_shell_count == 0);
    assert(info.face_count == 3 && info.boundary_edges == 8);
    ts_surface_shell shell;
    assert(ts_surface_model_shell_at(model,0,&shell) == TS_OK);
    assert(shell.entity_id == 105 && shell.closed == 0 && shell.face_count == 3 && shell.faces[0] != 0);
    assert(ts_surface_model_shell_at(model,1,&shell) == TS_NOT_FOUND && !shell.faces);
    ts_mesh *mesh = NULL;
    assert(ts_surface_model_shell_mesh(model,0,&mesh) == TS_OK);
    assert(ts_surface_model_retain(model) == TS_OK);
    ts_surface_model_release(model);
    ts_surface_model_release(model);
    /* The shell mesh outlives the model: one open component, never a solid. */
    ts_mesh_view view;
    assert(ts_mesh_get_view(mesh,&view) == TS_OK && view.components == 1 && view.boundary_edges > 0);
    ts_mesh_release(mesh);
    assert(ts_surface_model_get_info(NULL,&info) == TS_INVALID_ARGUMENT);
    assert(ts_surface_model_retain(NULL) == TS_INVALID_ARGUMENT);
    ts_surface_model_release(NULL);
}
static void session_contract(void) {
    _Static_assert(sizeof(ts_session_statistics) == 72, "session statistics layout");
    ts_document *doc = load("selection.step");
    ts_import_options options;
    assert(ts_import_options_for_document(doc, &options, sizeof options) == TS_OK);
    ts_import_session *session = NULL;
    assert(ts_import_session_create(doc,&options,&session) == TS_OK);
    ts_document_release(doc); /* The session retains its document. */
    ts_discovery *discovery = NULL;
    assert(ts_import_session_discover(session,TS_PREFER_EXACT,&discovery,NULL) == TS_OK);
    ts_discovery_info found;
    assert(ts_discovery_get_info(discovery,&found) == TS_OK && found.root_count == 5);
    ts_discovery_release(discovery);
    const uint64_t roots[] = {1000, 11000, 21000, 31000, 40002};
    size_t shared = 99;
    assert(ts_import_session_prepare(session,roots,5,&shared) == TS_OK && shared == 3);
    ts_mesh *mesh = NULL;
    ts_solid_info info;
    info.struct_size = (uint32_t)sizeof info;
    assert(ts_import_session_import_solid(session,1000,0.001,1.0,&mesh,&info,NULL) == TS_OK);
    assert(info.root_id == 1000 && info.face_count == 6);
    ts_mesh_release(mesh);
    ts_tessellated_info tessellated;
    assert(ts_import_session_import_tessellated(session,21000,0.001,&mesh,&tessellated,NULL) == TS_OK);
    assert(tessellated.kind == TS_TESSELLATED_SOLID && tessellated.link_id == 1000);
    ts_mesh_release(mesh);
    ts_session_statistics statistics;
    assert(ts_import_session_get_statistics(session,&statistics) == TS_OK);
    assert(statistics.discoveries == 4 && statistics.shared_decodes == 1 && statistics.imports == 2);
    ts_import_error error;
    assert(ts_import_session_import_solid(session,9999,0.001,1.0,&mesh,NULL,&error) == TS_NOT_FOUND && !mesh);
    assert(ts_import_session_prepare(session,NULL,1,&shared) == TS_INVALID_ARGUMENT && shared == 0);
    ts_import_session_release(session);

    doc = load("assembly.step");
    assert(ts_import_session_create(doc,NULL,&session) == TS_OK);
    ts_document_release(doc);
    ts_assembly *assembly = NULL;
    assert(ts_import_session_import_assembly(session,TS_PREFER_EXACT,&assembly,NULL) == TS_OK);
    ts_import_session_release(session);
    ts_assembly_info assembly_info;
    assert(ts_assembly_get_info(assembly,&assembly_info) == TS_OK && assembly_info.node_count == 11);
    ts_assembly_release(assembly);
    assert(ts_import_session_create(NULL,NULL,&session) == TS_INVALID_ARGUMENT && !session);
    assert(ts_import_session_get_statistics(NULL,&statistics) == TS_INVALID_ARGUMENT && statistics.imports == 0);
    ts_import_session_release(NULL);
}
static void assembly_options_contract(void) {
    _Static_assert(sizeof(ts_tolerated_encodings) == 40, "tolerated encodings layout");
    _Static_assert(sizeof(ts_assembly_adaptations) == 48, "assembly adaptations layout");
    ts_document *doc = load("mapped.step");
    ts_assembly *assembly = NULL, *failed = NULL;
    assert(ts_document_import_assembly_with_options(doc,TS_PREFER_EXACT,NULL,&assembly,NULL) == TS_OK);
    ts_document_release(doc);
    ts_assembly_adaptations adaptations;
    assert(ts_assembly_get_adaptations(assembly,&adaptations) == TS_OK && adaptations.mapped_placements == 6);
    ts_assembly_release(assembly);
    doc = load("inch.step");
    assert(ts_document_import_assembly_with_options(doc,TS_PREFER_EXACT,NULL,&assembly,NULL) == TS_OK);
    assert(ts_assembly_get_adaptations(assembly,&adaptations) == TS_OK);
    assert(adaptations.tolerated.omitted_conversion_dimensions == 1 && adaptations.mapped_placements == 0);
    ts_assembly_release(assembly);
    ts_import_options options;
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    options.flags = TS_IMPORT_STRICT;
    assert(ts_document_import_assembly_with_options(doc,TS_PREFER_EXACT,&options,&assembly,NULL) == TS_OK);
    ts_assembly_info info;
    assert(ts_assembly_get_info(assembly,&info) == TS_OK && info.imported_root_count == 0 && info.complete == 0);
    ts_assembly_release(assembly);
    assert(ts_document_import_assembly_with_options(doc,0,NULL,&failed,NULL) == TS_INVALID_ARGUMENT && !failed);
    ts_document_release(doc);
    assert(ts_assembly_get_adaptations(NULL,&adaptations) == TS_INVALID_ARGUMENT && adaptations.mapped_placements == 0);
}
static void diagnostics_contract(void) {
    _Static_assert(sizeof(ts_root_report) == 56, "root report layout");
    _Static_assert(offsetof(ts_root_report, message) == 40, "root report message offset");
    /* A session keeps its last failure text: the measured deviation and the bound. */
    ts_document *doc = load("near.step");
    ts_import_options options;
    assert(ts_import_options_init(&options, sizeof options) == TS_OK);
    options.model_distance_m = 1e-6;
    options.deviation_factor = 1;
    ts_import_session *session = NULL;
    assert(ts_import_session_create(doc,&options,&session) == TS_OK);
    ts_document_release(doc);
    ts_mesh *mesh = NULL;
    assert(ts_import_session_import_solid(session,1000,0.001,1.0,&mesh,NULL,NULL) == TS_INVALID_GEOMETRY);
    char text[512];
    size_t length = 0;
    assert(ts_import_session_last_message(session,text,sizeof text,&length) == TS_OK);
    assert(length > 0 && length < sizeof text && strlen(text) == length);
    assert(strstr(text, "3.000e-6 m") && strstr(text, "1.000e-6 m"));
    char tiny[4];
    size_t full = 0;
    assert(ts_import_session_last_message(session,tiny,sizeof tiny,&full) == TS_OK);
    assert(full == length && strlen(tiny) == 3 && memcmp(tiny, text, 3) == 0);
    assert(ts_import_session_last_message(session,NULL,0,&full) == TS_OK && full == length);
    assert(ts_import_session_last_message(session,NULL,4,&full) == TS_INVALID_ARGUMENT);
    ts_discovery *discovery = NULL;
    assert(ts_import_session_discover(session,TS_PREFER_EXACT,&discovery,NULL) == TS_OK);
    ts_discovery_release(discovery);
    assert(ts_import_session_last_message(session,text,sizeof text,&length) == TS_OK && length == 0 && text[0] == 0);
    ts_import_session_release(session);

    /* Session presentation and surface-model imports. */
    doc = load("presentation.step");
    assert(ts_import_session_create(doc,NULL,&session) == TS_OK);
    ts_document_release(doc);
    ts_presentation *presentation = NULL;
    assert(ts_import_session_import_presentation(session,300,0.001,&presentation,NULL) == TS_OK);
    ts_presentation_info presentation_info;
    assert(ts_presentation_get_info(presentation,&presentation_info) == TS_OK && presentation_info.item_count == 4);
    ts_presentation_release(presentation);
    ts_import_session_release(session);
    doc = load("surface-open.step");
    assert(ts_import_session_create(doc,NULL,&session) == TS_OK);
    ts_document_release(doc);
    ts_surface_model *model = NULL;
    assert(ts_import_session_import_surface_model(session,1000,0.001,1.0,&model,NULL) == TS_OK);
    ts_surface_model_release(model);
    ts_import_session_release(session);

    /* Per-root reports: tolerated encodings and failure text. */
    doc = load("inch.step");
    ts_assembly *assembly = NULL;
    assert(ts_document_import_assembly(doc,NULL,&assembly,NULL) == TS_OK);
    ts_root_report report;
    assert(ts_assembly_root_report_at(assembly,0,&report) == TS_OK);
    assert(report.tolerated.omitted_conversion_dimensions == 1 && !report.message.data && report.message.size == 0);
    assert(ts_assembly_root_report_at(assembly,5,&report) == TS_NOT_FOUND);
    ts_assembly_release(assembly);
    ts_document_release(doc);
    doc = load("cavity.step");
    assert(ts_document_discover(doc,TS_PREFER_EXACT,NULL,&discovery,NULL) == TS_OK);
    ts_document_release(doc);
    assert(ts_discovery_root_report_at(discovery,0,&report) == TS_OK);
    assert(text_is(report.message, "no supported representation contains the root"));
    ts_discovery_release(discovery);
}
int main(void) {
    parse_policy_contract();
    diagnostics_contract();
    repair_and_deviation_contract();
    surface_model_contract();
    session_contract();
    assembly_options_contract();
    import_options_contract();
    solid_contract();
    discovery_contract();
    assembly_contract();
    tessellated_contract();
    presentation_contract();
    solid_import_contract(0);
    solid_import_contract(1);
    import_policy_contract();
    appearance_contract();
    scene_contract();
    mesh_contract();
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
