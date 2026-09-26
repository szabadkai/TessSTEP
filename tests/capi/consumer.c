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
static int text_is(ts_string_view view, const char *expected) {
    return view.size == strlen(expected) && memcmp(view.data, expected, view.size) == 0;
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
int main(void) {
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
