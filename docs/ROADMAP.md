# Milestones

**Integration update:** selected planar `FACETED_BREP` and straight-edge
`MANIFOLD_SOLID_BREP` → validated geometry → owned
solid mesh is now available in Rust/C/C++. See [STEP_IMPORT.md](STEP_IMPORT.md) for
explicit units, reduced-profile scope and evidence. Existing triangulated STEP
tessellations import directly as owned meshes and tessellated annotations as placed
graphics, with discovery and representation selection (Milestone 20, Rust/C/C++); see
[EXISTING_TESSELLATIONS.md](EXISTING_TESSELLATIONS.md). Curved `MANIFOLD_SOLID_BREP`
import with elementary and B-spline geometry, computed pcurves and discovered context
units is available in Rust; see [CURVED_IMPORT.md](CURVED_IMPORT.md). Assemblies import
as scenes with surface colours (Milestone 21, Rust/C/C++); see
[PRODUCT_MODEL.md](PRODUCT_MODEL.md) and [APPEARANCE.md](APPEARANCE.md). Milestone 22
hardened the importers for industrial exports (tolerance policy, opt-in repair, surface
models, import sessions, named budgets) and froze the C ABI 1.0 export set; curved,
surface-model and session import are available in Rust, C and C++. The milestone
records below describe their delivery-time scope.

This delivery implements the Milestone 0 foundation, Milestone 1 physical-parser
vertical slice, Milestone 2 EXPRESS frontend, Milestone 3 Rust bindings/reflection and
Milestone 4 structural schema decoding, each with explicit conformance limits.
Milestone 5 delivers bounded local 3D product, representation, unit and assembly semantics.
Milestone 6 now provides the independent checked math foundation.
Milestones 7–10 add independent analytic curves/surfaces and NURBS curves/surfaces.
Milestones 11–17 add structural topology, supplied-pcurve UV trimming, shared-edge sampling, planar/regular curved-face refinement and owned manifold shell/solid meshes.
Milestone 18 adds shared assembly mesh assets and explicit nested placements.
Milestone 19 adds explicit color/opacity and inherited appearance overrides.
Milestone 20 imports existing shape and presentation tessellations without retessellation.
Milestone 21 compiles the AP long forms for a corpus schema stage, records AP242 geometry
coverage, and imports STEP assemblies as coloured scenes.
Milestone 22 hardens the importers for industrial exports and freezes the C ABI 1.0.
Milestone 23 (planned) makes the importers keep every body HOOPS Exchange meshes under
PreForm's import settings, and no slower.
AP conformance is not claimed. Read ARCHITECTURE.md, CONFORMANCE.md and the existing tests before starting every milestone. Finish code,
tests, fmt/clippy, applicable corpus/fuzz runs, documentation, conformance updates
and architecture review before declaring work done. Benchmarks
run automatically in the nightly CI workflow only; do not run them locally for milestone
sign-off.

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
adapters, C/C++ B-rep/tessellation entry points and industrial numerical hardening
remain pending; cavity shells followed with curved STEP adaptation. Milestone 18 now adds independently supplied assembly assets and transforms.

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
remain pending. Milestone 20 now imports existing tessellations.

**Milestone 20 delivered — existing shape and presentation tessellations.**

`tessstep_import::import_tessellated` converts a selected `TESSELLATED_SOLID`,
`TESSELLATED_SHELL` or triangulated surface set into an owned mesh without
retessellation. `TRIANGULATED_FACE` and `COMPLEX_TRIANGULATED_FACE` supply explicit
triangles, strips and fans over shared `COORDINATES_LIST`s. Vertex identity is the
coordinate-list index, never proximity. Supplied normals must agree with winding;
solids must be closed with positive volume. Opt-in decoder link slots retain B-rep
provenance links without decoding them, so linked exports import even when the
linked geometry is unsupported. The milestone was completed on 2026-09-26:

- **Tessellated edges and vertices.** `TESSELLATED_EDGE`, `TESSELLATED_VERTEX` and
  `TESSELLATED_CONNECTING_EDGE` items are polylines and points with mesh-vertex
  correspondence. Connecting edges join exactly the coordinate identities they
  declare, after checking equal coordinates and that every segment is a triangle edge
  of both faces, so faces with separate coordinate lists close without proximity
  welding.
- **Presentation tessellations.** `import_presentation` turns a
  `TESSELLATED_ANNOTATION_OCCURRENCE` or `TESSELLATED_GEOMETRIC_SET` into placed
  polylines, points and fill triangles. It composes `REPOSITIONED_TESSELLATED_ITEM`
  placements and retains styles as IDs. Graphics are not meshes: annotation normals
  (17% of corpus fills disagree with winding) are unused, and zero-area fills are
  kept and counted.
- **Representation selection.** `discover_tessellations` and `discover_presentations`
  find roots and their context units, through callouts, annotation planes and
  characterized draughting models. `select_representations` pairs a B-rep with its
  tessellations by explicit links, or by an unambiguous transformation-free
  representation relationship, under an explicit exact or tessellated preference.
- **C/C++ entry points.** `ts_document_import_tessellated` (owned mesh plus
  provenance info) and `ts_document_import_presentation` (a retained handle with
  zero-copy views), with C++ `Document::import_tessellated`, `Presentation` and
  `PresentationView`, bring ABI 1 to 50 exports under installed-consumer tests.
- **Corpus survey.** The runner surveys every shape tessellation root, reports
  whether representation selection chose it, and summarizes every annotation
  occurrence. All 8 real exported shape tessellations and 1,429 of 1,431 real
  annotation occurrences import.

One exporter deviation is tolerated by default and rejected by `strict`: without a
`pnindex`, a `pnmax` below `npoints` (975 HOOPS Exchange fills). Authored fixtures,
typed rejection tests, mutation smoke, fuzz targets, benchmarks and five hash-pinned
unmodified exporter files (NIST, CATIA, HOOPS) cover this milestone. See
[EXISTING_TESSELLATIONS.md](EXISTING_TESSELLATIONS.md). Cubic Bézier faces,
`TESSELLATED_WIRE`, style and PMI semantics, and product-definition linking of
discovered roots are outside it. The next numbered milestone is 21: systematic AP242
coverage.

**Curved STEP adaptation — first slice delivered 2026-09-26.**

`tessstep_import::import_brep_solid` imports `MANIFOLD_SOLID_BREP`s on planes,
cylinders, cones, spheres, ring tori and (rational, quasi-uniform) B-spline surfaces,
bounded by lines, circles, ellipses and B-spline curves, directly or as the 3D curve
of surface, seam and intersection curves, in simple or complex encodings. Pcurves are
computed from the 3D curves (exact where linear or conic, otherwise verified cubic
Hermite fits); supplied PCURVEs are retained as aggregate link slots. Closed B-spline
axes receive periodic kernel charts. Faces without `FACE_OUTER_BOUND` use their unique
counterclockwise loop, and annular faces on periodic surfaces are cut by an inserted
isoparametric seam; both adaptations are counted. `discover_solids` reads each root's
context units and uncertainty. The tessellator gained metric-aware best-first ear
clipping, queue-based Lawson flips and facet-plane chord error, and trimming places
holes by whole periods.
Eleven generated authored fixtures, three hash-pinned exporter files (ST-DEVELOPER,
CATIA V5, I-DEAS) and the corpus survey cover this slice: 348 of 1,173 surveyed solid
roots now pass every stage (22 before). See [CURVED_IMPORT.md](CURVED_IMPORT.md).
Edge splits for misaligned annuli and sphere re-charting followed the same day.
Pole and apex charts followed on 2026-09-26: collapsed edges close face charts at
sphere poles, cone apices and collapsed B-spline sides (loop joins, enclosing loops with
pole seams, `VERTEX_LOOP`s, full spheres), with polar-wedge tessellation at pole
copies, exact meridian pcurves and one-sided pole limits. Swept surfaces (exact
elementary charts or exact NURBS, with STEP normal orientation), `TRIMMED_CURVE`
bases and `BREP_WITH_VOIDS` cavity shells followed the same day. C/C++ entry points
and anisotropic refinement remained pending (delivered by Milestones 22.10 and 22.2).

**Import hardening backlog — observed on third-party CAD exports.**

Running the planar importers over every solid root of unmodified exports from
several CAD systems exposed the gaps below. They feed Milestones 21–22 and the
pending curved STEP adaptation; none is a conformance claim. Each item needs
authored fixtures, typed outcomes and corpus-stage evidence before delivery.

- **Curved geometry is the dominant blocker — elementary and B-spline adapters
  delivered 2026-09-26.** `CIRCLE`/`ELLIPSE` edges, cylindrical, conical, spherical
  and toroidal surfaces and (rational) B-spline curves and surfaces, including
  complex-instance encodings, import through the curved profile. Annular faces with
  misaligned loop vertices are handled by reported edge splits, spherical faces
  touching their STEP pole by re-charting, and faces reaching a pole, apex or
  collapsed B-spline side by collapsed-edge charts. `SURFACE_OF_LINEAR_EXTRUSION`,
  `SURFACE_OF_REVOLUTION`, `TRIMMED_CURVE` and `BREP_WITH_VOIDS` cavity shells are
  supported. The remaining corpus blockers, by frequency: open or inconsistently
  oriented shells (see below), edges off their surface, trimming and tessellation
  failures on planned faces, and tessellation budgets. Evidence:
  `brep_elementary_surfaces_close_with_expected_volumes`,
  `brep_edge_splits_and_sphere_recharting_are_reported`,
  `brep_poles_and_apices_close_with_collapsed_edges`,
  `brep_swept_surfaces_are_exact_elementary_or_nurbs_charts`,
  `brep_cavity_shells_mesh_as_inward_components`.
- **Implicit outer bounds — delivered 2026-09-26 in the curved profile.** A face with
  several bounds and no `FACE_OUTER_BOUND` uses its unique counterclockwise loop in
  the surface chart and otherwise fails with "ambiguous outer bound"; holes on
  periodic surfaces are placed by whole periods. The planar profile keeps its
  explicit-outer rule. Evidence:
  `brep_elementary_surfaces_close_with_expected_volumes` (washer) and
  `brep_rejections_are_typed_and_located`.
- **Default parse budgets — delivered 2026-09-27 (Milestone 22.1).** The defaults (4M total values, 1M entities, 2M records)
  reject well-formed exports of a few tens of megabytes. Size defaults against
  realistic industrial files, or derive them from input size, while keeping every
  budget explicit. Expose them in `stepdump` and the corpus runner, and name the
  exceeded limit and its configured value in the diagnostic. Changes to C
  `ts_parse_options_init` defaults are documented behavior changes.
- **Actionable unsupported-entity diagnostics — delivered 2026-09-26.** Profile
  rejection names the entity type, or every complex-instance component and those
  outside the profile, and says "outside the selected import profile" rather than
  "unknown to the schema". Evidence:
  `planar_unsupported_entities_are_named_as_outside_the_profile`.
- **Disconnected closed shells — delivered 2026-10-09 (Milestone 22.5).** A
  `CLOSED_SHELL` whose faces share no edges or vertices fails strictly as
  `UnsewnShell`, apart from other open shells; the opt-in repair stage
  (`ImportOptions::repair`) sews it within a stated tolerance and counts every merge.
  See 22.5 below.
- **Per-root import cost — delivered 2026-09-26 (decoder) and 2026-10-09 (session,
  Milestone 22.8).** Selected-root decoding used to scan and charge every document
  entity per root; it now visits only the closure, ordered through the entity index,
  which also removed decode budget exhaustion on large files. The document import
  session and its scaling benchmark are delivered under 22.8 below.
- **Root and unit discovery — delivered 2026-09-26.** `discover_solids` finds every
  solid root, the shape representations containing it and their context's SI or
  conversion-based length and plane-angle units and length uncertainty, through a
  bounded context profile. Missing or conflicting units are per-root errors, never
  defaults. Milestone 20 applies the same rules to shape tessellations and annotation
  occurrences. Linking roots to product definitions and placements (Milestone 5
  graphs) remained (delivered by Milestone 21.7). Evidence: `discovery_reads_context_units_and_uncertainty`,
  `presentation_discovery_follows_callouts_into_draughting_models`.
- **Corpus geometry stage — delivered 2026-09-26.** The corpus runner imports every
  solid root and reports per-root outcomes grouped by stage and error category; see
  [corpus testing](corpus-testing.md#solid-import-stage). Roots now use discovered
  context units and uncertainty (floored at 1e-7 m) and the curved profile; the
  baseline was refreshed again on 2026-09-26 after reviewing 234 changed inputs and no
  regressions. Milestone 20 added shape tessellation roots, selection status and an
  annotation summary; the baseline was refreshed after reviewing 52 changed inputs,
  none a lost root outcome (see [VALIDATION.md](VALIDATION.md)).
- **Unset derived slots — delivered 2026-09-26.** `$` instead of `*` in derived
  `ORIENTED_EDGE` endpoint slots is accepted by default and rejected by an explicit
  strict switch (`ImportOptions::strict`, C `TS_IMPORT_STRICT`, C++ `ImportPolicy`).
  In the corpus this moves 177 roots past the profile stage; all of them then fail
  topology checks (open or inconsistently oriented shells, broken wires), so they feed
  the disconnected-shell item above.

**Curved import requirements — observed on an Onshape export, 2026-09-26.**

An unmodified Onshape AP242 export (ST-DEVELOPER v20; one `MANIFOLD_SOLID_BREP` with
149 faces: 25 planes, 24 cylinders, 22 cones, 21 ring tori, 2 degenerate tori and 55
B-spline surfaces, 17 of them rational) parses cleanly but is rejected at the profile
stage. The curved adapters above are necessary but not sufficient for it; it also
relies on implicit outer bounds (94 multi-bound faces, no `FACE_OUTER_BOUND`). Each
item needs authored fixtures, typed outcomes and corpus-stage evidence. Add a
redistributable Onshape export with these features as a hash-pinned exporter file.
Moved to post-1.0 (decided 2026-10-10): the export used here is not cleared for
redistribution, so it stays outside the repository and its evidence is recorded in
[VALIDATION.md](VALIDATION.md); a cleared Onshape export will be pinned when available.

- **Toroidal surfaces, ring and degenerate — delivered 2026-09-27.** Ring
  `TOROIDAL_SURFACE`s import through the curved profile, and
  `DEGENERATE_TOROIDAL_SURFACE` (minor radius above the major radius, here 1 mm over
  0.25 mm) imports as a spindle torus whose `select_outer` part is the closed v domain
  with the two axis points as singular points. Evidence: `brep-spindle-apple.step`,
  `brep-spindle-lemon.step`, `analytic_spindle_torus_parts_end_at_the_axis_points`.
- **Tessellation refinement — delivered 2026-10-09 (Milestone 22.2).** With
  degenerate tori, seam placement between holes and tangent edges, tolerance-consistent
  sampling and trimming and sliver-face collapse, the whole 149-face B-rep passed
  profile, geometry and topology, but tessellation walked a sliver along a torus band
  (face #2168, R = 2.035 mm, r = 0.3 mm) to the round limit, the cylinder with 36 holes
  (face #2164) exceeded the 10M planar work budget, and the 36 two-pole B-spline faces
  folded their pole fans. With curvature-anisotropic refinement, polar caps and the
  cheaper planar triangulation of 22.2, the export imports as a closed solid within the
  default budgets (65,766 vertices, 131,556 triangles, 5.85e-6 m³, about 7 s).
- **Computed pcurves — delivered 2026-09-26.** Every coedge receives a pcurve computed
  from its 3D curve, exact or a verified fit, with the edge parameter as its parameter
  ([curved import](CURVED_IMPORT.md#computed-pcurves)). The export has no `PCURVE`,
  `SURFACE_CURVE` or `SEAM_CURVE` records, so each coedge's UV trim must come from its
  3D curve. Trimming accepts only supplied pcurves with an affine parameter
  correspondence. Compute pcurves exactly where the UV image is linear or conic (for
  example rulings and coaxial circles). Otherwise use point inversion with a verified
  fit and a nonlinear parameter correspondence; 181 B-spline edge uses lie on curved
  faces here.
- **Periodic faces without seam edges — delivered 2026-09-26** as inserted seams, edge
  splits and whole-period hole placement, including isocurve seams on periodic
  B-spline charts. 35 faces on cylinders, cones, tori and B-spline surfaces are
  bounded only by closed edges. Further faces combine a closed circle with an open
  loop, and none has a seam edge. Their loops wrap the period and do not close in the
  unwrapped chart, which trimming rejects. Insert an isoparametric seam as a reported
  adaptation without changing geometry. Holes, such as the 36 further closed-edge
  loops on one cylinder, must be placed in that seam's chart.
- **Poles and point bounds — delivered 2026-09-26** for sphere poles, cone apices and
  collapsed B-spline sides (collapsed chart edges, `VERTEX_LOOP` bounds, polar fans);
  degenerate-torus axis points remain with degenerate tori. 36 cubic B-spline faces
  each have one closed edge loop and two `VERTEX_LOOP` bounds. At least one of them
  lies on a control row that collapses to a point. Milestone 15 leaves poles and
  collapsed edges unsupported. Represent point bounds and collapsed control rows as
  collapsed chart edges, give pole fans a well-defined normal, and apply the same rule
  to cone apices, sphere poles and degenerate-torus axis points.

**Geometry coverage gaps — observed in a corpus survey, 2026-09-26.**

A text scan of all 3,230 external corpus files resolved every `ADVANCED_FACE` /
`FACE_SURFACE` surface and `EDGE_CURVE` curve by reference. It found B-rep geometry
that neither the adapter priorities nor the Onshape items above cover. Counts are
face or edge uses, with files in parentheses; the scan did not use the parser, so
they are approximate. OpenCASCADE test data supplies most curved uses and several
entities occur only in `dodgy-step-files`, so frequencies do not describe exporters
in general. Each item needs authored fixtures, typed outcomes and corpus-stage
evidence. A rare entity may be delivered as a named, typed rejection instead of an
adapter. These items feed Milestone 21.

- **Supplied pcurves — 3D curves delivered 2026-09-26; supplied pcurves read
  2026-09-27 (Milestone 21.6).** `SURFACE_CURVE`, `SEAM_CURVE`
  and `INTERSECTION_CURVE` import through their 3D curves with computed pcurves;
  supplied pcurves are retained as links, and reading them remains open. 14,062 edges
  (671 files) wrap their 3D curve in `SURFACE_CURVE` and 345 (28 files) in
  `SEAM_CURVE`, with `PCURVE`s over `DEFINITIONAL_REPRESENTATION`s. `SURFACE_CURVE` is
  already the third most frequent profile rejection in [VALIDATION.md](VALIDATION.md).
  Adapt them to the existing supplied-pcurve trimming and seam uses: honour
  `master_representation`, match each pcurve to its face's surface and check it
  against the 3D curve within model tolerance. Compute pcurves only for faces without
  an associated one.
- **Swept surfaces — delivered 2026-09-26** as exact elementary charts or exact NURBS
  with STEP normal orientation; `TRIMMED_CURVE` profiles contribute their basis and
  sense. `SURFACE_OF_LINEAR_EXTRUSION` (1,340 faces, 53 files) and
  `SURFACE_OF_REVOLUTION` (627, 42) need evaluators and adapters. Their profiles are
  mostly (rational) B-splines, with some lines, circles, ellipses and
  `TRIMMED_CURVE`s. A revolved profile that meets its axis creates poles, covered by
  the pole item above.
- **Implicit-knot B-spline forms — quasi-uniform forms delivered 2026-09-26;
  `UNIFORM_*` and `BEZIER_*` delivered 2026-09-27 (Milestone 21.6).** `QUASI_UNIFORM_SURFACE` (163 faces, 18 files)
  and `QUASI_UNIFORM_CURVE` (761 edges, 24 files), including rational complex
  instances, carry no knot list. Derive knots as ISO 10303-42 defines them and reuse
  the NURBS evaluators. `UNIFORM_*` and `BEZIER_*` forms follow the same rule (3 uses
  in total).
- **Hyperbolic and parabolic edges — delivered 2026-09-27 (Milestone 21.6) as exact
  quadratic arcs.** `HYPERBOLA` / `PARABOLA` edge geometry (22 edges,
  4 files) has evaluators but no adapter or trim-parameter mapping.
- **Cavity shells — delivered 2026-09-26:** `Solid::voids`, composed
  `ORIENTED_CLOSED_SHELL` orientation and per-shell volume checks. The 29
  `BREP_WITH_VOIDS` roots hold their voids as `ORIENTED_CLOSED_SHELL`s (83 uses).
  Compose the shell orientation and extend the solid checks to nested cavities.
- **Point bounds are not exporter specific — covered by the delivered pole item.** 276
  of the 287 `VERTEX_LOOP` face bounds occur in OpenCASCADE and NIST files, so the
  pole item above is general.
- **Surface models — scope decision; delivered as Milestone 22.6 (2026-10-09).** 4,127
  `SHELL_BASED_SURFACE_MODEL` roots
  outnumber the 1,460 `MANIFOLD_SOLID_BREP` roots; 2,539 of them are in 108
  OpenCASCADE files. The corpus runner does not survey them, although
  `tessellate_shell` already meshes open shells. Decide whether STEP import accepts
  `OPEN_SHELL` / `CLOSED_SHELL` surface models, reporting open results as shells,
  never as solids. Decided: it does, as shells (22.6 below).
- **Topology-free trimmed surfaces — decided 2026-09-27 (Milestone 21.6): named
  unsupported for v1.0.** `CURVE_BOUNDED_SURFACE`
  (3,216 records, 18 files) occurs almost only in `GEOMETRIC_SET`s, not as face
  geometry. Meshing it needs boundary-curve trimming without B-rep topology. Decide
  whether it is in scope; otherwise name it as unsupported.
- **Rare edge and face geometry — named typed rejections delivered 2026-09-27
  (Milestone 21.6).** These occur almost only in `dodgy-step-files`:
  `RECTANGULAR_TRIMMED_SURFACE` (38 faces), `OFFSET_SURFACE` (13),
  `RECTANGULAR_COMPOSITE_SURFACE` (4), `TRIMMED_CURVE` (42 edges; delivered),
  `PCURVE` used directly as edge geometry (17), `COMPOSITE_CURVE` (14),
  `OFFSET_CURVE_3D` (5), `DEGENERATE_PCURVE` (4) and `INTERSECTION_CURVE` (2;
  delivered). Give each a named, typed
  unsupported outcome first; add an adapter when a reviewed exporter file needs one.
- **Reproducible coverage survey — delivered 2026-09-27 (Milestone 21.5).** Add this survey to `scripts/corpus.py`: count
  face-surface, edge-curve and root types per source from decoded records and keep
  them in the baseline. The existing `unsupported_entity_types` summary names only
  the first rejection per root, so it hides entities behind an earlier failure.

**Milestone 21 — systematic AP schema coverage (planned 2026-09-26, completed 2026-09-27).**

Goal: every input whose header names a supported application protocol passes a real
schema stage, and every geometric, topological and shape-representation entity type
in those schemas has a recorded, tested outcome: adapted, a named typed rejection, or
an explicit out-of-scope decision. Coverage is derived from the compiled schemas, not
from whichever entities the corpus happens to contain. No ISO schema is compiled
today. The importers decode through five original reduced profiles
(`corpus/geometry/*.exp`), and the corpus schema and product stages are
`not_configured` for all 3,227 inputs.

Target schemas follow the corpus headers. File paths declare `AUTOMOTIVE_DESIGN`
(AP214; 2,723, plus 62 `_CC1`/`_CC2`/`_CC04`), the AP242 MIM long form (184),
`CONFIG_CONTROL_DESIGN` (AP203; 80) and the AP203 edition 2 MIM long form (14). The
remaining headers (AP238, AP209, AP210, IFC4, test schemas; about 170 paths) stay
`not_configured`, with the declared name reported. The pinned stepcode revision
ships long forms for all four: `data/ap242/242_mim_lf.exp` (WG12 N11521; 2,407
entities, 528 types, 348 functions, 58 rules), `data/ap214e3/AP214E3_2010.exp` (915
entities), `data/ap203e2/ap203e2_mim_lf.exp` (1,006) and `data/ap203/ap203.exp` (254).

- **21.1 Schema sourcing — delivered 2026-09-27.** `corpus/ap-schemas.json` pins the
  stepcode revision, paths and SHA-256 of the four long forms; `scripts/fetch_schemas.py`
  fetches and verifies them, CI fetches them with the corpus, and nothing is committed
  (`schemas/README.md` records the ISO/stepcode terms).
- **21.2 EXPRESS frontend gaps — delivered 2026-09-27.** All four schemas compile
  structurally (AP242: 3,363 declarations, 6,916 unsupported diagnostics from
  retained rules, functions and expressions); `scripts/build_ap_validator.py`
  generates one validator holding all four (19.7 MB of Rust, ~20 s, 8.9 MB
  executable). Evidence: `express_redeclarations_qualified_names_and_end_identifiers`,
  the codegen consumer test, [VALIDATION.md](VALIDATION.md). On 2026-09-26
  `expressc` had failed on all four schemas. The blockers found, in order:
  - attribute redeclaration `SELF\entity.attribute : type` in explicit and DERIVE
    sections, including multi-line forms. There are 1,011 such lines in AP242, and
    redeclaration is the first failure in every schema;
  - opaque-expression scanning stops at identifiers that begin with `end_` (for
    example `end_exit_faces` in `solid_with_slot` WR2), mistaking them for `END_*`
    keywords;
  - recursion through aggregates (`maths_value` ↔ `maths_tuple`, `atom_based_value`
    ↔ `atom_based_tuple`) is rejected as a type cycle (EX2007);
  - same-named attributes inherited through different supertypes are rejected
    (EX2008). There are 88 in AP203 edition 2 (`name`, `description`, `id`,
    `definition`), and they should be retained as distinct, group-qualified
    attributes.

  With the first two blockers worked around in a scratch copy, AP242 resolves 3,363
  declarations with 5,884 unsupported diagnostics and fails only on the five cycles.
  More gaps may be hidden behind these. Fix each gap with an original small fixture in
  `corpus/express/`, not an ISO extract. Then generate bindings and a validator for
  each schema, measure generation and rustc time and output size, and set explicit
  budgets. Acceptance: all four compile without errors; unsupported diagnostics come
  only from retained rules, functions and WHERE/UNIQUE bodies and are counted in the
  report; the generated bindings and validators build in CI.
- **21.3 Corpus schema stage — delivered 2026-09-27.** `scripts/corpus.py
  --schema-map corpus/ap-schema-map.json` selects each input's schema from its
  declared `FILE_SCHEMA` name through an explicit table (no fuzzy matching; the
  `AUTOMOTIVE_DESIGN_CC*` headers stay `not_configured`) and validates structurally
  under the decoder's retaining policy, which counts unevaluated rules and the
  tolerated `$` derived slots and unordered complex components. Outcomes are in the
  baseline and summary; a previously accepted input becoming non-accepted fails
  `--check`. Every rejection category was reviewed by entity
  ([VALIDATION.md](VALIDATION.md)); the object identifier is not yet compared
  against the compiled edition.
- **21.4 AP metadata and import profiles — delivered 2026-09-27 as re-scoped.**
  Decoding each root closure against the AP long form inside the library is not
  possible without redistributing ISO schemas (21.1), so the bounded original profiles
  stay the import path and the AP validator stays a corpus stage. Their outcomes are
  joined instead: the corpus summary reports, for every profile rejection naming an
  unsupported entity, whether the file's AP schema stage accepted it (the entity is
  then valid in the AP but not adapted) or rejected it. Accepted meshes are
  unchanged by this item.
- **21.5 Generated coverage matrix — delivered 2026-09-27.**
  `scripts/check_coverage.py` enumerates every instantiable subtype of
  `geometric_representation_item`, `topological_representation_item` and
  `shape_representation` from the compiled AP242 long form and checks
  `docs/coverage.json`: 501 entities, each adapted (and declared by an import
  profile), a profile supertype, a product-profile container or placement, a named
  unsupported rejection or a recorded out-of-scope decision; [COVERAGE.md](COVERAGE.md)
  is generated from it and CI runs the check. The corpus survey counts every record
  type in each root's reference closure, whatever the outcome, keeps the per-file
  counts in the baseline, and lists the unadapted types in failing roots' closures
  ([corpus testing](corpus-testing.md#product-stage-and-coverage-survey-milestone-21)).
- **21.6 Geometry adapters and named rejections — delivered 2026-09-27:**
  - supplied pcurves on `SURFACE_CURVE` / `SEAM_CURVE` are decoded apart with an
    original pcurve profile, mapped from STEP surface parameters into elementary and
    B-spline charts, tried master first and used only when they agree with the 3D
    curve at nine parameters within the model tolerance; every other use is computed
    ([curved import](CURVED_IMPORT.md#supplied-and-computed-pcurves));
  - `DEGENERATE_TOROIDAL_SURFACE` spindle tori, seam placement that avoids holes and
    edges tangent to the seam line, metric sampling and trimming tolerances, and
    sliver-face collapse (see the Onshape block above);
  - `UNIFORM_*` and `BEZIER_*` B-spline forms with ISO 10303-42 knots, and `HYPERBOLA` /
    `PARABOLA` edges as exact quadratic arcs;
  - named typed rejections for `OFFSET_SURFACE`, `RECTANGULAR_TRIMMED_SURFACE`,
    `RECTANGULAR_COMPOSITE_SURFACE`, `COMPOSITE_CURVE`, `OFFSET_CURVE_3D` and `PCURVE` /
    `DEGENERATE_PCURVE` edge geometry;
  - scope decisions: `SHELL_BASED_SURFACE_MODEL` is a named unsupported outcome for
    Milestone 21; importing surface models as shell meshes, never as solids, moves to
    Milestone 22. `CURVE_BOUNDED_SURFACE` is named unsupported for v1.0.
- **21.7 Product structure and assemblies — delivered 2026-09-27.**
  `tessstep_import::link_assembly` decodes product records with an original product
  profile (AP203/AP214/AP242 physical layouts; representation items and associated
  documents as links) and places supplied meshes by next assembly usage occurrences and
  their context dependent shape representations in a Milestone 18 scene;
  `import_assembly` imports every selected shape root in its context units first.
  Missing or ambiguous placements fail at the new product stage. The corpus runner
  runs this product stage on every mapped input. Evidence: `tests/assembly.rs`
  (authored nested, missing-placement, unplaced and inch fixtures) and, hash-pinned,
  stepcode `as1-oc-214.stp` with three further exporter variants of the same assembly
  (repeated nuts and bolts: 9 products, 13 occurrences, 28 definition nodes, 18 part
  instances; consistent bounds and volumes). The corpus holds no NIST assembly, so the
  repeated-part evidence is these unmodified exporter files. Mapped-item and
  Cartesian-operator assemblies and external document references are not linked.
- **21.8 Presentation — delivered 2026-09-27.** `import_appearance` adapts
  `STYLED_ITEM` / `OVER_RIDING_STYLED_ITEM` fill-area and rendering colours and
  transparency to Milestone 19 appearance on assets (solid, outer shell, shape
  representation) and asset faces, from an original style profile that never decodes
  curve or point styles. Context-dependent styles, layers, invisibility, curve styles
  and textures are counted, never applied; PMI is out of scope for v1.0
  ([appearance](APPEARANCE.md#step-surface-colours-milestone-21)).
- **21.9 Public interfaces — delivered 2026-09-27.** `ts_document_get_schema` (schema
  selection), `ts_document_import_assembly` with a retained `ts_assembly` (products,
  nodes, root outcomes, typed exclusions, style counts, scene and appearance
  acquisitions), C++ `Document::schema`, `Document::import_assembly` and `Assembly`;
  64 exports under installed-consumer tests ([C API](C_API.md)). The single-root curved
  import C entry point followed in Milestone 22.10.

Result (2026-09-27): every exit criterion below is met, with the scope decisions
recorded above; the reviewed corpus baseline holds 577 meshed roots and 585 accepted
product structures (see [VALIDATION.md](VALIDATION.md)).

Order: 21.1 → 21.2 → 21.3; then 21.4, 21.5 and 21.7 in parallel, with 21.8 after
21.7. 21.6 does not depend on schema work and can start now. 21.9 accompanies each
public operation.

Exit criteria: all four schemas compile, and their bindings and validators build in
CI. The corpus schema and product stages run on every mapped input, with reviewed
baselines. The coverage matrix is complete and tested. No solid root fails on an
unnamed entity. The Onshape export imports as a closed solid, or its remaining
blocker is on this roadmap. `as1-oc-214.stp` imports as a scene. The
conformance rows for AP242/AP214/AP203 move to "structural subset implemented",
with limits. EXPRESS rules, functions and WHERE clauses are not evaluated, so no AP
conformance is claimed.

**Milestone 22 — industrial hardening (planned 2026-09-27, items delivered 2026-10-10).**

Goal: the importers behave predictably on large and imperfect industrial exports.
Every budget is explicit and sized for real files, every repair is opt-in and
reported, every remaining corpus failure is a named outcome, and the public C/C++
interfaces cover the whole import path. Items carry the counts from the reviewed
2026-09-27 baseline (1,197 shape roots, 577 meshed); each one lands with reviewed
baseline movements, never a refreshed baseline.

- **22.1 Parse and import budgets — delivered 2026-09-27.** The corpus showed the
  physical parse defaults were not the limit: every well-formed input up to 30.7 MB
  and 413,209 entities parses within them (the larger files are rejected for invalid
  UTF-8 or hexadecimal, not budgets), while the import work budget (5M units) rejected
  the 11 `profile:resource_limit` roots, whose closures need 130 to 250 units per
  entity. `ImportOptions::for_document` sizes `max_work` (512 per entity) and
  `max_records` (one per entity) from the document with the default floors; the
  survey and corpus runner use it and record the budgets per file, with
  `--geometry-max-work` / `--geometry-max-records` overrides. Every parse, decode,
  adapter, discovery, assembly, style, pcurve and mesh budget failure names the
  budget and its configured value; `stepdump --max-<budget>=N` sets each parse
  budget and `--help` prints the defaults. `ts_parse_options_init` and the C import
  initializers are unchanged; C callers apply the sizing rule from
  `entity_count`. Evidence: `every_resource_budget_is_enforced`,
  `import_budgets_are_sized_by_the_document_and_named_in_diagnostics`, the reviewed
  corpus movement in [VALIDATION.md](VALIDATION.md).
- **22.2 Curvature-anisotropic refinement and tessellation budgets — delivered
  2026-10-09.** Longest-edge bisection in a length metric with Lawson flips walked
  slivers along tight torus and cylinder bands (24 `UnresolvedTolerance` and 25
  `ResourceLimit` faces; the Onshape torus band, face #2168). Refinement and flips now
  use a per-vertex curvature metric (the shape of |II|, plus a tangential bending term,
  an isotropic floor and a model-tolerance ceiling), so bands are split across rather
  than along; pole runs get explicit monotone fans (polar caps); boundary requests wait
  until the interior settles; faces and facets that already passed are reused; facets
  are measured through the surface lift of their corners and, below the model
  tolerance, by chord only. The planar cost was cut rather than the budget raised:
  bridges are tested in length order and ear tests use a grid, giving identical
  triangulations (the 36-hole face's 28M work units fell to 2.1M). Every resource
  limit, round limit and pass limit names its budget and value
  (`TessellationError::budget`); defaults are unchanged. The Onshape export imports as
  a closed solid; in the corpus 66 roots newly mesh (643 meshed), none is lost, and
  all 25 remaining `tessellation:resource_limit` roots name their budget (22 trim
  reconstruction work in `tessstep-trim`'s quadratic loop checks, two `max_work`, one
  `max_evaluations`). The thin-cylinder case was not reproduced by an authored band
  (it meshes on both versions), so its evidence is the Onshape export only. Evidence:
  `narrow_torus_band_refines_across_the_band`,
  `planar_face_with_many_holes_meshes_within_default_budgets`,
  `pole_caps_keep_fans_monotone_beside_seams_and_holes`,
  `tessellation_budgets_are_named_in_diagnostics`, the existing adaptive and pole
  tests, the reviewed corpus movement in [VALIDATION.md](VALIDATION.md).
  Follow-up 2026-10-10: `bug30790.step` (24 B-spline solids of 217,000–251,000
  triangles each) exhausted `max_evaluations` in every solid and the corpus deadline.
  Its meshes are genuinely large, so the waste went instead: probes reuse vertex and
  shared midpoint evaluations, faces keep evaluations and passed facets across
  boundary passes, the edge incidence is maintained rather than rebuilt per round,
  and mesh labels are rewritten without a second validation; meshes are unchanged and
  each solid needs 5–7M evaluations instead of 10–15M. Opt-in
  `TessellationOptions::threads` meshes a pass's faces in parallel with
  thread-independent results (the survey uses it; C keeps one thread). The file meshes
  in 19 s locally (83 s on one thread); evidence:
  `boundary_passes_reuse_evaluations_and_threads_change_nothing`,
  `spline_basis_matches_the_full_recurrence_bit_for_bit`.
- **22.3 Tolerance policy for edges off their surfaces — delivered 2026-10-09.**
  Measured first on the reviewed baseline: 77 roots had an edge farther than the model
  tolerance from a face surface (30 synthetic `dodgy-step-files`, 47 from exporter
  files), 20 an edge crossing a singular point (all on B-spline surfaces) and 20
  a vertex off its edge curve (10 synthetic, 10 exporter); the distribution by
  source and exporter is in [VALIDATION.md](VALIDATION.md). One policy covers both
  distances ([curved import](CURVED_IMPORT.md#tolerance-policy)):
  `ImportOptions::deviation_factor` (default 16; `strict` forces 1) accepts a
  distance above the model tolerance up to that multiple as a counted adaptation
  (`Adaptations::off_surface_edges`, `off_curve_vertices`), reports the largest in
  `ImportedSolid::largest_deviation` and validates, trims and tessellates with the
  model tolerance plus twice that distance; beyond it the import fails with a typed
  `InvalidGeometry` naming the measured distance and the bound. No coordinate moves:
  an off-surface edge receives the pcurve of its surface projection and the mesh
  follows its 3D curve, and an edge is bounded at its vertex's projection onto the
  curve (a shared vertex cannot be projected onto every edge). `tessellate` refused a
  chord tolerance below the accepted deviation until 1.1, which floors the accepted
  deviation at the consumer's chord instead (`ImportOptions::deviation_floor`). Every
  singular crossing with a
  measured singular point was a B-spline side that collapses only within a large
  declared uncertainty (46 µm under 50.8 µm, 0.10 mm under 0.127 mm, 5 µm under
  10 µm, 0.39 mm under 1 mm) with an edge running along it; such a side is now a
  regular chart side (`Adaptations::uncollapsed_sides`). Edges through analytic poles,
  apices or exactly collapsed sides, and singular parameterizations, stay
  `Unsupported`, naming the parameter, distance, side extent and tolerance.
  Result: 11 more roots mesh (577 → 588) and 41 more pass geometry/topology
  (745 → 786), none lost; every root of the three kinds now meshes with reported
  adaptations, fails a later stage, or is a typed rejection naming distance and
  bound. Remaining: 16 singular-crossing rejections (15 in one NX file whose edges
  pass within its 50.8 µm uncertainty of exact poles, one singular parameterization),
  deviations beyond 16 times, and newly geometry-accepted roots that fail tessellation
  (Milestones 22.2 and 22.4). The factor and counters are exposed in C/C++ by the
  22.10 second pass. Evidence: `tests/tolerance.rs`
  (`deviations_within_the_policy_are_counted_and_widen_the_working_tolerance`,
  `deviations_beyond_the_policy_are_typed_rejections_naming_distance_and_bound`,
  `bspline_sides_collapsed_only_within_tolerance_are_regular_and_poles_are_rejected`),
  the `brep-near-surface`, `brep-far-surface`, `brep-vertex-off-curve` and
  `brep-short-side` fixtures in `check_geometry.py`, and the reviewed corpus movement.
- **22.4 UV loop validity — delivered 2026-10-09.** The baseline had 92 roots whose
  first failure was an unlocated polygon error after topology passed: 34
  `SelfIntersection` and 21 `DegenerateLoop` at the mesh boundary, 22 trim `Limit`,
  11 trim `WrongOrientation` and 4 trim `SelfIntersection`. Classified by cause in
  [VALIDATION.md](VALIDATION.md); the causes that were the importer's are fixed: the
  mesh-boundary check used the 1e-5 UV chord tolerance as a distance (10 µm on metre
  plane charts; 55 faces) and now checks simplicity at floating-point resolution;
  polygon checks charged all n² segment pairs (21 faces) and now sweep boxes, with
  trim `max_samples` raised to its 1M ceiling (2 faces); and, one baseline face each
  plus 14 faces that surfaced behind the first fixes, trim seeds a few ulps apart,
  fitted-pcurve wobbles within the model tolerance, sub-micrometre loops against the
  UV tolerance, a seam line crossed between scan samples, a closed B-spline flagged
  `.U.` and a sphere lying outside its only loop. Every remaining loop failure names
  its `EDGE_CURVE`(s), vertex or loop and a cause class (`trim::Error::{coedge,
  other, wire, budget}`, [curved import](CURVED_IMPORT.md#uv-loop-failures)): 10
  wrong orientations (a `FACE_OUTER_BOUND` on a hole, reversed dodgy loops), 2
  self-touching loops and 1 cusp thinner than the tolerance, all in the input (the
  cusp would need trim refinement on conflict). 30 more roots mesh (607); 51 now stop
  at later tessellation budgets, refinement or the triangulator (22.2). Evidence:
  `tessstep-trim/tests/loop_validity.rs`, `tests/boundary_resolution.rs`,
  `uv_loops::tests`, `brep::scan::tests` and `tests/uv_loops.rs` with
  `brep-unknown-closure-cylinder.step`, `brep-cut-sphere.step` and
  `brep-pinched-pocket.step`. Follow-up 2026-10-10: the 27 roots of the 1.0.0
  baseline whose triangulation ended in an unlocated `Polygon(UnresolvedGeometry)`
  had two causes, both the triangulator's: a hole bridged to the wrong copy of a
  vertex an earlier bridge had repeated (23 first failures; the polygon folds and has
  no ear), and clipping across a run straight only to its fitting noise, leaving a
  remainder with no ear at floating-point resolution (4, and 3 faces behind the first
  cause). Bridges now leave the copy whose interior angle faces them, and clipping
  that finds no ear is redone once with a 1e-9 blocking slack; no other triangulation
  changes. 20 roots mesh (762) and 1 more surface-model shell; the other 7 fail later
  with named errors, and a triangulator failure is named `unresolved triangulation`.
  Evidence: `planar::tests`, `uv_loops::tests`.
- **22.5 Opt-in sewing and orientation repair — delivered 2026-10-09.** 123 `OpenShell`,
  38 `BrokenWire` and 35 `InconsistentOrientation` roots. Strict rejection stays the
  default and its outcomes are unchanged; a disconnected `CLOSED_SHELL` (faces sharing
  no edge or vertex) now fails as `UnsewnShell` (29 of the 123 roots). The repair stage
  is `tessstep_topology::{plan_sewing, orient_faces, diagnose_gap}` on raw B-reps, run
  by the curved, planar and faceted importers when `ImportOptions::repair`
  (`RepairOptions { sew, orient, sewing_factor }`, off by default, sewing tolerance ten
  times the model tolerance) asks for it: coincident boundary vertices and free edges
  merge without moving any coordinate (the sewn model is checked at the model tolerance
  plus the largest merge distance), faces flip into a consistent orientation, faces
  whose loops run against their surface are reversed when the trimmer then accepts
  them, and closed solid shells are reversed by the sign of a coarse tessellation's
  volume. `ImportedSolid::repairs()` counts every edit (`RepairReport`); a shell that
  sewing cannot close fails with its classification and the distance it would need.
  The survey and corpus runner take `--repair` / `--geometry-repair`; the repair-on
  survey meshes 642 of 1,197 roots (strict 577): 59 synthetic roots and six OpenCASCADE
  exporter roots (four `BREP_WITH_VOIDS` whose cavity shells face out of their
  cavities, an inside-out shell and a face whose loop runs against its surface).
  T-junctions, partial overlaps, missing faces and single reversed edges in consistent
  loops are not repaired. The option and report are exposed in C/C++ by the 22.10
  second pass.
  Evidence: `tests/repair.rs` in `tessstep-topology` and `tessstep-import`, ten
  generated fixtures (`repair_fixtures.py`), [VALIDATION.md](VALIDATION.md).
- **22.6 Surface models as shell meshes — delivered 2026-10-09.** `SHELL_BASED_SURFACE_MODEL` (4,127 roots,
  2,539 in 108 OpenCASCADE files) imports as open or closed shell meshes through
  `tessellate_shell`, reported as shells and never as solids. The corpus runner
  surveys them as a separate root class with their own baseline outcomes.
  Delivered: `discover_surface_models` and `import_surface_model` (Rust) reuse the
  curved profile's face and edge adapters; the profile gains `OPEN_SHELL`,
  `ORIENTED_OPEN_SHELL` and the surface model root. Each declared shell is validated
  open or closed as declared, without a solid, kept as its own B-rep (`SurfaceShell`:
  entity, closure, face entities, open boundary edges) and meshed as one connected
  component, watertight when closed, with no volume or orientation check. The survey's
  `surface_models` section, `surface_model_result`, the `surface_model` stage and two
  new baseline fields form the corpus class; files without surface models compare
  unchanged. Of the 4,127 path-level roots, 2,242 are in physically accepted inputs
  (1,756 are in `bug26451_Test_STEP.stp`, which the parser rejects); 893 mesh: 519 of
  780 OpenCASCADE, 29 of 31 NIST and 345 of 1,431 `dodgy-step-files` roots. The
  remaining exporter failures belong to 22.2 (tessellation) and 22.3 (edges off their
  surfaces); sewing is not part of this item (22.5), and C/C++ exposure came with
  the 22.10 second pass. Evidence: `tests/surface_model.rs`
  (`surface_model_open_patches_mesh_as_one_open_shell`,
  `surface_model_closed_shells_are_watertight_shells_not_solids`,
  `surface_model_rejections_are_typed_and_located`,
  `surface_model_unmodified_catia_surface_export`), `scripts/test_corpus_surface.py`,
  the authored `surface-*.step` fixtures and two hash-pinned exporter files in
  `check_geometry.py` (CATIA V5 open shells, Pro/ENGINEER closed shell), and the
  reviewed corpus movement in [VALIDATION.md](VALIDATION.md).
- **22.7 Mapped-item placements and tolerated unit encodings — delivered 2026-10-09.**
  The 9 unplaced roots turned out not to be mapped-item placements; mapped-item
  assemblies were instead rejected with `MissingPlacement` (`trj3_s1-db-214.stp`, CATIA
  V4 via debis, and `bug33261.stp`, Creo). The linker now decodes the MAPPED_ITEMs of
  occurrence shape representations, and `Model::expand` places an occurrence that has
  no context dependent placement by the mapped item its own representation and its
  parent's representation both hold (target times inverse mapping origin).
  `CARTESIAN_TRANSFORMATION_OPERATOR_3D` places as a relationship operator or mapping
  target with its reflection and uniform scale; other operators are typed rejections
  naming them. The context and product profiles accept, count (`ToleratedEncodings` on
  `LinkedAssembly`, `ContextUnits`, `RootImport` and the corpus product stage) and in
  strict mode reject explicit SI dimensions, `$` SI dimensions, starred conversion
  dimensions, untyped measures and unordered complex components. Five-parameter
  `PRODUCT_DEFINITION`s stay rejected. Named reasons for the 9 unplaced roots: a
  five-parameter definition (`A116`), a tessellated shell no representation holds
  (`Pmi049`), a B-rep related to the part only by a
  `MECHANICAL_DESIGN_AND_DRAUGHTING_RELATIONSHIP` (`bug29241`), supplemental
  constructive geometry (two NIST FTC 08 copies) and round-hole feature tool bodies
  (`trj2_f1`, `trj3_t2`, two each). Corpus: 11 more accepted product structures, 3 more
  meshed roots ([VALIDATION.md](VALIDATION.md)). Evidence:
  `mapped_items_place_occurrences_by_target_times_inverse_origin`,
  `cartesian_operators_place_with_reflection_and_uniform_scale`,
  `explicit_si_dimensions_and_unordered_unit_components_are_counted`,
  `untyped_measure_values_are_counted_and_keep_their_unit`,
  `mapped_items_place_occurrences_without_relationships`,
  `untyped_select_values_and_profile_tolerances_are_admitted_only_by_policy`.
- **22.8 Document import session and scaling benchmark — delivered 2026-10-09.**
  Measured first: parsing and decoding were not where the time went. `bug33261.stp`
  (77,956 entities, 24 roots) parses in 0.14 s, decodes every closure in 0.18 s,
  adapts in 0.42 s and tessellates for 22 s (local medians under load);
  `trj3_s1-id-203.stp` spends 19.7 of its 19.8 s tessellating (22.2). Decoding was
  already closure sized, so what a document repeated per root was discovery, a
  whole-document scan. `ImportSession` borrows a parsed
  document, runs each discovery pass once and, through `prepare`, decodes each
  profile's roots together once; each import adapts its root from the shared records.
  Results equal the free functions': a shared decode is kept only when it succeeds
  within one root's `max_work`, else the group decodes alone. `import_assembly`,
  `link_assembly`, the survey and the assembly example (with styles) use it; survey
  JSON and `assembly --meshes` output are byte-identical on all 3,227 inputs.
  `SessionBudgets` bound decoded records, adapted records and triangles per session
  (`for_document`: 4, 16 and 256 per entity, from measured maxima of 1.0, 1.86 and
  62.6) and are named in diagnostics. The nightly `session` benchmark records time and
  peak memory per stage for both inputs and a generated series where independent
  per-root import costs 20× the session's per root at 256 roots. Evidence:
  `tests/session.rs`, [VALIDATION.md](VALIDATION.md). Not shared, by measurement:
  adapted geometry between roots, since corpus closures share 1,281 of 2.33 million
  decoded records. Sharing a decode of disjoint closures is not free: the union decode
  costs 8 to 13% more than the per-root decodes it replaces, each root's closure must
  be recovered, and the union stays resident until its last root is imported (decode
  and adapt of `bug32239.STEP`'s 15 roots 0.66 → 0.80 s, survey peak RSS 180 → 198
  MiB; corpus wall time unchanged within noise). It pays only where closures overlap;
  whether the survey and `import_assembly` should keep preparing by default is open for
  review. The 22.10 second pass exposes the session in C and C++.
- **22.9 Extended fuzzing — delivered 2026-10-02.** The nightly workflow builds the
  fifteen `fuzz/Cargo.toml` targets once with AddressSanitizer, coverage and debug
  assertions and fuzzes each in its own matrix job for a fixed budget (240 s;
  `workflow_dispatch` takes a budget and a target subset) with per-target `-max_len`,
  `-timeout`, `-rss_limit_mb` and `-malloc_limit_mb`. It replaces the 14 sequential
  120 s sessions, which missed `brep_import` and gave the binary harnesses no seeds.
  `scripts/fuzz_seeds.py` derives the matrix from `fuzz/Cargo.toml` and assembles
  deterministic seeds from the authored corpora, generated layouts for the binary
  harnesses and committed findings. Each failing input is minimized with libFuzzer's
  `-minimize_crash` loop (as `cargo fuzz tmin`) and replayed. `scripts/fuzz_report.py`
  reports per target executions, coverage and features, corpus, duration and findings,
  marked `instrumented: true` only on evidence (coverage counters in the log, the
  AddressSanitizer runtime in the binary), and the stable `fuzz_smoke` tests as
  `instrumented: false`. Minimized findings are committed under
  `fuzz/regressions/<target>/` and replayed through each target's exercise function in
  normal CI; the six targets whose bodies were inline now share them through
  `fuzz/support/`. The first finding came from stable driver smoke over the new seeds,
  not an instrumented run: `brep_import` required one mesh component, but a
  `BREP_WITH_VOIDS` cavity meshes as an inward component, so the authored sphere cavity
  failed it; the oracle now requires one component per shell and the input is the first
  committed regression. Evidence: `scripts/test_fuzz.py`,
  `every_fuzz_target_has_a_stable_replay`, `committed_fuzz_findings_replay_without_failure`,
  `regression_replay_names_every_failing_finding_and_unknown_target`, and
  [VALIDATION.md](VALIDATION.md). First instrumented nightly runs: the scheduled runs of
  2026-10-11 to 2026-10-13 ran all 15 targets instrumented and clean (236 million
  executions on 2026-10-13, run 38377980764).
- **22.10 Public interfaces — delivered 2026-10-10; ABI 1.0 export set frozen.**
  Single-root curved import, root and unit discovery, representation selection and
  import options (strictness, budgets, tolerance policy, repair opt-in) in C and C++,
  under installed-consumer tests; static linkage in the CMake package. After this item
  the C ABI export set is frozen for v1.0. Delivered in three passes (2026-10-02,
  10-09, 10-10), on main as each Rust item landed:
  - the extensible `ts_import_options` (168 bytes: strict, repair and surface-model
    flags; decode, tessellation, assembly, session and presentation budgets; model and
    tessellation tolerances; sewing and deviation factors), `ts_import_options_init`
    and `ts_import_options_for_document`; `ts_solid_info` and `ts_surface_model_info`
    are the other extensible records. The development drafts were folded, so ABI 1.0
    publishes exactly one revision of each; later 1.x revisions may only append;
  - `ts_document_import_solid` (curved or faceted profile, explicit length and
    plane-angle units, adaptation counters, 22.3 deviations and working tolerance,
    22.5 repair report) and `ts_document_import_surface_model` (22.6, a retained
    `ts_surface_model` of shell meshes, never solids);
  - `ts_document_discover` with a retained `ts_discovery` (solid, tessellation,
    annotation and, on request, surface-model roots; zero-copy representation arrays;
    context units or located unit errors; representation choices);
  - the `ts_import_session` handle (22.8): uniquely owned, retains its document,
    serializes calls; discovery, prepare, solid, surface-model, tessellation,
    presentation and assembly import, statistics;
  - `ts_document_import_assembly_with_options`, `ts_assembly_get_adaptations` (22.7)
    and per-root reports (`ts_assembly_root_report_at`, `ts_discovery_root_report_at`:
    tolerated context encodings and failure text);
  - failure text as handle state, not per-thread state (the header promises no global
    error state): `ts_import_session_last_message` and the per-root reports; the C++
    `Error::message`;
  - `TessSTEP::TessSTEP_static` with the system libraries rustc reports;
  - the freeze: the record [abi/exports.txt](../abi/exports.txt) (95 functions, all
    `1.0`), checked against the sources and header by `tessstep-capi/tests/abi.rs`
    (which pins the 1.0 set) and against the installed shared library and static
    archive by `check_capi.py`; the policy (frozen symbols, layouts and values; 1.x
    adds only) is in [C API](C_API.md#abi-1-freeze).
  Evidence: `tessstep-capi/tests/{discovery,session,abi}.rs`,
  `default_options_are_the_rust_defaults`, the installed C/C++ consumers (shared,
  C-only and static). Post-1.0 ([C API](C_API.md#post-10)): document-level failure
  text, annotation unit text, surface models in sessions and assemblies, scene limits
  in the assembly options, Debug static consumers.
- **22.11 Release engineering — pre-release delivered 2026-09-27; 1.0.0 set
  2026-10-10.** The `v0.1.0` tag (c80d0ff) ran the Release workflow green (run
  36303712652): the tag/version check, the full CI verify with the external corpus and
  the MSRV, release-mode tests, packages for Linux x86-64, Windows x86-64 and macOS Apple
  Silicon and Intel built and consumer-tested outside the source tree, and a draft
  GitHub release holding the four archives and their SHA-256 files, reviewed and
  published as a GitHub pre-release on 2026-10-10. The workspace, the CMake package and its consumers are now version 1.0.0
  (`find_package(TessSTEP 1.0)`), [CHANGELOG.md](../CHANGELOG.md) records 0.1.0 and
  1.0.0, and the ABI 1.0 export set is frozen (22.10, `abi/exports.txt`). The reviewed
  corpus baseline saved on 2026-10-10 is the 1.0.0 compatibility record
  ([VALIDATION.md](VALIDATION.md)). With the maintainer's approval `main` was pushed
  (CI run 38025959160 green on every target) and `v1.0.0` tagged at 86f2536; its
  Release workflow (run 38026639055) passed and drafted the 1.0.0 release with the four
  archives and their checksums; on every target the package exports exactly the 95
  ABI 1.0 symbols and its installed consumers pass (shared and static, Debug and
  Release). Publishing the 1.0.0 draft is the maintainer's step.

Decision to record here before v1.0: EXPRESS rules, functions and WHERE clauses stay
unevaluated, so v1.0 claims structural AP242/AP214/AP203 coverage and no AP
conformance. Evaluating rules is post-1.0 work unless a consumer requires it.

Order: 22.11's pre-release tag first, as soon as CI is green; 22.1 and 22.8 together
(budgets need the session's measurements); 22.2, 22.3 and 22.4 next, since they hold
the most exporter roots and the Onshape file; then 22.5, 22.6 and 22.7 in parallel;
22.9 runs throughout; 22.10 accompanies each public operation and closes last.

Exit criteria: CI is green on all four targets and the release workflow has produced
a reviewed pre-release. The Onshape export imports as a closed solid. Every corpus
root has a named outcome; no root fails on an unclassified polygon error, an
unnamed budget or an unnamed entity. Every repair and tolerance adaptation is
opt-in or counted, and reported per root. Surface models import as shells. The C
ABI covers discovery, curved import and import options, and its export set is
frozen with the consumer tests passing on the installed package.

Result (2026-10-10): every item is delivered and integrated on `main` with reviewed
corpus movements, and the 1.0.0 baseline is saved: 762 of 1,197 shape roots mesh (577
before), 1,128 surface models mesh as shells, no input times out, and no file lost a
meshed root ([VALIDATION.md](VALIDATION.md)). Exit criteria met locally: the Onshape
export imports as a closed solid; every repair and tolerance adaptation is opt-in or
counted and reported per root; surface models import as shells; every budget failure
names its budget; the C ABI covers discovery, curved import and import options and its
export set is frozen with the installed-package consumer tests passing. The 27 roots
whose triangulation ended in an unlocated `Polygon(UnresolvedGeometry)` were resolved
by the 22.4 follow-up: 20 mesh, 7 fail later with named errors, and no root fails on
an unclassified polygon error. The pinned Onshape exporter file moved to post-1.0
(decided 2026-10-10). CI is green on every target for the pushed `main` (run
38025959160: Linux, macOS and Windows builds and tests, the MSRV, dependency policy
and the external corpus), and the `v1.0.0` tag's Release workflow passed (run
38026639055), drafting the 1.0.0 release with its four archives for review.

**Milestone 23 — PreForm's import settings (planned 2026-10-13).**

Goal: imported the way PreForm imports, TessSTEP keeps every body that HOOPS Exchange
meshes, places no body twice, and is no slower or larger than HOOPS on large files.
PreForm imports through the C++ `import_assembly` with a 12.5 µm chord, a 30° normal
angle, sewing and orientation repair, and raised output budgets.

**Baseline.** On 2026-10-13 both libraries ran on the 54 files of the internal PreForm
CAD collection, each called as PreForm calls it: HOOPS Exchange 2026.5 imports, sews at
0.1 mm and runs accurate tessellation. Each tool ran three times on one thread; nothing
from the collection is committed.

| | TessSTEP | HOOPS |
| --- | --- | --- |
| Files with closed meshes | 49 with the tolerant parse, 47 with PreForm's strict parse | 44, plus 9 open |
| Time on the 48 files both mesh | 11.4 s (geometric mean 0.79× HOOPS's per file) | 14.5 s, plus 0.77 s SDK load |
| Peak memory and triangles on those files | 0.45× and 0.37× HOOPS's | — |
| `Bad_STEP.stp` (55.7 MB, 646 roots) | 641 bodies, 13.0 s, 1.24 GB peak | 646 bodies (6 open), 10.1 s, 0.86 GB |

Wherever both meshes are closed, volumes agree within 0.015 %.

**Reach in the external corpus.** The same comparison ran over the 2,072 corpus inputs
that hold solids or surface models; 402 of them come from real exporters (OpenCASCADE
dataset, NIST, stepcode, foxtrot). Of those 402:
- HOOPS meshes 396: 296 closed, 100 open;
- TessSTEP closes 271, 30 of them where the HOOPS mesh is open;
- 81 contain only surface models, which `import_assembly` does not import, so PreForm
  gets no body from TessSTEP; HOOPS meshes 35 of them closed and 46 open;
- 48 lose some or all bodies in TessSTEP where HOOPS meshes. The causes are per-face
  planar budgets (8 files), edges or vertices beyond the deviation policy (11), horn
  tori (7), and 22 files with one-off geometry, trimming or decoding causes.

On the 267 real files both mesh fully, TessSTEP takes 0.73× HOOPS's time and is faster
on 173. It is slower on large multi-body files: `bug30790` 12.9 s against 4.7 s,
`trj4_s1-ac-214` 8.4 s against 1.7 s. No corpus file fails on the coarse angle (23.1)
or on sibling representations (23.3). The corpus is mostly older OpenCASCADE test data,
so these counts measure reach, not the frequencies PreForm users see.

Each item lands with an authored fixture, typed outcomes and reviewed corpus movements.
The collection benchmark is repeated on every item.

- **23.1 Coarse normal angles converge.** `Core__2_.step` (Onshape) meshes at 15° and
  below. At 20° and above it fails at every chord from 12.5 to 100 µm: one face
  exhausts `max_rounds`. A looser angle must never turn a converging face into a
  failure.
- **23.2 Per-face planar budgets follow the requested mesh.** At 12.5 µm three NIST
  files lose their only body to the 10M `planar.max_work` per face:
  `nist_ctc_02_asme1_rc`, `nist_ftc_09_asme1_rd` and `nist_stc_09_asme1_ap242-e3`. All
  three mesh at 25 µm. In the corpus the same budgets, `planar.max_work` and
  `planar.max_vertices`, cost 18 roots in 8 files. The OpenCASCADE `as1_motor`
  assembly loses 8 of 22 bodies, and the CAx-IF `trj3_s1-ac-214` spaceship 7 of 8.
  Raising `max_tessellation_work` does not lift the per-face budgets, and C cannot set
  them. Size the per-face budgets from the solid's tessellation budget or from the face
  boundary, or make planar triangulation cheaper; every budget stays named.
- **23.3 Exact and tessellated sibling representations.** CATIA V5 writes a B-rep and
  a `TESSELLATED_SOLID` whose geometric link is `$`. Each is related to the product's
  shape representation by its own `SHAPE_REPRESENTATION_RELATIONSHIP`.
  `import_assembly` places both, so the four NIST UUID plate exports from CATIA
  V5-6R2022 import twice (+100 % volume) where HOOPS has one body. Representation
  selection pairs such siblings and keeps the preferred one.
- **23.4 Tolerant parsing in consumers.** The C++ `Document::parse(bytes, options)`
  overload is strict, like `ts_document_parse`. PreForm calls it and rejects two files
  that HOOPS opens: a literal backslash in `827-9999-904 rev c.stp` and a `$`
  authorisation in `nist_stc_10_asme1_ap242-e2.stp`. Both import through the ABI 1.1
  overload `parse(bytes, options, nullptr)`. Fix the consumer (PreForm side), and
  state the strict default prominently in the C++ header and [C API](C_API.md).
- **23.5 The last `Bad_STEP.stp` bodies and their corpus siblings.** Five of 646 roots
  still fail. Two of their causes also cost real corpus files:
  - **Deviations beyond the policy** (a vertex 27.8 µm off its curve; 11 corpus files).
    PreForm's HOOPS path sews at 0.1 mm, while TessSTEP accepts at most 16 times the
    model tolerance or the chord. When repair is requested, accept deviations up to
    the sewing distance, as a counted adaptation.
  - **Horn tori** (major radius equal to minor radius; 8 roots in 7 corpus files). Add
    them as the limit of the spindle torus, with the apex on the axis.
  - **The rest:** a pinched face (a 51 µm lobe under a 17 µm working tolerance), a
    pinched vertex (`NonManifoldVertex`, rejected by design) and a plane overlapping
    a fillet along generators that are no edges.

  Each either imports with a counted adaptation or opt-in repair, or stays a decided,
  named rejection.
- **23.6 Parallel import in C and C++.** On `Bad_STEP.stp`, 12 of TessSTEP's 13.0 s
  go to importing 646 independent roots one after another on a single thread.
  - Append a thread count to `ts_import_options` (ABI 1.2) for
    `TessellationOptions::threads`.
  - Import independent roots of an assembly or session concurrently, with results that
    do not depend on the thread count.
  - Reduce peak parse memory (80 bytes per value today).
- **23.7 PreForm integration.** The PreForm reader (`PP_WITH_TESSSTEP`) has these gaps:
  - it is uncommitted;
  - it links a checkout's `target/release` archive by path;
  - it logs failed roots without telling the user;
  - it has no fallback to HOOPS.

  It also passes the 12.5 µm chord to every model, where its HOOPS path coarsens the
  chord for models larger than the largest build volume. Consume the release package
  (`TessSTEP::TessSTEP_static`, and the iOS XCFramework for the iPad build), parse
  tolerantly (23.4), scale the chord as the HOOPS path does, and decide the
  failed-root policy. A per-file fallback to HOOPS when TessSTEP loses a body would
  turn every remaining gap into today's behaviour instead of a lost part. Add a
  PreForm regression over the collection. The work lands in PreForm; TessSTEP
  supplies the package and any API it needs.
- **23.8 Releases.** Release 1.1.0 from the prepare-release workflow: ABI 1.1, plus the
  HOOPS parity, `Bad_STEP.stp` and iOS changes of the Unreleased section. Release
  1.2.0 with this milestone.
- **23.10 Surface models in sessions and assemblies.** 81 of the 402 real corpus files
  hold only `SHELL_BASED_SURFACE_MODEL` roots. They come from I-DEAS, OpenCASCADE,
  Unigraphics, Pro/ENGINEER, CATIA V5 and other exporters. `import_surface_model`
  meshes 669 of these shells, but the session and `import_assembly` skip surface models
  (a post-1.0 item in [C API](C_API.md#post-10)). Place surface-model roots in sessions
  and assemblies on request, as shell meshes that report open or closed and are never
  called solids. Sewing their open shells is opt-in. Which shells a consumer turns into
  printable bodies stays its own policy: in PP-31465, a SOLIDWORKS thread surface body
  in the same space as its solid filled a hole when the surface was printed.
- **23.9 Reviewed corpus baseline.** The baseline has not been saved since 9d7b376: 778
  → 786 meshed shape roots, 1,130 → 1,133 surface models. Run `corpus.py --check`,
  review it and save, then again with each item above.

Order, by measured impact (re-evaluated 2026-10-13):
1. 23.9, 23.4 and 23.8 (1.1.0), since they change no import code.
2. 23.3, because a duplicated body is wrong output that no fallback detects.
3. 23.10 (81 real files) and 23.2 (11 files).
4. 23.6, for the large multi-body files.
5. 23.5 (deviation policy, 12 files; horn tori, 8 files).
6. 23.1 (1 file; until then PreForm can pass 15° or less, since TessSTEP's normal angle
   and HOOPS's segment angle measure different things).
7. 23.7 closes the milestone, with the C/C++ exposure of 23.2, 23.6 and 23.10 in one
   ABI 1.2 revision.

Exit criteria:
- Under PreForm's settings, the collection benchmark loses no body HOOPS meshes, apart
  from decided rejections, and places no body twice.
- `Bad_STEP.stp` imports within HOOPS's time and peak memory.
- The external corpus check has no regressions.
- PreForm reads STEP through the released package.

Milestone 4 connects physical instances to schema-aware decoding. Milestone 5 builds
product/representation/units/assembly semantics. Milestones 6–10 build independent math,
analytic curves/surfaces and NURBS. Milestones 11–12 establish topology validity states
and UV trimming. Milestones 13–17 implement shared-edge sampling, planar and curved
face tessellation and the watertight solid pipeline. Milestones 18–19 supply independent assembly assets and appearance; Milestone 20 imports
existing tessellations; Milestones 21–22 extend systematic AP242 coverage and industrial hardening. Conformance is evaluated by stage, not by whether a model opens.

The public-interface workstream must deliver both the stable C ABI and C++ RAII
wrapper, including typed errors and the installed `TessSTEP::TessSTEP` CMake
target. Begin with the first usable document operations; add immutable zero-copy
mesh views as the mesh pipeline becomes available. Design storage with that
contract in mind. C/C++ installed-package consumer tests and ABI containment are
release gates, not optional bindings work. See [C_API.md](C_API.md). This contract
also applies as product and geometry operations become public. The first physical-document
slice now implements both interfaces, typed results and the shared and static CMake
package targets; mesh import and retained read-only views, selected faceted/planar
solid import, existing-tessellation and presentation import, assembly import, root
and unit discovery with representation selection, single-root curved solid import,
surface-model import, import sessions, and extensible import options with the
tolerance policy and opt-in repair, failure text and per-root reports are now
implemented. Public schema decoding and B-rep construction remain Rust-only. The ABI
1.0 export set is frozen in [abi/exports.txt](../abi/exports.txt); ABI 1.x may only
add to it ([C API](C_API.md#abi-1-freeze)).
