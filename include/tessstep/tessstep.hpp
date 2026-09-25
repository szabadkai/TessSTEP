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
    not_found = TS_NOT_FOUND, internal_error = TS_INTERNAL_ERROR
};
enum class Severity : uint32_t { error = TS_SEVERITY_ERROR, warning = TS_SEVERITY_WARNING };
struct Diagnostic {
    Severity severity;
    uint64_t entity_id, start_offset, end_offset, line, column;
    std::string code, message;
};
struct Error {
    ErrorCode code;
    std::vector<Diagnostic> diagnostics;
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
}
#endif
