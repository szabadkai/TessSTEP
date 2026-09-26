# Milestones

**Integration update:** selected planar `FACETED_BREP` and straight-edge
`MANIFOLD_SOLID_BREP` → validated geometry → owned
solid mesh is now available in Rust/C/C++. See [STEP_IMPORT.md](STEP_IMPORT.md) for
explicit units, reduced-profile scope and evidence. The milestone records below
describe their delivery-time scope; general curved STEP adaptation remains pending.

This delivery implements the Milestone 0 foundation, Milestone 1 physical-parser
vertical slice, Milestone 2 EXPRESS frontend, Milestone 3 Rust bindings/reflection and
Milestone 4 structural schema decoding, each with explicit conformance limits.
Milestone 5 delivers bounded local 3D product, representation, unit and assembly semantics.
Milestone 6 now provides the independent checked math foundation.
Milestones 7–10 add independent analytic curves/surfaces and NURBS curves/surfaces.
Milestones 11–17 add structural topology, supplied-pcurve UV trimming, shared-edge sampling, planar/regular curved-face refinement and owned manifold shell/solid meshes.
Milestone 18 adds shared assembly mesh assets and explicit nested placements.
Milestone 19 adds explicit color/opacity and inherited appearance overrides.
STEP geometry adaptation and industrial hardening are not claimed. Read ARCHITECTURE.md, CONFORMANCE.md and the existing tests before starting every milestone. Finish code,
tests, fmt/clippy, applicable corpus/fuzz/bench runs, documentation, conformance updates
and architecture review before declaring work done.

**Milestone 2 delivered:** `tessstep-express` and `expressc` provide a bounded lexer,
source-located AST, resolved schema IR, USE/REFERENCE dependency handling and basic
semantic validation. Original small `.exp` fixtures cover entities, types, inheritance,
aggregates, SELECT/enumeration, attributes, DERIVE/INVERSE/WHERE/UNIQUE, constants and
imports. Expressions and algorithm declarations remain explicit opaque source with
unsupported diagnostics. Malformed-input tests, limits, deterministic fuzz smoke,
a libFuzzer target and compiler benchmarks are included. See EXPRESS.md for the exact
subset and VALIDATION.md for observed checks. No AP242 hierarchy was hand encoded.

**Milestone 3 delivered:** `tessstep-codegen` generates deterministic owned Rust records,
nominal types, enums, typed entity references and immutable `tessstep-schema` metadata
from successful supplied compilations. `expressc --rust` emits source with explicit
unsupported-semantic diagnostics. Consumer tests compile/run generated code and check
that unrelated entity references cannot be assigned. Inheritance, import identity,
anonymous domains, constraint preservation, generation budgets, fuzz smoke and a benchmark
are covered. See SCHEMA.md for the contract and VALIDATION.md for observed checks.

**Milestone 4 delivered: structural instance decoding with explicit validation limits.**

The decoder supports internal/external complex mappings, multiple inheritance, SELECT
encodings, aggregate bounds/uniqueness, local reference compatibility and bounded integer
bound expressions. It supplies borrowed views, typed errors and finite resource budgets.
`expressc --validator` emits a standalone schema checker; corpus reporting now supports
an explicit schema stage with authored positive/negative fixtures. General EXPRESS rules,
algorithms and AP conformance remain unsupported rather than silently accepted; see
[DECODING.md](DECODING.md) for the contract and VALIDATION.md for observed evidence.

**Milestone 5 delivered — bounded local 3D products, representations, units and assemblies.**

The owned product graph now evaluates schema-defined axis2 placements and Cartesian
operators using the independent math layer. Contextual item closure, transitive shape
associations, explicit SI units and positive uncertainty measures are supported.
Representation maps retain reusable sources and evaluated placements. Explicit bounded
assembly expansion preserves repeated definitions, parent paths and world transforms;
missing or ambiguous placements fail instead of inventing an identity or alternative.
Eleven authored corpus fixtures check schema/product outcomes and numerical placements.
See [PRODUCT_MODEL.md](PRODUCT_MODEL.md) for scope: full AP configurations, external
assemblies, non-immediate/quantified usage forms and automatic STEP mesh adaptation
remain separate unsupported capabilities, not Milestone 5 acceptance claims.

**Milestone 6 delivered — independent checked math foundation.**

`tessstep-math` provides finite space-tagged points/vectors, normalized directions,
explicit lengths/angles/unit conversion, separate model/tessellation tolerances and
typed affine transforms. Composition, inversion, normal transformation, axis-angle
rotation and explicit right-handed frames have independent regression, property,
compile-fail and arbitrary-bit tests plus fuzz/benchmark harnesses. Numerical limits
are explicit in [NUMERICAL_ROBUSTNESS.md](NUMERICAL_ROBUSTNESS.md). This foundation does
not implement exact predicates, STEP placement defaults/adapters or curve/surface
geometry. Milestone 7 now supplies independent analytic curve evaluation; Milestone 5 now uses the math foundation for checked placement adaptation.

**Milestone 7 delivered — independent analytic curves.**

`tessstep-curves` evaluates 2D/3D lines, circles, ellipses, parabolas and hyperbolas
with first/second derivatives, explicit periodic domains and oriented parameter spans.
Plane frames, seam crossing, reversed spans and affine derivative transformation are
checked independently of STEP. Regression/finite-difference tests, arbitrary-bit smoke,
a fuzz target and a benchmark cover the scope. See [CURVES.md](CURVES.md). Curve/schema
adapters, STEP trim selection and pcurve units remain pending. The next independent
geometry milestones 8–10 now provide analytic surfaces and NURBS.

**Milestone 8 delivered — independent analytic surfaces.**

`tessstep-surfaces` evaluates planes, cylinders, cones, spheres and ring tori with
first/second partials, parameter domains, oriented normals and explicit singularities.
See [SURFACES.md](SURFACES.md). STEP surface adapters and UV trimming are pending.

**Milestone 9 delivered — bounded NURBS curves.**

The curve crate adds expanded-knot validation, iterative basis derivatives, positive-weight
homogeneous evaluation and single interior knot insertion. Degrees 1–16, repeated knots,
one-sided derivatives, nonclamped/periodic representations and resource limits are tested.
See [NURBS.md](NURBS.md) for published algorithms and exact supported limits.

**Milestone 10 delivered — bounded tensor-product NURBS surfaces.**

The surface crate reuses the same knot basis, evaluates all first/second partials and
inserts knots in either axis while retaining the common control-net weight scale.
Rational patches, seams, mixed derivatives, refinement invariance and singular normals
are covered. STEP geometry adapters,
NURBS editing beyond single insertion remains open; Milestone 5 placement semantics are now implemented.

**Milestone 11 delivered — independent structural topology validity states.**

`tessstep-topology` supplies distinct handles, Raw/Validated/Normalized B-reps,
canonical incidence and bounded structural diagnostics. Wire closure, ownership,
endpoint agreement, shell edge incidence/connectivity and vertex links are checked.
Normalization is immutable and does not heal. See [TOPOLOGY.md](TOPOLOGY.md) for
limits: no self-intersection, volume containment or general cavity-shell validation.

**Milestone 12 delivered — bounded supplied-pcurve UV reconstruction.**

`tessstep-trim` reconstructs analytic/NURBS boundaries, retains seam uses and period
shifts, verifies sampled curve/surface agreement, checks outer/hole polygons and
classifies UV points. Missing pcurves and singularities fail explicitly. General
projection, nonlinear parameter correspondence, exact predicates and certified
continuous error bounds remain open. See [TRIMMING.md](TRIMMING.md).

**Milestone 13 delivered — canonical shared-edge boundary sampling.**

`tessstep-tessellate` samples each normalized edge once and exposes zero-copy oriented
views to its uses. Knot/period seeds, measured chord/tangent refinement, canonical
vertex endpoints and explicit limits are tested independently. Face boundary mapping
retains shared position references and distinct seam UVs. No triangle meshes are
produced. See [TESSELLATION.md](TESSELLATION.md) for sampled-error limitations.
Milestone 14 now adds planar face triangulation. STEP geometry adapters remain pending.

**Milestone 14 delivered — bounded constrained planar face triangulation.**

The shared-edge cache now triangulates analytic plane faces with concave outlines and
multiple holes. Actual-resolution polygon validation, deterministic visible bridges and
ear clipping preserve all boundary segments and canonical sample positions. Immutable
face-local results expose borrowed boundary vertices, u32 triangle indices and oriented
normals. Area/incidence checks, independent containment tests, mutation smoke and a
benchmark cover this scope. Exact predicates, triangle quality optimization, NURBS-plane
recognition and exact predicate/quality guarantees remain pending. Milestones 15–17
now provide regular curved faces, owned mesh assets and C/C++ mesh import/views.

**Milestone 15 delivered — bounded analytic curved-face tessellation.**

Regular plane/cylinder/cone/sphere/torus faces use constrained UV triangles, measured
surface chord/normal checks, periodic-span guards and conforming refinement. Boundary
requests are synchronized through the shared-edge cache, including straight edges with
varying surface normals. Singular poles/apices and automatic collapsed-edge repair
remain unsupported. Dense independent probes and periodic seam tests cover this scope.

**Milestone 16 delivered — bounded NURBS face refinement.**

Positive-weight tensor-product surfaces reuse the adaptive pipeline, adding probes in
every intersected knot cell and one-sided evaluations at cell boundaries. Rational
patches, narrow knot-span features, tighter tolerances, shared boundary requests and
explicit limits are tested. Continuous certified bounds, exact predicates and explicit
crease-aligned meshing inside a nonsmooth NURBS face remain outside this bounded slice.

**Milestone 17 delivered — owned manifold shell/solid mesh pipeline.**

`tessstep-mesh` owns indexed positions, per-corner UVs/normals and face provenance.
Assembly welds only canonical topology/sample identities. Oriented edge incidence,
vertex links, components, closure and algebraic volume are checked; solid results require
one closed component and positive volume. Closed cylinders and doubly periodic tori
are covered. This is topological watertightness, not self-intersection/material proof.
C/C++ mesh import and retained read-only views have installed consumer coverage. STEP
adapters, C/C++ B-rep/tessellation entry points, nested cavity shells and industrial
numerical hardening remain pending. Milestone 18 now adds independently supplied assembly assets and transforms.

**Milestone 18 delivered — independent assembly mesh assets.**

`tessstep-mesh::scene` shares immutable mesh assets across explicit occurrences and
resolves nested SI affine placements without recursive expansion. Sparse IDs, input
order, provenance and group nodes are retained. Reflections, nonuniform scale/shear,
inverse-transpose normals and explicit instance baking have independent regression,
mutation and benchmark coverage. C/C++ scenes retain original asset buffers, expose
scalar placement queries and return owned mesh acquisitions/bakes through the installed
package. See [ASSEMBLY_ASSETS.md](ASSEMBLY_ASSETS.md). Milestone 5 now supplies STEP placement adaptation and bounded definition-DAG expansion.
Automatic mesh binding and world-space tolerance-driven remeshing remain pending.
Milestone 19 now adds independently supplied appearance layers.

**Milestone 19 delivered — explicit mesh appearance.**

`tessstep-mesh::appearance` retains shared scene geometry and validates linear RGBA
palettes with asset/face assignments and inherited occurrence overrides. Allocation-free
triangle queries preserve winning-assignment provenance and distinguish unstyled from
transparent results. Face IDs and triangle order retain correspondence after reflected
baking. Bounded iterative validation, an independent mutation oracle, benchmarks and
installed C/C++ ownership/layout consumers cover this slice. The precedence policy is
explicitly independent of ISO style semantics. See [APPEARANCE.md](APPEARANCE.md).
STEP presentation adapters, textures, lighting models, surface-side styling and rendering
remain pending. The next numbered milestone is 20: existing tessellations.

Milestone 4 connects physical instances to schema-aware decoding. Milestone 5 builds
product/representation/units/assembly semantics. Milestones 6–10 build independent math,
analytic curves/surfaces and NURBS. Milestones 11–12 establish topology validity states
and UV trimming. Milestones 13–17 implement shared-edge sampling, planar and curved
face tessellation and the watertight solid pipeline. Milestones 18–19 supply independent assembly assets and appearance; Milestones 20–22 extend
existing tessellations, systematic AP242 coverage and industrial hardening. Conformance is evaluated by stage, not by whether a model opens.

The public-interface workstream must deliver both the stable C ABI and C++ RAII
wrapper, including typed errors and the installed `TessSTEP::TessSTEP` CMake
target. Begin with the first usable document operations; add immutable zero-copy
mesh views as the mesh pipeline becomes available. Design storage with that
contract in mind. C/C++ installed-package consumer tests and ABI containment are
release gates, not optional bindings work. See [C_API.md](C_API.md). This contract
also applies as product and geometry operations become public. The first physical-document
slice now implements both interfaces, typed results and the shared CMake package;
mesh import and retained read-only views are now implemented. Public schema decoding,
B-rep construction and tessellation entry points remain pending.
