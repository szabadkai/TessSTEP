# Milestones

This delivery implements the Milestone 0 foundation, Milestone 1 physical-parser
vertical slice, Milestone 2 EXPRESS frontend, Milestone 3 Rust bindings/reflection and
Milestone 4 structural schema decoding, each with explicit conformance limits.
Milestone 5 now includes the initial structural product/representation/units/assembly slice.
Milestone 6 now provides the independent checked math foundation.
Milestone 7 adds independent analytic curves. Surface evaluation and industrial
hardening are not claimed. Read ARCHITECTURE.md, CONFORMANCE.md and the existing tests before starting every milestone. Finish code,
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

**Milestone 5 in progress — products, representations, units and assembly semantics.**

The first slice adds an independent owned product graph, a schema-decoded adapter,
explicit SI/conversion unit scales, assembly occurrences, representation relationships
and reusable mapped-item links. Placement descriptions retain ordered item pairs;
no world transforms are evaluated. Separate authored schema/product corpus stages,
cycle/resource tests, mutation smoke and an assembly benchmark cover this scope.
See [PRODUCT_MODEL.md](PRODUCT_MODEL.md) for exact restrictions. Indirect context/shape
association paths, uncertainty measures, additional assembly/transform forms and
geometric placement interpretation remain. The math foundation is available; schema-specific placement evaluation remains pending.

**Milestone 6 delivered — independent checked math foundation.**

`tessstep-math` provides finite space-tagged points/vectors, normalized directions,
explicit lengths/angles/unit conversion, separate model/tessellation tolerances and
typed affine transforms. Composition, inversion, normal transformation, axis-angle
rotation and explicit right-handed frames have independent regression, property,
compile-fail and arbitrary-bit tests plus fuzz/benchmark harnesses. Numerical limits
are explicit in [NUMERICAL_ROBUSTNESS.md](NUMERICAL_ROBUSTNESS.md). This foundation does
not implement exact predicates, STEP placement defaults/adapters or curve/surface
geometry. Milestone 7 now supplies independent analytic curve evaluation; Milestone 5's
documented semantic gaps remain open.

**Milestone 7 delivered — independent analytic curves.**

`tessstep-curves` evaluates 2D/3D lines, circles, ellipses, parabolas and hyperbolas
with first/second derivatives, explicit periodic domains and oriented parameter spans.
Plane frames, seam crossing, reversed spans and affine derivative transformation are
checked independently of STEP. Regression/finite-difference tests, arbitrary-bit smoke,
a fuzz target and a benchmark cover the scope. See [CURVES.md](CURVES.md). Curve/schema
adapters, STEP trim selection and pcurve units remain pending. The next independent
geometry milestone is analytic surfaces; NURBS follows in later milestones.

Milestone 4 connects physical instances to schema-aware decoding. Milestone 5 builds
product/representation/units/assembly semantics. Milestones 6–10 build independent math,
analytic curves/surfaces and NURBS. Milestones 11–12 establish topology validity states
and UV trimming. Milestones 13–17 implement shared-edge sampling, planar and curved
face tessellation and the watertight solid pipeline. Milestones 18–22 extend assembly
assets, appearance, existing tessellations, systematic AP242 coverage and industrial
hardening. Conformance is evaluated by stage, not by whether a model opens.

The public-interface workstream must deliver both the stable C ABI and C++ RAII
wrapper, including typed errors and the installed `TessSTEP::TessSTEP` CMake
target. Begin with the first usable document operations; add immutable zero-copy
mesh views as the mesh pipeline becomes available. Design storage with that
contract in mind. C/C++ installed-package consumer tests and ABI containment are
release gates, not optional bindings work. See [C_API.md](C_API.md). This contract
also applies as product and geometry operations become public. The first physical-document
slice now implements both interfaces, typed results and the shared CMake package;
mesh views and public schema-decoding operations remain pending.
