#include <tessstep/tessstep.hpp>
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
int main(int argc, char** argv) {
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
