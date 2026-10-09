#include <tessstep/tessstep.hpp>
#include <algorithm>
#include <cassert>
#include <future>
#include <cmath>
#include <fstream>
#include <iterator>
#include <type_traits>
using namespace tessstep;
static_assert(!std::is_copy_constructible_v<Document>);
static_assert(std::is_nothrow_move_constructible_v<Document>);
static_assert(std::is_nothrow_move_assignable_v<Document>);
static_assert(std::is_nothrow_destructible_v<Document>);
constexpr auto input = "ISO-10303-21;HEADER;FILE_DESCRIPTION(('test'),'2;1');"
    "FILE_NAME('','',('a'),('o'),'','','');FILE_SCHEMA(('EXAMPLE'));ENDSEC;DATA;"
    "#9=ITEM(#42);ENDSEC;END-ISO-10303-21;";
static void mesh_contract() {
    static_assert(!std::is_copy_constructible_v<Mesh>);
    static_assert(std::is_nothrow_move_constructible_v<Mesh>);
    static_assert(std::is_nothrow_destructible_v<MeshView>);
    static_assert(!std::is_copy_constructible_v<MeshView>);
    double xyz[] = {0,0,0, 1,0,0, 0,1,0};
    const uint32_t indices[] = {0,1,2};
    auto retained = [&] {
        auto result = Mesh::from_triangles(xyz,3,indices,1);
        assert(result);
        Mesh mesh = std::move(result).value();
        assert(!result.value().view());
        auto first = mesh.view();
        auto second = mesh.view();
        assert(first.value().data().positions == second.value().data().positions);
        xyz[0] = 9;
        assert(first.value().data().positions[0] == 0);
        return std::move(first).value();
    }(); // Both original Mesh and other view are gone.
    assert(retained.data().vertex_count == 3);
    auto read = [&] { for (int i=0;i<100;++i) assert(retained.data().positions[3] == 1 && retained.data().triangles[2] == 2); };
    auto first = std::async(std::launch::async,read);
    auto second = std::async(std::launch::async,read);
    first.get(); second.get();
    MeshView moved = std::move(retained);
    assert(!retained.data().positions && moved.data().normals[2] == 1);
    auto options = default_mesh_options();options.require_solid = 1;xyz[0] = 0;
    auto invalid = Mesh::from_triangles(xyz,3,indices,1,&options);
    assert(!invalid && invalid.error().code == ErrorCode::invalid_mesh);
}
static void scene_contract() {
    static_assert(!std::is_copy_constructible_v<Scene>);
    static_assert(std::is_nothrow_move_constructible_v<Scene>);
    static_assert(std::is_nothrow_move_assignable_v<Scene>);
    static_assert(std::is_nothrow_destructible_v<Scene>);
    const double xyz[] = {0,0,0, 1,0,0, 0,1,0};
    const uint32_t tri[] = {0,1,2};
    auto result = [&] {
        auto asset=Mesh::from_triangles(xyz,3,tri,1); assert(asset);
        auto parent=scene_instance(42); parent.translation[0]=10;
        auto child=scene_instance(7,42,9); child.translation[0]=1; child.linear[0]=-2;
        return Scene::create({{9,&asset.value()}},{child,parent});
    }();
    assert(result);
    Scene scene=std::move(result).value();
    assert(!result.value().info() && result.value().info().error().code==ErrorCode::invalid_argument);
    assert(scene.info().value().asset_count==1);
    assert(scene.instance_at(0).value().world_translation[0]==11);
    assert(scene.instance_at(0).value().mirrored==1);
    auto read=[&] {for(int i=0;i<100;++i) assert(scene.instance_at(0).value().source.id==7);};
    auto first=std::async(std::launch::async,read), second=std::async(std::launch::async,read);
    first.get(); second.get();
    auto asset=scene.asset_mesh(9); assert(asset);
    auto another=scene.asset_mesh(9); assert(another);
    auto view=asset.value().view(); assert(view);
    assert(view.value().data().positions==another.value().view().value().data().positions);
    auto baked=scene.bake(7); assert(baked);
    assert(baked.value().view().value().data().positions[3]==9);
    assert(!scene.bake(42) && scene.bake(42).error().code==ErrorCode::not_found);
    auto empty=Scene::create({},{}); assert(empty);
    scene=std::move(empty).value();
    assert(view.value().data().positions[3]==1);
    assert(baked.value().view().value().data().positions[0]==11);
    auto invalid=Scene::create({}, {scene_instance(1,1)});
    assert(!invalid && invalid.error().code==ErrorCode::invalid_scene);
}
static void appearance_contract() {
    static_assert(!std::is_copy_constructible_v<Appearance>);
    static_assert(std::is_nothrow_move_constructible_v<Appearance>);
    static_assert(std::is_nothrow_move_assignable_v<Appearance>);
    static_assert(std::is_nothrow_destructible_v<Appearance>);
    const double xyz[]={0,0,0,1,0,0,0,1,0}; const uint32_t tri[]={0,1,2};
    const double* positions=nullptr;
    auto result=[&] {
        auto mesh=Mesh::from_triangles(xyz,3,tri,1);assert(mesh);
        positions=mesh.value().view().value().data().positions;
        auto scene=Scene::create({{1,&mesh.value()}},{scene_instance(7,42,1),scene_instance(8,42,1),scene_instance(42)});
        assert(scene);
        return Appearance::create(scene.value(),{{11,{1,0,0,1}},{22,{0,1,0,0}}},
            {style_binding(StyleScope::instance,42,11),style_binding(StyleScope::instance_face,7,22,0)});
    }(); // Both original scene and mesh are gone.
    assert(result);
    Appearance appearance=std::move(result).value();
    assert(!result.value().info() && result.value().info().error().code==ErrorCode::invalid_argument);
    assert(appearance.info().value().material_count==2);
    assert(appearance.binding_at(0).value().material_id==11);
    assert(appearance.material_at(1).value().rgba[3]==0);
    auto read=[&] {for(int i=0;i<100;++i) {
        auto face=appearance.resolve_triangle(7,0);assert(face && face.value().material.id==22);
        assert(face.value().material.rgba[3]==0 && face.value().source.id==7);
        assert(appearance.resolve_triangle(8,0).value().material.id==11);
    }};
    auto first=std::async(std::launch::async,read),second=std::async(std::launch::async,read);
    first.get();second.get();
    assert(!appearance.resolve_triangle(7,1) && appearance.resolve_triangle(7,1).error().code==ErrorCode::not_found);
    auto scene=appearance.scene();assert(scene);
    auto mesh=scene.value().asset_mesh(1);assert(mesh);
    auto view=mesh.value().view();assert(view && view.value().data().positions==positions);
    auto empty=Appearance::create(scene.value(),{},{});assert(empty);
    appearance=std::move(empty).value(); // Releases prior appearance, retained view survives.
    assert(view.value().data().positions[3]==1);
    assert(appearance.resolve_triangle(7,0).value().material.id==0);
    auto invalid=Appearance::create(scene.value(),{{1,{2,0,0,1}}},{});
    assert(!invalid && invalid.error().code==ErrorCode::invalid_appearance);
    auto options=default_appearance_options();options.max_materials=0;
    auto limited=Appearance::create(scene.value(),{{1,{1,0,0,1}}},{},&options);
    assert(!limited && limited.error().code==ErrorCode::resource_limit);
}
static void faceted_contract() {
    auto retained = [] {
        std::ifstream file("faceted.step", std::ios::binary);
        assert(file);
        const std::string bytes{std::istreambuf_iterator<char>(file),std::istreambuf_iterator<char>()};
        auto parsed = Document::parse(bytes);
        assert(parsed);
        Document document = std::move(parsed).value();
        auto imported = document.tessellate_faceted(1000,0.001);
        assert(imported);
        auto view = imported.value().view();
        assert(view && view.value().data().face_ids[0] == 106);
        auto bad = document.tessellate_faceted(1,0.001);
        assert(!bad && bad.error().code == ErrorCode::unsupported);
        assert(bad.error().import_failure.stage == ImportStage::geometry);
        assert(bad.error().import_failure.entity_id == 1);
        assert(!parsed.value().tessellate_faceted(1000,0.001));
        return std::move(view).value();
    }();
    assert(retained.data().vertex_count == 8 && retained.data().triangle_count == 12);
    assert(retained.data().boundary_edges == 0 && retained.data().components == 1);
    assert(retained.data().signed_volume > 0.000005999 && retained.data().signed_volume < 0.000006001);
}
static void planar_contract(const char* path, uint64_t root, double scale, double volume,
    double x, double y, double z) {
    auto retained = [&] {
        std::ifstream file(path,std::ios::binary); assert(file);
        const std::string bytes{std::istreambuf_iterator<char>(file),std::istreambuf_iterator<char>()};
        auto parsed = Document::parse(bytes); assert(parsed);
        auto options = default_planar_options();
        auto converted = parsed.value().tessellate_planar(root,scale,&options); assert(converted);
        auto view = converted.value().view(); assert(view);
        assert(view.value().data().face_ids[0] == 106);
        auto bad = parsed.value().tessellate_planar(root,0.);
        assert(!bad && bad.error().code == ErrorCode::invalid_argument);
        options.max_work=0;
        bad = parsed.value().tessellate_planar(root,scale,&options);
        assert(!bad && bad.error().code == ErrorCode::resource_limit);
        assert(bad.error().import_failure.stage == ImportStage::profile);
        return std::move(view).value();
    }();
    const auto& data=retained.data();
    assert(data.vertex_count == 8 && data.triangle_count == 12 && data.boundary_edges == 0 && data.components == 1);
    assert(std::abs(data.signed_volume-volume)<volume*1e-10);
    const double bounds[] = {x,y,z};
    for(size_t axis=0;axis<3;++axis) {
        double maximum=0.;
        for(size_t v=0;v<data.vertex_count;++v) {
            const auto value=data.positions[3*v+axis];
            assert(value>=-1e-12);
            if(value>maximum) maximum=value;
        }
        assert(std::abs(maximum-bounds[axis])<1e-12);
    }
}
static std::string read_file(const char* path) {
    std::ifstream file(path,std::ios::binary); assert(file);
    return {std::istreambuf_iterator<char>(file),std::istreambuf_iterator<char>()};
}
static void tessellated_contract() {
    static_assert(!std::is_copy_constructible_v<Presentation>);
    static_assert(std::is_nothrow_move_constructible_v<Presentation>);
    static_assert(!std::is_copy_constructible_v<PresentationView>);
    static_assert(std::is_nothrow_destructible_v<PresentationView>);
    auto mesh = [] {
        auto parsed = Document::parse(read_file("tessellated.step")); assert(parsed);
        auto imported = parsed.value().import_tessellated(1000,0.001); assert(imported);
        assert(imported.value().kind() == TessellatedKind::solid);
        assert(imported.value().info.edge_count == 13 && imported.value().info.joined_points == 24);
        auto options = default_tessellated_options();
        options.flags = 2u;
        auto bad = parsed.value().import_tessellated(1000,0.001,&options);
        assert(!bad && bad.error().code == ErrorCode::invalid_argument);
        auto list = parsed.value().import_tessellated(10,0.001);
        assert(!list && list.error().code == ErrorCode::unsupported);
        assert(list.error().import_failure.stage == ImportStage::geometry && list.error().import_failure.entity_id == 10);
        return std::move(imported).value().mesh;
    }(); // The document is gone; the mesh is independent.
    auto view = mesh.view(); assert(view);
    assert(view.value().data().vertex_count == 8 && view.value().data().boundary_edges == 0);
    auto graphics = [] {
        auto parsed = Document::parse(read_file("presentation.step")); assert(parsed);
        auto imported = parsed.value().import_presentation(300,0.001); assert(imported);
        auto missing = parsed.value().import_presentation(110,0.001);
        assert(!missing && missing.error().code == ErrorCode::unsupported);
        auto limits = default_presentation_options();
        limits.max_depth = 0;
        assert(parsed.value().import_presentation(300,0.001,&limits).error().code == ErrorCode::resource_limit);
        Presentation presentation = std::move(imported).value();
        assert(presentation.info().value().item_count == 4 && presentation.style_at(0).value() == 203);
        assert(presentation.style_at(1).error().code == ErrorCode::not_found);
        assert(static_cast<PresentationKind>(presentation.item_at(1).value().kind) == PresentationKind::surface_set);
        return std::move(presentation.view()).value();
    }(); // The view retains the graphics after the Presentation is destroyed.
    const auto& data = graphics.data();
    assert(data.vertex_count == 12 && data.polyline_count == 3 && data.polyline_offsets[3] == 9);
    assert(data.triangle_count == 3 && data.point_count == 1 && data.positions[0] == 0.1);
    PresentationView moved = std::move(graphics);
    assert(moved.data().polyline_count == 3 && !graphics.data().positions);
}
static void assembly_contract() {
    auto [assembly, scene] = [] {
        auto parsed = Document::parse(read_file("assembly.step")); assert(parsed);
        auto schema = parsed.value().schema(); assert(schema);
        assert(schema.value().protocol == Protocol::ap214 && schema.value().schema_count == 1);
        assert(schema.value().declared.rfind("AUTOMOTIVE_DESIGN", 0) == 0);
        auto options = default_assembly_options();
        options.chord_m = -1;
        assert(parsed.value().import_assembly(&options).error().code == ErrorCode::invalid_argument);
        auto imported = parsed.value().import_assembly(); assert(imported);
        auto scene = imported.value().scene(); assert(scene);
        return std::pair{std::move(imported).value(), std::move(scene).value()};
    }(); // The document is gone; the assembly and scene are independent.
    auto info = assembly.info().value();
    assert(info.node_count == 11 && info.product_count == 3 && info.occurrence_count == 4 && info.complete == 1);
    auto root = assembly.root_at(0).value();
    assert(root.kind == RootKind::faceted_brep && root.selected && root.status == TS_OK);
    assert(assembly.root_at(1).error().code == ErrorCode::not_found);
    std::vector<std::string> names;
    for (size_t i = 0; i < info.product_count; ++i) names.push_back(assembly.product_at(i).value().name);
    std::sort(names.begin(), names.end());
    assert((names == std::vector<std::string>{"block", "pair", "rig"}));
    size_t leaves = 0;
    for (size_t i = 0; i < info.node_count; ++i) leaves += assembly.node_at(i).value().root_id != 0;
    assert(leaves == 4 && scene.info().value().instance_count == 11);
    {
        auto parsed = Document::parse(read_file("styled.step")); assert(parsed);
        auto styled = parsed.value().import_assembly(); assert(styled);
        auto style = styled.value().style_info().value();
        assert(style.asset_styles == 1 && style.face_styles == 1 && style.no_surface_colour == 1);
        auto appearance = styled.value().appearance(); assert(appearance);
        assert(appearance.value().info().value().material_count == 2);
    }
    auto parsed = Document::parse(read_file("missing-placement.step")); assert(parsed);
    auto missing = parsed.value().import_assembly();
    assert(!missing && missing.error().code == ErrorCode::invalid_geometry);
    assert(missing.error().import_failure.stage == ImportStage::product);
}
static void discovery_contract() {
    static_assert(!std::is_copy_constructible_v<Discovery>);
    static_assert(std::is_nothrow_move_constructible_v<Discovery>);
    static_assert(std::is_nothrow_move_assignable_v<Discovery>);
    static_assert(std::is_nothrow_destructible_v<Discovery>);
    auto owned = [] {
        auto parsed = Document::parse(read_file("selection.step")); assert(parsed);
        auto options = parsed.value().import_options(); assert(options);
        assert(options.value().struct_size == sizeof(ImportOptions) && options.value().max_work == 5000000);
        auto found = parsed.value().discover(Preference::exact, &options.value()); assert(found);
        auto choice = found.value().choice_at(0).value();
        assert(choice.selected == 0 && choice.alternatives == std::vector<size_t>{2});
        assert(choice.pairing == Pairing::link);
        auto root = found.value().root_at(choice.selected).value();
        assert(root.root_id == 1000 && root.kind == RootKind::manifold_solid_brep && root.units);
        assert(root.units.value().metres_per_unit == 0.001 && root.units.value().distance_uncertainty_m);
        auto solid = parsed.value().import_solid(root, &options.value()); assert(solid);
        assert(solid.value().kind() == RootKind::manifold_solid_brep && solid.value().info.face_count == 6);
        assert(solid.value().info.struct_size == sizeof(SolidInfo));
        auto tessellated = parsed.value().discover(Preference::tessellated); assert(tessellated);
        auto first = tessellated.value().choice_at(0).value();
        assert(first.selected == 2 && first.alternatives == std::vector<size_t>{0});
        Discovery moved = std::move(found).value();
        assert(!found.value().info() && found.value().info().error().code == ErrorCode::invalid_argument);
        return std::pair{std::move(moved), std::move(solid).value().mesh};
    }(); // The document is gone; the discovery and mesh are independent.
    Discovery& discovery = owned.first;
    Mesh& mesh = owned.second;
    auto read = [&discovery] {
        for (int i = 0; i < 100; ++i) {
            auto root = discovery.root_at(3).value();
            assert(root.kind == RootKind::tessellated_shell && root.link_id == 30021);
            assert(root.representations.size() == 1);
        }
    };
    auto first = std::async(std::launch::async, read), second = std::async(std::launch::async, read);
    first.get(); second.get();
    assert(discovery.info().value().choice_count == 3);
    assert(discovery.choice_at(1).value().pairing == Pairing::relationship);
    assert(discovery.choice_at(3).error().code == ErrorCode::not_found);
    auto volume = mesh.view().value().data().signed_volume;
    assert(volume > 0.000005999 && volume < 0.000006001);
    {
        auto parsed = Document::parse(read_file("presentation.step")); assert(parsed);
        auto found = parsed.value().discover(); assert(found);
        auto note = found.value().presentation_at(0).value();
        assert(note.occurrence_id == 300 && note.containers == std::vector<uint64_t>{310});
        assert(note.representations.size() == 2 && note.units);
        auto orphan = found.value().presentation_at(1).value();
        assert(!orphan.units && orphan.units.error().code == ErrorCode::unsupported);
        assert(orphan.units.error().import_failure.entity_id == 301);
        auto options = default_import_options();
        options.max_work = 10;
        auto limited = parsed.value().discover(Preference::exact, &options);
        assert(!limited && limited.error().code == ErrorCode::resource_limit);
        assert(limited.error().import_failure.stage == ImportStage::profile);
    }
    {
        // A root without units propagates its typed error; explicit units import it.
        auto parsed = Document::parse(read_file("cavity.step")); assert(parsed);
        auto root = parsed.value().discover().value().root_at(0).value();
        assert(root.kind == RootKind::brep_with_voids && !root.units);
        auto missing = parsed.value().import_solid(root);
        assert(!missing && missing.error().code == ErrorCode::unsupported);
        assert(missing.error().import_failure.entity_id == 1000);
        auto solid = parsed.value().import_solid(1000, 0.001, 1.0); assert(solid);
        assert(solid.value().kind() == RootKind::brep_with_voids && solid.value().info.shell_count == 2);
        assert(solid.value().mesh.view().value().data().components == 2);
    }
    auto parsed = Document::parse(read_file("cylinder.step")); assert(parsed);
    auto options = default_import_options();
    assert(options.struct_size == TS_IMPORT_OPTIONS_SIZE_2 && options.chord_m == 1e-4);
    auto cylinder = parsed.value().import_solid(1000, 0.001, 1.0, &options); assert(cylinder);
    assert(cylinder.value().info.inserted_seams == 1 && cylinder.value().info.face_count == 3);
    options.flags = 32u;
    assert(parsed.value().import_solid(1000, 0.001, 1.0, &options).error().code == ErrorCode::invalid_argument);
    options = default_import_options();
    options.max_vertices = 0;
    auto limited = parsed.value().import_solid(1000, 0.001, 1.0, &options);
    assert(!limited && limited.error().code == ErrorCode::resource_limit);
    assert(limited.error().import_failure.stage == ImportStage::tessellation);
    assert(parsed.value().import_solid(1000, 0., 1.0).error().code == ErrorCode::invalid_argument);
}
static void second_pass_contract() {
    static_assert(!std::is_copy_constructible_v<SurfaceModel>);
    static_assert(std::is_nothrow_move_constructible_v<SurfaceModel>);
    static_assert(!std::is_copy_constructible_v<ImportSession>);
    static_assert(std::is_nothrow_move_constructible_v<ImportSession>);
    static_assert(std::is_nothrow_destructible_v<ImportSession>);
    {
        // Opt-in repair, reported in the second solid-info revision.
        auto parsed = Document::parse(read_file("duplicated.step")); assert(parsed);
        auto strict = parsed.value().import_solid(1000, 0.001, 1.0);
        assert(!strict && strict.error().import_failure.stage == ImportStage::topology);
        auto options = default_import_options();
        options.flags |= TS_IMPORT_REPAIR_SEW | TS_IMPORT_REPAIR_ORIENT;
        auto repaired = parsed.value().import_solid(1000, 0.001, 1.0, &options); assert(repaired);
        assert(repaired.value().info.struct_size == TS_SOLID_INFO_SIZE_2);
        assert(repaired.value().info.merged_vertices == 1 && repaired.value().info.merged_edges == 3);
    }
    {
        // The tolerance policy: a counted deviation, rejected with a factor of 1.
        auto parsed = Document::parse(read_file("near.step")); assert(parsed);
        auto options = default_import_options();
        options.model_distance_m = 1e-6;
        auto near = parsed.value().import_solid(1000, 0.001, 1.0, &options); assert(near);
        assert(near.value().info.off_surface_edges == 1);
        assert(std::abs(near.value().info.largest_deviation_m - 3e-6) < 1e-12);
        options.deviation_factor = 1;
        auto far = parsed.value().import_solid(1000, 0.001, 1.0, &options);
        assert(!far && far.error().code == ErrorCode::invalid_geometry);
        assert(far.error().import_failure.stage == ImportStage::geometry);
    }
    auto [model, shell_mesh] = [] {
        auto parsed = Document::parse(read_file("surface-open.step")); assert(parsed);
        auto options = default_import_options();
        options.flags |= TS_DISCOVER_SURFACE_MODELS;
        auto found = parsed.value().discover(Preference::exact, &options); assert(found);
        auto root = found.value().root_at(0).value();
        assert(root.kind == RootKind::shell_based_surface_model && root.units);
        auto imported = parsed.value().import_surface_model(root); assert(imported);
        auto mesh = imported.value().shell_mesh(0); assert(mesh);
        return std::pair{std::move(imported).value(), std::move(mesh).value()};
    }(); // The document is gone; the model and its shell mesh are independent.
    auto info = model.info().value();
    assert(info.shell_count == 1 && info.closed_shell_count == 0 && info.boundary_edges == 8);
    auto shell = model.shell_at(0).value();
    assert(shell.entity_id == 105 && !shell.closed && shell.faces.size() == 3);
    assert(model.shell_at(1).error().code == ErrorCode::not_found);
    auto view = shell_mesh.view(); assert(view);
    assert(view.value().data().components == 1 && view.value().data().boundary_edges > 0);
    {
        auto parsed = Document::parse(read_file("selection.step")); assert(parsed);
        auto options = parsed.value().import_options(); assert(options);
        auto created = parsed.value().import_session(&options.value()); assert(created);
        ImportSession session = std::move(created).value();
        parsed = Document::parse(read_file("assembly.step")); // The session keeps its document.
        auto found = session.discover(); assert(found);
        assert(found.value().info().value().root_count == 5);
        auto shared = session.prepare({1000, 11000, 21000, 31000, 40002});
        assert(shared && shared.value() == 3);
        auto solid = session.import_solid(found.value().root_at(1).value()); assert(solid);
        assert(solid.value().kind() == RootKind::faceted_brep);
        auto tessellated = session.import_tessellated(21000, 0.001); assert(tessellated);
        assert(tessellated.value().kind() == TessellatedKind::solid);
        auto statistics = session.statistics().value();
        assert(statistics.shared_decodes == 1 && statistics.imports == 2);
        auto missing = session.import_solid(9999, 0.001, 1.0);
        assert(!missing && missing.error().code == ErrorCode::not_found);
        ImportSession moved = std::move(session);
        assert(!session.statistics() && session.statistics().error().code == ErrorCode::invalid_argument);
        assert(moved.statistics().value().imports == 3);
    }
    {
        auto parsed = Document::parse(read_file("mapped.step")); assert(parsed);
        auto assembly = parsed.value().import_assembly(Preference::exact); assert(assembly);
        assert(assembly.value().adaptations().value().mapped_placements == 6);
        auto session = parsed.value().import_session(); assert(session);
        auto again = session.value().import_assembly(); assert(again);
        assert(again.value().info().value().occurrence_count == 4);
    }
}
int main(int argc, char** argv) {
    second_pass_contract();
    discovery_contract();
    assembly_contract();
    tessellated_contract();
    planar_contract("planar.step",1000,0.001,6e-6,0.01,0.02,0.03);
    {
        std::ifstream file("placeholder.step",std::ios::binary); assert(file);
        const std::string bytes{std::istreambuf_iterator<char>(file),std::istreambuf_iterator<char>()};
        auto parsed = Document::parse(bytes); assert(parsed);
        assert(parsed.value().tessellate_planar(1000,0.001));
        auto policy = default_import_policy();
        assert(policy.flags == 0 && parsed.value().tessellate_planar(1000,0.001,nullptr,&policy));
        policy.flags |= TS_IMPORT_STRICT;
        auto strict = parsed.value().tessellate_planar(1000,0.001,nullptr,&policy);
        assert(!strict && strict.error().code == ErrorCode::invalid_geometry);
        assert(strict.error().import_failure.stage == ImportStage::profile);
    }
    if(argc == 2) planar_contract(argv[1],121,1.,0.000098322384,0.0508,0.0254,0.0762);
    faceted_contract();
    appearance_contract();
    scene_contract();
    mesh_contract();
    auto parsed = Document::parse(input);
    assert(parsed);
    Document doc = std::move(parsed).value();
    assert(!parsed.value().info()); // Moved-from wrappers remain safe.
    assert(parsed.value().info().error().code == ErrorCode::invalid_argument);
    assert(doc.info().value().entity_count == 1);
    auto read = [&doc] {
        for (int i = 0; i < 100; ++i) {
            assert(doc.entity_at(0).value().id == 9);
            assert(doc.record_name(9).value() == "ITEM");
            assert(doc.diagnostics().value().at(0).code == "TS1103");
        }
    };
    auto first = std::async(std::launch::async, read);
    auto second = std::async(std::launch::async, read);
    first.get(); second.get();
    assert(doc.entity_at(1).error().code == ErrorCode::not_found);
    assert(doc.record_name(99).error().code == ErrorCode::not_found);
    auto owned = [&] {
        auto other = Document::parse(input);
        auto report = other.value().diagnostics();
        assert(report && report.value().size() == 1);
        return std::move(report).value();
    }();
    assert(owned[0].code == "TS1103" && owned[0].entity_id == 9 && owned[0].severity == Severity::error);
    auto replacement = Document::parse(input);
    doc = std::move(replacement).value(); // Releases previous ownership.
    assert(doc.record_name(9).value() == "ITEM");
    auto empty = Document::parse({});
    assert(!empty && empty.error().code == ErrorCode::parse_error);
    assert(!empty.error().diagnostics.empty());
    auto options = default_parse_options();
    options.max_input_bytes = 0;
    auto limited = Document::parse(input, &options);
    assert(!limited && limited.error().code == ErrorCode::resource_limit);
    assert(limited.error().diagnostics.at(0).code == "TS1201");
}
