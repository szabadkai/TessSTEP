# Architecture

Status: repository foundation, Part 21 vertical slice, EXPRESS frontend, Rust generation
and runtime reflection, plus a bounded product semantics slice, checked math and
independent analytic/NURBS curves and surfaces, structural topology, supplied-pcurve UV
reconstruction, shared-edge sampling, adaptive regular-face tessellation and owned
manifold shell/solid meshes. STEP geometry adaptation and industrial hardening remain pending.

## Implemented dependency graph

```text
expressc ──→ tessstep-codegen ──→ tessstep-express
    └─────────────────────────→ tessstep-express

generated consumer ──→ tessstep-schema (standard library only)

tessstep-capi ──→ tessstep-model / tessstep-part21 / tessstep-mesh
C++ wrapper ──→ public C ABI only

stepdump ──→ tessstep-model ──→ tessstep-part21
                  └─────────→ tessstep-schema
    └────────────────────────→ tessstep-part21

tessstep-ap242 ──→ tessstep-model / tessstep-schema / tessstep-part21
       └────────→ tessstep-product (standard library only)

tessstep-surfaces ──→ tessstep-curves ──→ tessstep-math (standard library only)
         └───────────────────────────→ tessstep-math

tessstep-tessellate ──→ tessstep-trim ──→ tessstep-topology
        └────────────────────────────→ tessstep-topology
        └────→ tessstep-mesh ──→ tessstep-math
        └────→ tessstep-math / tessstep-curves / tessstep-surfaces
tessstep-trim / tessstep-topology ──→ tessstep-math / tessstep-curves / tessstep-surfaces

tessstep ──→ tessstep-topology / tessstep-trim / tessstep-tessellate / tessstep-mesh
    └──────→ tessstep-surfaces / tessstep-curves / tessstep-math / tessstep-ap242 / tessstep-product
    ├──────→ tessstep-codegen
    ├──────→ tessstep-schema
    ├──────→ tessstep-express
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
and IR. It also selects Rust generation and its output/error policy; it does not decode
physical instances against a schema.

Milestone 2 architecture review: the new `expressc → tessstep-express` edge is isolated
from the physical-parser graph. Ordered maps and source-order IDs keep results stable
for the same ordered inputs. Import propagation uses a monotone fixed point; schema
cycles are legal. Inheritance/type cycle detection and ancestor traversal are iterative.
Recursive type parsing/lowering is hard-capped at 128. AST expansion, imported-name copies
and graph traversal consume a work budget. Worst-case import closure and inherited
attribute checks can be superlinear; they terminate with a resource diagnostic rather
than claiming linear complexity. Source/AST/IR retention is bounded by logical budgets,
not an exact RSS cap. No new production dependency or unsafe code was introduced.

## Milestone 3 architecture review

`tessstep-codegen` depends only on `tessstep-express`. It traverses resolved domains and
linked source AST constraints and emits Rust through a byte-bounded writer. Schema and
declaration order follow the frontend; symbol maps are ordered. Ancestor expansion is
iterative and work-bounded, with diamond deduplication. Expanding inherited fields can
be superlinear; no linear-time or exact-memory guarantee is claimed. Nested domains
have a hard depth cap. Invalid compilation, output, work and nesting failures are typed.

`tessstep-schema` depends only on the standard library. Generated consumers need this
runtime crate, not the compiler frontend. Metadata consists of static slices, strings
and resolved IDs, with borrowed, allocation-free lookups. Owned records and nonzero typed
references are unchecked representation types, separate from the generic physical model.
Constraints and unsupported semantics remain visible in metadata; no EXPRESS expression
is executed. No runtime source recompilation, global state, unsafe code or new external
production dependency was added. Rust bindings are not the public C ABI; that boundary
and the future C++ ownership contracts remain unchanged.

## Planned boundaries

Only implemented crates are instantiated; empty geometry crates would imply capability
without providing it. The following names and responsibilities are reserved. Add each
crate when its first tested vertical slice is implemented.

| Crate | Responsibility |
|---|---|
| tessstep-ap242 (active) | schema-decoded product adapters now; geometry adapters later |
| tessstep-product (active) | independent owned products, representations and assembly graphs |
| tessstep-math (active) | STEP-independent units, spaces, transforms and tolerances |
| tessstep-curves (active) | analytic/NURBS curves, bounded spline basis and insertion |
| tessstep-surfaces (active) | analytic and tensor-product NURBS surface evaluation/refinement |
| tessstep-topology (active) | typed handles, builders and immutable validity states |
| tessstep-trim (active) | UV boundaries, periodic seams and trim reconstruction |
| tessstep-mesh (active) | owned indexed assets, per-corner attributes and manifold invariants |
| tessstep-tessellate (active) | shared-edge sampling, adaptive regular faces and shell/solid assembly |
| tessstep-validate | structured semantic, topology and mesh reports |
| tessstep-io | exporters outside the kernel |

The supported public C++ wrapper sits above `tessstep-capi` and consumes only its
C contract. It provides RAII ownership, typed errors, and zero-copy read-only mesh
views where possible. Its installed CMake package exports `TessSTEP::TessSTEP`.
No Rust type, layout, allocator, panic, or ownership semantics may cross the ABI
boundary. Mesh storage must accommodate stable public read-only buffer formats
without exposing private kernel layouts. See [C_API.md](C_API.md) for ownership,
view lifetime, compatibility and consumer-test requirements. Physical documents and owned mesh import/read-only views are implemented in both
interfaces; schema decoding and B-rep/tessellation entry points remain future work.

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

## Milestone 4 initial decoder review

`tessstep-model → tessstep-schema` is now active; the physical parser and reflection
runtime remain independent standard-library-only crates. Generic document storage is
unchanged. An explicit `model::decode` phase borrows the physical document and supplied
metadata, resolves identities before values, and publishes immutable views only on
success. No schema compiler or generator is added to the runtime dependency graph.

Hierarchy and reference-compatibility walks are iterative; nested domains/values have
a hard recursion ceiling of 128. Work budgets cover metadata traversal and retained
attribute expansion. References are checked by identity without graph recursion. Sparse
ordered indexes and source-order output preserve deterministic behavior. Logical work
limits do not promise exact RSS/time bounds; symbol searches and repeated inheritance
expansion can be superlinear. Unsupported semantics are typed failures, not warnings
attached to successful validation. See DECODING.md for the supported subset and lifetime
contract. This addition contains no unsafe code or third-party dependency and exposes
no Rust representation through C/C++ interfaces.

## Public-interface slice architecture review

`tessstep-capi` depends on the model and physical parser only, with no third-party
production dependencies. It translates kernel data into explicitly C-defined records,
keeps immutable documents behind retained opaque handles, and gives reports independent
ownership. Source-order entity summaries add O(n) storage and permit O(1) index access;
ID/component-name lookup uses the model's O(log n) index. Reference diagnostics are
computed explicitly per request. Names are borrowed in C and explicitly copied in C++.

Only this bridge permits unsafe code, with local SAFETY invariants and
`deny(unsafe_op_in_unsafe_fn)`; all kernel crates still forbid it. Output slots are
cleared before validation and handles published only after fallible work completes.
An unwind guard contains unexpected panics without assuming allocator failures or
process aborts are recoverable. The C++ wrapper knows only the C protocol. Installed
consumer, layout, exported-symbol, relocation and sanitizer checks are release gates.
The initial package contains a shared library; static linkage and mesh storage remain
unimplemented. The dependency checker now covers cdylib and rlib targets explicitly.

## Milestone 4 structural decoder review

No new crate or production dependency is introduced. Model decoding now resolves complete
inheritance memberships before attribute validation. Iterative postorder traversal preserves
parent order and deduplicates diamonds; active-path tracking rejects cycles. External
mappings preserve partial-entity owners and check component completeness/order/connectivity.
Reference compatibility uses precomputed memberships and never expands instance graphs.

SELECT traversal is iterative and depth/work bounded. Aggregate comparisons use checked
numeric equivalence and budgeted ordered/unordered matching; worst-case uniqueness is
quadratic. Integer bound parsing is private to the decoder, depth-bounded, overflow-checked
and explicitly smaller than an EXPRESS evaluator. Recognized expression spans reconcile
frontend unsupported diagnostics without mutating or suppressing the source metadata.

The generator emits a validator driver as bounded source text; filesystem/CLI behavior
lives only in that generated application. It adds no model dependency to the compiler
runtime. The corpus runner optionally invokes a snapshot of this executable, validates
its bounded JSON protocol and keeps schema outcomes separate from physical/reference
outcomes. The schema stage and validator identity are recorded without altering the
reviewed physical baseline. C ABI layouts, symbols and ownership contracts are unchanged.

## Milestone 5 initial product architecture review

This historical snapshot is superseded by the Milestone 5 completion review below
and the current [product contract](PRODUCT_MODEL.md).

`tessstep-product` has no dependencies and exposes validated immutable graphs built
from caller-owned inputs. Its typed IDs are sparse identities, not offsets. Unit values
are immutable positive SI scales; no STEP interpretation or geometry evaluator lives
here. Assembly and mapped-use cycle checks use iterative topological traversal,
include disconnected components and preserve shared definitions. No occurrence tree
is expanded. Placements retain ordered descriptions and explicit absence.

`tessstep-ap242` composes supplied decoded metadata, physical values and the product
model. The dependency checker explicitly permits its Part 21 value-type dependency;
no reverse edge enters the parser, decoder or schema runtime. Role identity comes
from caller-selected schema symbols and decoded memberships, with no hand-encoded
AP inheritance tree. The crate name does not imply AP conformance. Units are resolved
iteratively with dimension, cycle, depth and numeric checks.

Traversal, attribute scans and copied text have finite budgets. Graph validation
consumes remaining adapter work, and only successful complete models are published.
Ordered maps preserve determinism. Membership scans can be superlinear; this is not
an exact RSS/time guarantee. Typed identities and located errors retain provenance.
Opaque items still need the original document for later geometry interpretation.
No unsafe code, third-party production dependency, global state, external fetching
or ABI layout change was added.

The corpus product stage follows schema acceptance, with a separate executable hash,
timeout and protocol check. Older baselines without this stage normalize to
`not_implemented`; this does not rewrite the baseline or claim product acceptance.
See PRODUCT_MODEL.md for direct context/shape membership restrictions and unevaluated
placement semantics.

## Milestone 6 math architecture review

`tessstep-math` has no dependencies and is reexported by the umbrella crate. The
existing architecture checker already reserves this boundary. No edge is added from
the physical parser, schema runtime, product model or adapter to the math crate yet.
Product unit interpretation stays in the semantic layer; math receives explicit scales.

Private finite value storage and frame type parameters separate coordinate validity
from transform invertibility. Construction permits finite singular affine maps;
inversion and normal operations check numerical pivots and fail explicitly. Model,
tessellation and numerical tolerances are separate value types. Normalized direction
storage is distinct from general vectors. All 3D operations use fixed stack arrays;
coordinates use caller-selected compile-time dimensions. There are no heap allocations,
recursion, mutable global state, unsafe blocks or new third-party production dependencies.

The crate has no raw STEP entity access, topology validity claims or mesh layout. Its
Rust representations are not exposed across the C ABI. Physical C/C++ operations,
layouts and package ownership are unchanged; public geometry bindings remain pending.
The corpus stages remain honest: math tests provide no STEP geometry acceptance evidence.

## Milestone 7 analytic curve architecture review

`tessstep-curves` depends only on `tessstep-math`; the umbrella reexports it. The
existing dependency checker already permits this edge. No physical/schema/product
layer now depends on curves, and no curve evaluator reads raw STEP parameters.
Private basis/frame storage guarantees supported dimensions and constructor invariants.
Analytic kind selection is exhaustive; evaluation publishes position and two derivatives
only when all three succeed. Curve regularity is not implied by finite storage.

Plane frames normalize explicit caller axes and reject parallel inputs. Curves preserve
parameterization; parameter spans are a separate fixed-size wrapper with explicit
orientation and chain-rule derivatives. Affine evaluation transformation does not
reclassify the analytic basis. All operations have fixed work and stack storage, with
no recursion, heap allocation, global state, unsafe blocks or external dependencies.
Frame tags propagate through points, vectors and affine derivative transformations.

The geometry corpus stage remains unimplemented because there is no schema-to-curve
adapter/checker. Independent constructed-geometry tests are not reported as successful
STEP geometry validation. No C symbols, ABI layouts or ownership contracts changed;
public C/C++ curve operations remain pending under the existing interface requirement.

## Milestones 8–10 geometry architecture review

The new surface crate depends only on math and curves. Shared knot validation and
iterative basis evaluation belong to the curve crate; surfaces add tensor products,
quotient differentiation and net refinement without duplicating basis algorithms.
No physical/parser/schema/product edge enters these evaluators. Analytic constructors
use explicit frames; private spline storage preserves degree, knot, count and positive
weight invariants. Derivative regularity is separate from successful finite evaluation.

Analytic evaluation uses fixed stack data. Splines own bounded knot/control/weight vectors;
evaluation uses fixed degree-limited stack tables and no heap allocation or recursion.
Single knot insertion allocates a bounded replacement, with original storage immutable.
The surface weight normalization is global to the net, never independent per row/column.
Limits are logical bounds, not an exact RSS contract. No unsafe code, external numerical
library, mutable global state or C ABI layout/symbol was added.

Curve/surface derivatives carry frame types, while one-sided knot selection and singular
normal results remain explicit. No finite result certifies topology, continuity at a
repeated knot, or industrial numerical robustness. External corpus stages stay unchanged
until a schema adapter and real geometry checker exist. C/C++ geometry operations remain
pending and must follow the existing public interface and release-testing contract.

## Milestones 11–13 boundary pipeline architecture review

The three new crates follow the reserved downward graph. Topology owns constructed
curve/surface geometry and private validity states; trimming consumes normalized
objects; tessellation consumes normalized objects and the trim chart contract.
No layer reads raw STEP, schema records or product graphs. Typed model-local indices
separate topology kinds. Public immutable access cannot mutate a checked model.
The shared-edge cache borrows its normalized source; face boundaries borrow the
cache's actual position samples, preventing independent per-face resampling cracks.

Validation and refinement use iterative walks/stacks, ordered incidence maps and
finite logical budgets. Canonical normalization preserves source geometry and index
order, retaining the incidence cache computed at validation. UV intersection checks
are quadratic with charged work. Refinement uses knot/period seeds and sampled errors;
no global continuous accuracy bound or exact predicates are inferred. Curve geometry
and validation allocations remain private Rust storage, not C buffer layouts.
No unsafe code, mutable global state or third-party production dependency is added.

Structural topology validity, sampled trim validity, shared-edge sample identity and
future mesh validity are distinct contracts. Open/unsewn shapes remain explicit;
solid handles currently mean a structurally closed single shell, not a certified
volume. STEP adapters, triangle assets and C/C++ geometry APIs are still absent.
The stable physical-document C ABI and installed C++ package remain release-tested;
no new symbol, allocator or Rust representation crosses their boundary. External
corpus stages remain unimplemented until real STEP adapters/checkers exist.

## Milestone 14 planar triangulation architecture review

Planar triangulation remains in `tessstep-tessellate`; the already permitted surfaces
edge moves from test-only to production use for explicit analytic-plane discrimination.
There is no new crate, external dependency or unsafe code. Trimming owns the reusable
polygon validity contract; reconstruction and actual-resolution validation share hole
relationship checks. Tessellation never inspects physical or schema entities.

Face results borrow canonical cached positions and own only UV/sample-reference records,
u32 indices, loop indices and a normal. Public access is immutable; no general owned
mesh asset layer is claimed. Bridge/ear operations are deterministic and iterative.
Finite logical budgets bound potentially cubic work; incidence postchecks use ordered
maps. No boundary vertex is healed, welded or discarded, and failure publishes no mesh.
Exact geometric robustness and triangle quality are separate future responsibilities.

At Milestone 14, public C/C++ mesh buffers were still unimplemented; the bridge must translate into
explicit C-defined storage and retain its owner rather than expose these Rust references.
The existing C ABI has no symbol, allocator, layout or ownership changes from this work.
Independent constructed-face tests do not enable the external STEP geometry or
tessellation corpus stages without an actual schema-to-geometry adapter/checker.

## Milestones 15–17 adaptive mesh pipeline architecture review

`tessstep-mesh` activates the reserved mesh/math boundary. It owns immutable flat arrays
and validates oriented manifold incidence without any STEP, topology or surface dependency.
The tessellator composes normalized topology, UV constraints, surface evaluation and mesh
validation. Its shared UV triangulator also continues to serve the borrowed planar API.
No parser/schema layer acquires a geometry dependency.

Adaptive refinement is iterative and conforming. A global driver gathers boundary split
requests and resamples canonical edges before rebuilding selected faces; interpolation
is never independently performed on two uses of the same boundary. Position identity
uses topology vertices and edge sample ordinals, not proximity. Per-corner normals/UVs
retain discontinuities after position welding. NURBS cell clipping and one-sided probes
avoid missing entire knot spans, while retaining an explicit sampled-error contract.

All maps and iteration orders are deterministic. Separate sample/trim/triangulation,
adaptive and final-mesh budgets are explicit. Output is private until complete success;
no healing, recursive graph expansion, unsafe kernel code or external numerical dependency
was added. Closed-edge endpoint incidences are now distinguished in topology vertex links.
Watertightness is a manifold incidence property, not a nonintersection or cavity proof.

The C composition root adds only a mesh dependency. Mesh creation copies scalar inputs,
validates them and copies into explicit C scalar storage behind a retained opaque handle.
Queries return address-stable read-only pointers without copying. C++ views independently
retain ownership. Five additive C functions and one status extend ABI 1 without changing
existing layouts, symbols or ownership; installed C/C++ consumers validate new layouts,
retention, moves, concurrent reads, failure cleanup and relocation. No Rust kernel layout,
allocator or lifetime semantics cross that boundary. Public B-rep/tessellation inputs
and STEP geometry adaptation remain distinct future interfaces.

## Milestone 18 assembly asset architecture review

`mesh::scene` extends the existing mesh/math boundary without new dependency edges.
It has no physical/schema/product access: callers supply asset identity and metre-based
local placements. Immutable Arc-owned assets are shared across occurrences. Ordered
sparse indexes and iterative breadth-first forest traversal preserve input order and
bound graph work/depth without expanding repeated subassembly definitions. Transform
composition, inverse-transpose caching and scaled handedness checks use checked math.
Explicit baking preserves asset-local face IDs, fixes reflected corner order and consumes
its remaining budget in mesh validation. Scene construction makes no world-mesh validity
or tolerance claim. No unsafe kernel code or external dependency is added.

The C bridge retains the validated kernel mesh alongside its existing C scalar buffers,
adding one kernel copy per imported asset, never per occurrence. A scene retains original
C handles and kernel Arcs, so asset queries return the same address-stable buffers without
repacking. No raw input pointer is retained without acquiring library ownership. Eight
additive functions and TS_INVALID_SCENE extend ABI 1 with separate frozen scalar records;
existing layouts and codes are unchanged. C++ Scene and acquired Mesh objects use RAII.
Installed consumers check independent lifetimes, pointer identity, moves, layouts, error
outputs and concurrent reads. STEP product/placement semantics and external geometry
corpus stages remain separate, unimplemented adapter work.

## Milestone 19 appearance architecture review

`mesh::appearance` stays within the existing mesh/math boundary and reads only an
immutable, explicitly retained scene. It owns a validated palette and sparse exact-ID
bindings; it neither duplicates geometry nor allocates per-occurrence triangle arrays.
Asset face sets are cached only during binding validation. Iterative forest traversal
resolves nearest occurrence overrides once, preserving ancestor provenance. Triangle
queries use ordered lookups without allocation, recursion or mesh scans. Separate limits
cover materials, bindings, face scans and traversal; they are logical, not exact RSS caps.
No appearance rule is inferred from STEP data, and no renderer/texture/shader dependency
or unsafe kernel code is introduced. Previous Rust mesh and scene record layouts and
constructor contracts remain unchanged.

C appearance handles retain the original scene handle and share its Arc-owned kernel
scene. This internal ownership adjustment adds no scene or mesh copy. New operations
translate C scalar input/output records without exposing Rust vectors, Arc, enums or
color layouts. Nine additive functions and TS_INVALID_APPEARANCE extend ABI 1; all
previous symbols, records and status meanings remain intact. C++ RAII and independent
scene/mesh acquisitions preserve the existing zero-copy mesh-view contract. Installed
consumers verify destruction order, copied appearance inputs, original geometry pointer
identity, concurrent queries, moves, failure clearing and layout/export compatibility.

## Milestone 5 completion review

The previously initial product slice now adds `tessstep-product → tessstep-math` and
`tessstep-ap242 → tessstep-math`, already permitted by the dependency checker. No STEP
access enters math or mesh. The adapter evaluates frame/operator placements only after
structural schema decoding; geometry items remain opaque. Each origin and uncertainty
is normalized with its own unit; dimensionless operator scale is not a unit conversion.

Context membership is an iterative closure through representation-item references,
without crossing representation-map source boundaries. Shape ownership propagates only
through untransformed shape associations. Assembly and mapping cycle checks remain
separate. Explicit expansion tracks occurrence paths by parent index, handles endpoint
direction, and rejects missing/ambiguous placements. Instance/work/depth budgets bound
output before expansion; no recursive tree copying or hidden identity transform occurs.

All new graph scans, reference traversal and copies consume logical budgets; graph
validation uses remaining adapter work. Checked affine construction, inversion and
composition enforce finite/numerical limits. Structured errors preserve source spans
and math/graph kinds. Owned output remains immutable; no new dependency, unsafe code,
global state, external fetch or C ABI change was introduced. Source descriptors and
evaluated maps remain separate; independent caller graphs can retain missing placements,
which expansion explicitly rejects. Corpus acceptance now checks evaluated matrices
and uncertainty values as well as graph counts. Full AP validation and STEP geometry
adaptation remain distinct stages.
