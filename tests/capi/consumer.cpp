#include <tessstep/tessstep.hpp>
#include <cassert>
#include <future>
#include <type_traits>
using namespace tessstep;
static_assert(!std::is_copy_constructible_v<Document>);
static_assert(std::is_nothrow_move_constructible_v<Document>);
static_assert(std::is_nothrow_move_assignable_v<Document>);
static_assert(std::is_nothrow_destructible_v<Document>);
constexpr auto input = "ISO-10303-21;HEADER;FILE_DESCRIPTION(('test'),'2;1');"
    "FILE_NAME('','',('a'),('o'),'','','');FILE_SCHEMA(('EXAMPLE'));ENDSEC;DATA;"
    "#9=ITEM(#42);ENDSEC;END-ISO-10303-21;";
int main() {
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
