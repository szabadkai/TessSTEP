#ifndef TESSSTEP_HPP
#define TESSSTEP_HPP

#include "tessstep.h"
#include <memory>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

namespace tessstep {
// C++17 wrapper using only the public C protocol. Owning types are move-only;
// destructors and moves never throw. Results report library failures. Standard
// allocation exceptions may still arise when constructing C++ strings/vectors.
enum class ErrorCode : uint32_t {
    invalid_argument = TS_INVALID_ARGUMENT, parse_error = TS_PARSE_ERROR,
    unsupported = TS_UNSUPPORTED, resource_limit = TS_RESOURCE_LIMIT,
    not_found = TS_NOT_FOUND, internal_error = TS_INTERNAL_ERROR, invalid_mesh = TS_INVALID_MESH, invalid_scene = TS_INVALID_SCENE, invalid_appearance = TS_INVALID_APPEARANCE, invalid_geometry = TS_INVALID_GEOMETRY
};
enum class Severity : uint32_t { error = TS_SEVERITY_ERROR, warning = TS_SEVERITY_WARNING };
struct Diagnostic {
    Severity severity;
    uint64_t entity_id, start_offset, end_offset, line, column;
    std::string code, message;
};
enum class ImportStage : uint32_t {
    none = TS_IMPORT_STAGE_NONE, profile = TS_IMPORT_STAGE_PROFILE,
    geometry = TS_IMPORT_STAGE_GEOMETRY, topology = TS_IMPORT_STAGE_TOPOLOGY,
    tessellation = TS_IMPORT_STAGE_TESSELLATION
};
struct ImportFailure {
    ImportStage stage = ImportStage::none;
    uint64_t entity_id = 0, start_offset = 0, end_offset = 0;
};
struct Error {
    ErrorCode code;
    std::vector<Diagnostic> diagnostics;
    ImportFailure import_failure{};
};
template<class T> class [[nodiscard]] Result {
    std::variant<T, Error> data_;
public:
    Result(T value) : data_(std::move(value)) {}
    Result(Error error) : data_(std::move(error)) {}
    explicit operator bool() const noexcept { return std::holds_alternative<T>(data_); }
    // Access the matching branch only; incorrect access throws bad_variant_access.
    T& value() & { return std::get<T>(data_); }
    const T& value() const & { return std::get<T>(data_); }
    T&& value() && { return std::get<T>(std::move(data_)); }
    const Error& error() const { return std::get<Error>(data_); }
};
using ParseOptions = ts_parse_options;
using DocumentInfo = ts_document_info;
using EntityInfo = ts_entity_info;
inline ParseOptions default_parse_options() noexcept {
    ParseOptions options{};
    ts_parse_options_init(&options);
    return options;
}
namespace detail {
inline Error error(ts_status status) { return {static_cast<ErrorCode>(status), {}}; }
inline std::string copy(ts_string_view view) {
    return view.size ? std::string(view.data, view.size) : std::string();
}
struct DocumentDeleter {
    void operator()(ts_document* p) const noexcept { ts_document_release(p); }
};
struct ReportDeleter {
    void operator()(ts_diagnostics* p) const noexcept { ts_diagnostics_release(p); }
};
using Report = std::unique_ptr<ts_diagnostics, ReportDeleter>;
inline Result<std::vector<Diagnostic>> copy_report(const Report& report) {
    size_t count = 0;
    auto status = ts_diagnostics_count(report.get(), &count);
    if (status != TS_OK) return error(status);
    std::vector<Diagnostic> result;
    result.reserve(count);
    for (size_t i = 0; i < count; ++i) {
        ts_diagnostic d{};
        status = ts_diagnostics_get(report.get(), i, &d);
        if (status != TS_OK) return error(status);
        result.push_back({static_cast<Severity>(d.severity), d.entity_id,
            d.start_offset, d.end_offset, d.line, d.column, copy(d.code), copy(d.message)});
    }
    return result;
}
}
using FacetedOptions = ts_faceted_options;
inline FacetedOptions default_faceted_options() noexcept {
    FacetedOptions options{}; ts_faceted_options_init(&options); return options;
}
using PlanarOptions = ts_planar_options;
inline PlanarOptions default_planar_options() noexcept {
    PlanarOptions options{}; ts_planar_options_init(&options); return options;
}
// flags 0 is the tolerant default; set flags |= TS_IMPORT_STRICT to reject
// accepted exporter deviations (see ts_import_policy).
using ImportPolicy = ts_import_policy;
inline ImportPolicy default_import_policy() noexcept {
    ImportPolicy policy{}; ts_import_policy_init(&policy); return policy;
}
class Mesh;
class Document {
    std::unique_ptr<ts_document, detail::DocumentDeleter> handle_;
    explicit Document(ts_document* handle) noexcept : handle_(handle) {}
public:
    Document(const Document&) = delete;
    Document& operator=(const Document&) = delete;
    Document(Document&&) noexcept = default;
    Document& operator=(Document&&) noexcept = default;
    ~Document() noexcept = default;

    // Bytes are borrowed only during the call. This parses physical syntax only;
    // diagnostics() performs the separate reference/trust analysis.
    static Result<Document> parse(std::string_view bytes, const ParseOptions* options = nullptr) {
        ts_document* raw = nullptr;
        ts_diagnostics* raw_report = nullptr;
        const auto status = ts_document_parse(reinterpret_cast<const uint8_t*>(bytes.data()),
            bytes.size(), options, &raw, &raw_report);
        Document document(raw);
        detail::Report report(raw_report); // Guard every allocation before C++ copies.
        if (status == TS_OK) return document;
        Error error = detail::error(status);
        if (report) {
            auto copied = detail::copy_report(report);
            if (!copied) return copied.error();
            error.diagnostics = std::move(copied).value();
        }
        return error;
    }
    // Explicit root and source units. The owned result survives this document.
    Result<Mesh> tessellate_faceted(uint64_t entity_id, double metres_per_unit,
        const FacetedOptions* options = nullptr) const;
    Result<Mesh> tessellate_planar(uint64_t entity_id, double metres_per_unit,
        const PlanarOptions* options = nullptr, const ImportPolicy* policy = nullptr) const;
    // A moved-from Document is safe to destroy, reassign or query. Queries return
    // invalid_argument. Concurrent const queries require the wrapper to stay alive.
    Result<DocumentInfo> info() const {
        DocumentInfo info{};
        const auto status = ts_document_get_info(handle_.get(), &info);
        if (status != TS_OK) return detail::error(status);
        return info;
    }
    Result<EntityInfo> entity_at(size_t index) const {
        EntityInfo info{};
        const auto status = ts_document_entity_at(handle_.get(), index, &info);
        if (status != TS_OK) return detail::error(status);
        return info;
    }
    // Returns owned text, copied explicitly; it survives moves/destruction.
    Result<std::string> record_name(uint64_t id, size_t component = 0) const {
        ts_string_view text{};
        const auto status = ts_document_record_name(handle_.get(), id, component, &text);
        if (status != TS_OK) return detail::error(status);
        return detail::copy(text);
    }
    // Returns owned diagnostic strings; results outlive this document.
    Result<std::vector<Diagnostic>> diagnostics() const {
        ts_diagnostics* raw = nullptr;
        const auto status = ts_document_diagnostics(handle_.get(), &raw);
        detail::Report report(raw);
        if (status != TS_OK) return detail::error(status);
        return detail::copy_report(report);
    }
};
using MeshOptions = ts_mesh_options;
inline MeshOptions default_mesh_options() noexcept {
    MeshOptions options{};
    ts_mesh_options_init(&options);
    return options;
}
namespace detail {
struct MeshDeleter { void operator()(ts_mesh* p) const noexcept { ts_mesh_release(p); } };
}
// Each view owns an independent retained reference, so it survives destruction or
// movement of the originating Mesh. Buffers themselves are never copied by view().
class MeshView {
    std::unique_ptr<ts_mesh, detail::MeshDeleter> owner_;
    ts_mesh_view data_{};
    friend class Mesh;
    MeshView(ts_mesh* owner, ts_mesh_view data) noexcept : owner_(owner), data_(data) {}
public:
    MeshView(const MeshView&) = delete;
    MeshView& operator=(const MeshView&) = delete;
    MeshView(MeshView&& other) noexcept
        : owner_(std::move(other.owner_)), data_(std::exchange(other.data_, ts_mesh_view{})) {}
    MeshView& operator=(MeshView&& other) noexcept {
        if (this != &other) { owner_ = std::move(other.owner_); data_ = std::exchange(other.data_, ts_mesh_view{}); }
        return *this;
    }
    ~MeshView() noexcept = default;
    const ts_mesh_view& data() const noexcept { return data_; }
};
class Mesh {
    friend class Scene;
    friend class Document;
    std::unique_ptr<ts_mesh, detail::MeshDeleter> handle_;
    explicit Mesh(ts_mesh* raw) noexcept : handle_(raw) {}
public:
    Mesh(const Mesh&) = delete;
    Mesh& operator=(const Mesh&) = delete;
    Mesh(Mesh&&) noexcept = default;
    Mesh& operator=(Mesh&&) noexcept = default;
    ~Mesh() noexcept = default;
    // Flat XYZ and index arrays are copied during creation; counts are records.
    static Result<Mesh> from_triangles(const double* xyz, size_t vertices,
        const uint32_t* indices, size_t triangles, const MeshOptions* options = nullptr) {
        ts_mesh* raw = nullptr;
        const auto status = ts_mesh_create(xyz, vertices, indices, triangles, options, &raw);
        Mesh mesh(raw);
        if (status != TS_OK) return detail::error(status);
        return mesh;
    }
    Result<MeshView> view() const {
        ts_mesh_view data{};
        auto status = ts_mesh_get_view(handle_.get(), &data);
        if (status != TS_OK) return detail::error(status);
        status = ts_mesh_retain(handle_.get());
        if (status != TS_OK) return detail::error(status);
        return MeshView(handle_.get(), data);
    }
};

inline Result<Mesh> Document::tessellate_faceted(uint64_t entity_id, double metres_per_unit,
    const FacetedOptions* options) const {
    ts_mesh* raw = nullptr;
    ts_import_error failure{};
    const auto status = ts_document_tessellate_faceted(handle_.get(), entity_id,
        metres_per_unit, options, &raw, &failure);
    Mesh mesh(raw);
    if (status != TS_OK) {
        auto error = detail::error(status);
        error.import_failure = {static_cast<ImportStage>(failure.stage), failure.entity_id,
            failure.start_offset, failure.end_offset};
        return error;
    }
    return mesh;
}

inline Result<Mesh> Document::tessellate_planar(uint64_t entity_id, double metres_per_unit,
    const PlanarOptions* options, const ImportPolicy* policy) const {
    ts_mesh* raw = nullptr;
    ts_import_error failure{};
    const auto status = ts_document_tessellate_planar_with_policy(handle_.get(), entity_id,
        metres_per_unit, options, policy, &raw, &failure);
    Mesh mesh(raw);
    if (status != TS_OK) {
        auto error = detail::error(status);
        error.import_failure = {static_cast<ImportStage>(failure.stage), failure.entity_id,
            failure.start_offset, failure.end_offset};
        return error;
    }
    return mesh;
}

using SceneOptions = ts_scene_options;
using SceneInstance = ts_scene_instance;
using SceneInstanceInfo = ts_scene_instance_info;
using SceneInfo = ts_scene_info;
inline SceneOptions default_scene_options() noexcept {
    SceneOptions options{}; ts_scene_options_init(&options); return options;
}
inline SceneInstance scene_instance(uint64_t id, uint64_t parent = 0, uint64_t asset = 0) noexcept {
    SceneInstance instance{}; instance.id=id; instance.parent_id=parent; instance.asset_id=asset;
    instance.linear[0]=instance.linear[4]=instance.linear[8]=1; return instance;
}
namespace detail {
struct SceneDeleter { void operator()(ts_scene* p) const noexcept { ts_scene_release(p); } };
}
class Scene {
    friend class Appearance;
    std::unique_ptr<ts_scene, detail::SceneDeleter> handle_;
    explicit Scene(ts_scene* raw) noexcept : handle_(raw) {}
public:
    // Borrowed only during create(); the scene retains each asset independently.
    struct Asset { uint64_t id; const Mesh* mesh; };
    Scene(const Scene&) = delete;
    Scene& operator=(const Scene&) = delete;
    Scene(Scene&&) noexcept = default;
    Scene& operator=(Scene&&) noexcept = default;
    ~Scene() noexcept = default;
    static Result<Scene> create(const std::vector<Asset>& assets,
        const std::vector<SceneInstance>& instances, const SceneOptions* options = nullptr) {
        std::vector<ts_scene_asset> inputs; inputs.reserve(assets.size());
        for (const auto& asset : assets) inputs.push_back({asset.id, asset.mesh ? asset.mesh->handle_.get() : nullptr});
        ts_scene* raw=nullptr;
        auto status=ts_scene_create(inputs.data(),inputs.size(),instances.data(),instances.size(),options,&raw);
        Scene scene(raw);
        if (status!=TS_OK) return detail::error(status);
        return scene;
    }
    Result<SceneInfo> info() const {
        SceneInfo info{}; auto status=ts_scene_get_info(handle_.get(),&info);
        if (status!=TS_OK) return detail::error(status);
        return info;
    }
    Result<SceneInstanceInfo> instance_at(size_t index) const {
        SceneInstanceInfo info{}; auto status=ts_scene_instance_at(handle_.get(),index,&info);
        if (status!=TS_OK) return detail::error(status);
        return info;
    }
    Result<Mesh> asset_mesh(uint64_t id) const {
        ts_mesh* raw=nullptr; auto status=ts_scene_asset_mesh(handle_.get(),id,&raw);
        Mesh mesh(raw);
        if (status!=TS_OK) return detail::error(status);
        return mesh;
    }
    Result<Mesh> bake(uint64_t instance, const MeshOptions* options = nullptr) const {
        ts_mesh* raw=nullptr; auto status=ts_scene_bake_instance(handle_.get(),instance,options,&raw);
        Mesh mesh(raw);
        if (status!=TS_OK) return detail::error(status);
        return mesh;
    }
};

using AppearanceOptions = ts_appearance_options;
using Material = ts_material;
using StyleBinding = ts_style_binding;
using ResolvedMaterial = ts_resolved_material; // material.id == 0 means unstyled.
using AppearanceInfo = ts_appearance_info;
enum class StyleScope : uint32_t {
    asset=TS_STYLE_ASSET, asset_face=TS_STYLE_ASSET_FACE,
    instance=TS_STYLE_INSTANCE, instance_face=TS_STYLE_INSTANCE_FACE
};
inline StyleBinding style_binding(StyleScope scope, uint64_t id, uint64_t material, uint64_t face=0) noexcept {
    return {{static_cast<uint32_t>(scope),0,id,face},material};
}
inline AppearanceOptions default_appearance_options() noexcept {
    AppearanceOptions options{}; ts_appearance_options_init(&options); return options;
}
namespace detail {
struct AppearanceDeleter { void operator()(ts_appearance* p) const noexcept { ts_appearance_release(p); } };
}
class Appearance {
    std::unique_ptr<ts_appearance,detail::AppearanceDeleter> handle_;
    explicit Appearance(ts_appearance* raw) noexcept : handle_(raw) {}
public:
    Appearance(const Appearance&) = delete;
    Appearance& operator=(const Appearance&) = delete;
    Appearance(Appearance&&) noexcept = default;
    Appearance& operator=(Appearance&&) noexcept = default;
    ~Appearance() noexcept = default;
    // Records are copied; the original Scene and its mesh buffers are retained.
    static Result<Appearance> create(const Scene& scene, const std::vector<Material>& materials,
        const std::vector<StyleBinding>& bindings, const AppearanceOptions* options=nullptr) {
        ts_appearance* raw=nullptr;
        auto status=ts_appearance_create(scene.handle_.get(),materials.data(),materials.size(),
            bindings.data(),bindings.size(),options,&raw);
        Appearance appearance(raw);
        if (status!=TS_OK) return detail::error(status);
        return appearance;
    }
    Result<AppearanceInfo> info() const {
        AppearanceInfo info{}; auto status=ts_appearance_get_info(handle_.get(),&info);
        if (status!=TS_OK) return detail::error(status);
        return info;
    }
    Result<Material> material_at(size_t index) const {
        Material material{}; auto status=ts_appearance_material_at(handle_.get(),index,&material);
        if (status!=TS_OK) return detail::error(status);
        return material;
    }
    Result<StyleBinding> binding_at(size_t index) const {
        StyleBinding binding{}; auto status=ts_appearance_binding_at(handle_.get(),index,&binding);
        if (status!=TS_OK) return detail::error(status);
        return binding;
    }
    Result<ResolvedMaterial> resolve_triangle(uint64_t instance, size_t triangle) const {
        ResolvedMaterial material{};
        auto status=ts_appearance_resolve_triangle(handle_.get(),instance,triangle,&material);
        if (status!=TS_OK) return detail::error(status);
        return material;
    }
    Result<Scene> scene() const {
        ts_scene* raw=nullptr; auto status=ts_appearance_get_scene(handle_.get(),&raw);
        Scene scene(raw);
        if (status!=TS_OK) return detail::error(status);
        return scene;
    }
};

}
#endif
