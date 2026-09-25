# Public C ABI and C++ wrapper

The C ABI and C++ wrapper are supported public interfaces. They carry the same
compatibility, documentation and release-testing obligations as the public Rust
API. This is a binding architectural requirement. Implementation status: neither
interface is exported in the current Milestone 0–2 delivery.

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

These are future implementation gates, not tests claimed by this documentation
change. See [ARCHITECTURE.md](ARCHITECTURE.md) and [CONFORMANCE.md](CONFORMANCE.md).
