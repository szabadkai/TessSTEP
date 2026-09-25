# Architecture

Status: repository foundation, Part 21 vertical slice and EXPRESS frontend. Architecture is a
contract for later work, not a declaration that the geometry kernel already exists.

## Implemented dependency graph

```text
expressc ──→ tessstep-express

stepdump ──→ tessstep-model ──→ tessstep-part21
    └────────────────────────→ tessstep-part21

tessstep ──→ tessstep-express
    ├──────→ tessstep-model
    └──────→ tessstep-part21
```

`tessstep-part21` owns physical tokens, generic values, source locations, diagnostics,
and a pull parser over `BufRead`. It has no schema names, entity interpretation, or
geometry dependencies. The parser emits records and section events without retaining
previous records. It interns names, so memory also scales with distinct symbols.

`tessstep-model` collects events into a dense entity vector plus a BTreeMap from
strong physical IDs to private slots. IDs never become allocation sizes. It enforces
unique occurrence numbers across entity and value namespaces, preserves section
membership, and analyzes references separately. It never follows a graph recursively.
Document inspection is immutable. Public syntax structs describe raw data, not validity
states for future geometry.

`tessstep` reexports these explicit phases and the independent EXPRESS frontend. `stepdump` is an application
boundary: it owns formatting and exit status, not parsing policy. It writes JSON with
proper escaping and has independent JSON decoder tests. No production external
libraries are required for this slice.

`tessstep-express` is a separate standard-library-only frontend. It consumes explicitly
supplied UTF-8 source strings, with independent diagnostics and spans; it has no Part 21,
model, filesystem, network or geometry dependency. The lexer and declaration parser
produce a source-located AST. Compilation resolves names to declaration IDs, constructs
schema visibility/export tables, lowers attribute/type domains, and checks basic graph
and declaration invariants. Each IR declaration points back to its complete AST. The
compiler returns no IR on structural errors; opaque expressions and algorithm bodies
produce explicit unsupported diagnostics even when structural checks pass.

`expressc` owns bounded file reading, output formatting and exit policy. It never searches
for schemas implicitly. Its JSON is an inspection summary; the library owns the full AST
and IR. No physical instance schema-validation stage or code generation is introduced.

Milestone 2 architecture review: the new `expressc → tessstep-express` edge is isolated
from the physical-parser graph. Ordered maps and source-order IDs keep results stable
for the same ordered inputs. Import propagation uses a monotone fixed point; schema
cycles are legal. Inheritance/type cycle detection and ancestor traversal are iterative.
Recursive type parsing/lowering is hard-capped at 128. AST expansion, imported-name copies
and graph traversal consume a work budget. Worst-case import closure and inherited
attribute checks can be superlinear; they terminate with a resource diagnostic rather
than claiming linear complexity. Source/AST/IR retention is bounded by logical budgets,
not an exact RSS cap. No new production dependency or unsafe code was introduced.

## Planned boundaries

Only implemented crates are instantiated; empty geometry crates would imply capability
without providing it. The following names and responsibilities are reserved. Add each
crate when its first tested vertical slice is implemented.

| Crate | Responsibility |
|---|---|
| tessstep-codegen | deterministic Rust generation from supplied IR |
| tessstep-schema | runtime schema reflection and binding interfaces |
| tessstep-ap242 | generated bindings and explicit STEP-to-CAD adapters |
| tessstep-product | normalized products, representations, assembly instances |
| tessstep-math | STEP-independent units, spaces, transforms and tolerances |
| tessstep-curves / tessstep-surfaces | exact mathematical evaluators |
| tessstep-topology | typed handles, builders and immutable validity states |
| tessstep-trim | UV boundaries, periodic seams and trim reconstruction |
| tessstep-mesh | mesh assets and mesh invariants |
| tessstep-tessellate | tolerance-driven tessellation of normalized B-reps |
| tessstep-validate | structured semantic, topology and mesh reports |
| tessstep-io | exporters outside the kernel |
| tessstep-capi | supported public C ABI with versioned opaque handles and panic containment |

The supported public C++ wrapper sits above `tessstep-capi` and consumes only its
C contract. It provides RAII ownership, typed errors, and zero-copy read-only mesh
views where possible. Its installed CMake package exports `TessSTEP::TessSTEP`.
No Rust type, layout, allocator, panic, or ownership semantics may cross the ABI
boundary. Mesh storage must accommodate stable public read-only buffer formats
without exposing private kernel layouts. See [C_API.md](C_API.md) for ownership,
view lifetime, compatibility and consumer-test requirements. Both interfaces are
planned public deliverables; neither is implemented in the current parser slice.

The three data worlds remain separate: generic STEP instances, normalized CAD objects,
and mesh assets. A tessellator must not inspect raw STEP parameters. Geometry must be
constructible and testable without any STEP file. Units must be explicit at the semantic
boundary; physical reals have no presumed units.

`scripts/check_architecture.py` enforces direct dependency edges and unsafe policy on
all active crates. Every intermediate layer is checked, preventing indirect paths through
an unrestricted helper. New crates require an explicit boundary. The umbrella and C API
are intentional composition roots. Production dependency additions require policy review.

## Determinism and costs

Entity iteration follows source order. Type counts sort by name. Diagnostic and reference
reports sort by byte position. No result ordering relies on a randomized hash table.
Index insertion and lookup are O(log n). Lexing consumes the input once; parsing is linear
in tokens plus O(log s) name interning for s distinct names. Reference analysis performs
bounded traversal and O(log n) lookups, then sorts by source offset.

The streaming parser retains one record, reader buffer, bounded recursion and symbol
pool; the collector retains the full AST and indexes. Collection is not constant-memory.
Each parse has finite input, token, value, nesting, entity, section and record budgets.
There is no global mutable state, hidden healing, unsafe optimization, or CAD-kernel FFI.
