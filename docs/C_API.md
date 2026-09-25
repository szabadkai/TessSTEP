# Public C ABI and C++ wrapper

The C ABI and C++ wrapper are supported public interfaces. They carry the same
compatibility, documentation and release-testing obligations as the public Rust
API. This is a binding architectural requirement. The first implementation exports
ABI 1 physical-document operations, a C++17 wrapper and a shared CMake package.
Schema decoding, geometry and meshes are not exposed through this interface yet.

## Using the first slice

The authoritative protocol is [tessstep.h](../include/tessstep/tessstep.h); the
header-only C++ wrapper is [tessstep.hpp](../include/tessstep/tessstep.hpp).
Build/install from source (Rust 1.85+, CMake 3.20+, C11/C++17 toolchain):

```sh
cmake -S . -B target/cmake -DCMAKE_INSTALL_PREFIX=/absolute/install/path
cmake --build target/cmake --config Release
cmake --install target/cmake --config Release
```

Consumers set `CMAKE_PREFIX_PATH` to that installation, then use:

```cmake
find_package(TessSTEP 0.1 CONFIG REQUIRED)
target_link_libraries(my_app PRIVATE TessSTEP::TessSTEP)
```

Prebuilt consumers need only CMake and their C/C++ toolchain, not Cargo. This slice
supports shared linkage on 64-bit Linux (GCC/Clang), Windows (MSVC 2022), and macOS
(Apple Clang). Debug and Release consumers link the same release ABI library; no
CRT allocation ownership crosses the boundary. Static linkage is not provided.
Windows applications must make the installed `bin` directory available to the DLL
loader (for example through PATH, or by placing the DLL beside the executable).
Package relocation and compiler/platform behavior are CI gates; local verification
is macOS arm64 only. See VALIDATION.md for observed results.

```cpp
#include <tessstep/tessstep.hpp>
// bytes contains a complete physical STEP file, read by the application.
auto parsed = tessstep::Document::parse(bytes);
if (!parsed) {
    // parsed.error().code and .diagnostics own their structured error details.
    return 1;
}
auto document = std::move(parsed).value();
auto info = document.info();
auto report = document.diagnostics(); // Separate reference/trust analysis.
```

`ts_document_parse` borrows bytes only during the call, returns a complete immutable
document or a typed failure, and accepts explicit budgets initialized by
`ts_parse_options_init`. `ts_document_get_info`, `ts_document_entity_at`, and
`ts_document_record_name` inspect counts, source-order IDs and component names.
C names are borrowed without copying; C++ `record_name` explicitly returns owned
text. Reference analysis returns an independent report that survives document
release. A successful analysis can contain error diagnostics. Parse success never
means schema or CAD validity.

Each document acquisition/retain requires `ts_document_release`; every diagnostic
report requires `ts_diagnostics_release`. C++ wraps both paths in RAII, including
when C++ string/vector allocation throws. C++ document copies are disabled; moves
and destructors are nonthrowing. Queries on moved-from documents return a typed
invalid-argument result. Diagnostics and names returned by C++ are owned copies.
There are no callbacks, global last-error slots, file access or mesh placeholders.

ABI 1 freezes the header's fixed-width status values and record layouts. Options
must have the exact size and ABI version initialized by the library; zero budgets
mean zero. Future incompatible records receive new names, never appended fields.
NULL, empty input, failure outputs, borrowed-text lifetimes and concurrent reads are
specified in the header. C opaque handles never expose Rust storage layouts.
Unwinding panics become internal failures; an unexpected panic payload is deliberately
forgotten to prevent a panicking destructor from escaping. Aborts and allocation
termination remain unrecoverable. Release paths have no expected panic sources.

## ABI boundary

No Rust type, layout, allocator, panic, or ownership semantics may cross the ABI
boundary. Define the protocol in a public C header: opaque handles, explicitly
specified C-compatible scalar fields, pointer/count buffers, stable status codes,
and an ABI version query such as `ts_api_version`. Any exposed record has a
C-defined layout and versioning contract; it is not an exported kernel struct.
Rust bridge types implement that contract privately.

Do not export Rust references, slices, enums, `Vec`, `String`, `Box`, trait objects,
or implementation-dependent layouts. The C++ wrapper calls only the public C ABI;
it does not inspect opaque handles or depend on Rust symbols or object layouts.
Isolate necessary unsafe bridge code in `tessstep-capi`, with SAFETY invariants
and focused tests. Kernel crates continue to forbid unsafe code.

## Allocation, ownership and lifetime

Every function documents whether each handle or buffer is borrowed, retained,
transferred, or newly created in terms of the C protocol. Acquired handles have
matching library release operations. Library-owned memory is released only by
the library; callers never use `free`, `delete`, or their allocator on it. Caller
buffers remain caller-owned. Copying a C handle value does not acquire ownership.

Input pointers have explicit validity, count and duration requirements. Output
parameters have defined failure states; a failed operation must not leave a
partially acquired object or require callers to understand Rust drop behavior.
Text has documented encoding and length. Define null/empty cases, thread-safety,
handle retention, and destruction order in the header before implementation.

C++ owning types release handles automatically, support safe moves, and have
nonthrowing destructors. Copies are disabled unless a documented retain or clone
operation implements their ownership. Failed construction and result propagation
must be exception-safe and leak-free. No manual Rust lifetime management is
required of a C or C++ consumer.

## Read-only mesh views

Provide zero-copy mesh access where immutable storage can satisfy the public
buffer format. The C interface exposes const pointers and counts with documented
scalar types, strides, index widths, alignment, and validity. Avoid reinterpreting
private Rust point/vector structs as public data. Plan compatible contiguous
storage in the mesh layer so ordinary access does not require a conversion.

A C++ mesh-view object exposes read-only ranges, such as `std::span<const T>` when
supported by the chosen C++ baseline. The view retains an owning reference to the
opaque mesh storage so destroying the document or original mesh wrapper does not
invalidate the view. Extracted raw pointers or spans remain borrowed from that
view; they must not outlive its retained storage. Specify this rule explicitly.

Mesh storage must remain immutable and address-stable while views exist, including
during concurrent reads. Requests requiring repacking or conversion use an explicit
owned-copy API with documented cost. Do not hide copies behind a zero-copy promise.

## Errors and panic containment

C operations return stable typed status codes and explicit diagnostic access.
Diagnostics have documented ownership and lifetime; they never expose a Rust
error object or borrowed formatter state. Distinguish expected invalid input,
unsupported functionality, resource limits, and internal failures.

C++ maps these into typed error codes and result objects with structured diagnostic
information. If an exception convenience API is added, its typed exceptions are
constructed entirely in C++; no exception or panic may unwind through the C ABI.

Convert expected failures into results before the boundary. Where Rust unwinding
is enabled, contain unexpected panics inside each exported operation and translate
them into an internal-error status without publishing partial output or reusing
invalid state. Release paths must remain nonpanicking. Process aborts and allocator
termination are not recoverable errors; the implementation must not claim that
panic containment can recover them. Callback designs, if introduced, must also
prevent C++ exceptions from entering Rust.

## CMake package

Install a relocatable CMake package with a version file, public headers, libraries,
and the imported target `TessSTEP::TessSTEP`. The intended consumer interface is:

```cmake
find_package(TessSTEP CONFIG REQUIRED)
target_link_libraries(my_app PRIVATE TessSTEP::TessSTEP)
```

The target supplies include paths, its required C++ language level, the C ABI
library, and platform link dependencies transitively. Consumers of a prebuilt
package do not need Cargo or knowledge of Rust linkage. Publish the supported
C/C++ compiler baseline and shared/static build configurations. Verify exported
symbols, visibility, runtime dependencies, and Debug/Release configuration mapping
on Linux, Windows and macOS.

## Compatibility and acceptance evidence

Version the C ABI from its first release. Prefer additive evolution; define size
and version handling before exposing extensible option records. The C++ API and
CMake target are public compatibility surfaces, not internal implementation aids.

Before marking either interface available, compile and run standalone C and C++
consumers against the installed package, without source-tree paths or Rust tooling.
Test status/diagnostic mapping, null/empty cases, acquire/release symmetry, moves,
construction failures, destruction order, panic containment, and concurrent reads
where supported. Mesh tests must verify pointer stability, view lifetime after
parent destruction, and the absence of copies on documented zero-copy paths.
Exercise relocation and `find_package` through `TessSTEP::TessSTEP` on all supported
platforms, with sanitizer and ABI/layout checks where applicable.

The document subset is exercised by `cargo test -p tessstep-capi` and
`python3 scripts/check_capi.py`. The latter installs and relocates the package,
compiles/runs independent C11 and C++17 consumers in Debug/Release with Rust tool
invocations blocked, and verifies the exact exported symbol set. Release packaging
runs those same consumers. Native consumer ASan/UBSan is available with
`--sanitizers`; this does not instrument the Rust library. Mesh lifetime and zero-copy
acceptance tests remain future gates. See [ARCHITECTURE.md](ARCHITECTURE.md) and
[CONFORMANCE.md](CONFORMANCE.md).
