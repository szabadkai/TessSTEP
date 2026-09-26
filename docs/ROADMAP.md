# Milestones

**Integration update:** selected planar `FACETED_BREP` and straight-edge
`MANIFOLD_SOLID_BREP` → validated geometry → owned
solid mesh is now available in Rust/C/C++. See [STEP_IMPORT.md](STEP_IMPORT.md) for
explicit units, reduced-profile scope and evidence. Existing triangulated STEP
tessellations import directly as owned meshes and tessellated annotations as placed
graphics, with discovery and representation selection (Milestone 20, Rust/C/C++); see
[EXISTING_TESSELLATIONS.md](EXISTING_TESSELLATIONS.md). Curved `MANIFOLD_SOLID_BREP`
import with elementary and B-spline geometry, computed pcurves and discovered context
units is available in Rust; see [CURVED_IMPORT.md](CURVED_IMPORT.md). The milestone
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
STEP geometry adaptation and industrial hardening are not claimed. Read ARCHITECTURE.md, CONFORMANCE.md and the existing tests before starting every milestone. Finish code,
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
and anisotropic refinement remain pending.

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
- **Default parse budgets.** The defaults (4M total values, 1M entities, 2M records)
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
- **Disconnected closed shells.** Some exporters write `CLOSED_SHELL`s whose faces
  share no edges or vertices. Strict rejection stays the default. Any sewing must be
  an explicit, opt-in, tolerance-bounded repair stage whose repairs are reported.
  Reports should classify this defect separately from other open-shell failures.
- **Per-root import cost — decoder part delivered 2026-09-26.** Selected-root decoding
  used to scan and charge every document entity per root; it now visits only the
  closure, ordered through the entity index, which also removed decode budget
  exhaustion on large files. Sharing adapter state across roots (a document-level
  import session) and a benchmark showing that assembly import scales with total
  closure size, not roots × document size, remain.
- **Root and unit discovery — delivered 2026-09-26.** `discover_solids` finds every
  solid root, the shape representations containing it and their context's SI or
  conversion-based length and plane-angle units and length uncertainty, through a
  bounded context profile. Missing or conflicting units are per-root errors, never
  defaults. Milestone 20 applies the same rules to shape tessellations and annotation
  occurrences. Linking roots to product definitions and placements (Milestone 5
  graphs) remains. Evidence: `discovery_reads_context_units_and_uncertainty`,
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

- **Toroidal surfaces, ring and degenerate — ring tori delivered 2026-09-26.** Ring
  `TOROIDAL_SURFACE`s import through the curved profile; degenerate tori remain.
  `DEGENERATE_TOROIDAL_SURFACE` has a minor radius larger than its major radius (here
  1 mm over 0.25 mm), and `select_outer` picks the outer (apple) or inner (lemon) part
  of the self-intersecting torus. The surface crate evaluates only ring tori. Add
  spindle tori with the selected part as the parameter domain and the two axis points
  as explicit singularities.
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

- **Supplied pcurves — 3D curves delivered 2026-09-26.** `SURFACE_CURVE`, `SEAM_CURVE`
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
  `UNIFORM_*` and `BEZIER_*` remain.** `QUASI_UNIFORM_SURFACE` (163 faces, 18 files)
  and `QUASI_UNIFORM_CURVE` (761 edges, 24 files), including rational complex
  instances, carry no knot list. Derive knots as ISO 10303-42 defines them and reuse
  the NURBS evaluators. `UNIFORM_*` and `BEZIER_*` forms follow the same rule (3 uses
  in total).
- **Hyperbolic and parabolic edges.** `HYPERBOLA` / `PARABOLA` edge geometry (22 edges,
  4 files) has evaluators but no adapter or trim-parameter mapping.
- **Cavity shells — delivered 2026-09-26:** `Solid::voids`, composed
  `ORIENTED_CLOSED_SHELL` orientation and per-shell volume checks. The 29
  `BREP_WITH_VOIDS` roots hold their voids as `ORIENTED_CLOSED_SHELL`s (83 uses).
  Compose the shell orientation and extend the solid checks to nested cavities.
- **Point bounds are not exporter specific — covered by the delivered pole item.** 276
  of the 287 `VERTEX_LOOP` face bounds occur in OpenCASCADE and NIST files, so the
  pole item above is general.
- **Surface models — scope decision.** 4,127 `SHELL_BASED_SURFACE_MODEL` roots
  outnumber the 1,460 `MANIFOLD_SOLID_BREP` roots; 2,539 of them are in 108
  OpenCASCADE files. The corpus runner does not survey them, although
  `tessellate_shell` already meshes open shells. Decide whether STEP import accepts
  `OPEN_SHELL` / `CLOSED_SHELL` surface models, reporting open results as shells,
  never as solids.
- **Topology-free trimmed surfaces — scope decision.** `CURVE_BOUNDED_SURFACE`
  (3,216 records, 18 files) occurs almost only in `GEOMETRIC_SET`s, not as face
  geometry. Meshing it needs boundary-curve trimming without B-rep topology. Decide
  whether it is in scope; otherwise name it as unsupported.
- **Rare edge and face geometry.** These occur almost only in `dodgy-step-files`:
  `RECTANGULAR_TRIMMED_SURFACE` (38 faces), `OFFSET_SURFACE` (13),
  `RECTANGULAR_COMPOSITE_SURFACE` (4), `TRIMMED_CURVE` (42 edges; delivered),
  `PCURVE` used directly as edge geometry (17), `COMPOSITE_CURVE` (14),
  `OFFSET_CURVE_3D` (5), `DEGENERATE_PCURVE` (4) and `INTERSECTION_CURVE` (2;
  delivered). Give each a named, typed
  unsupported outcome first; add an adapter when a reviewed exporter file needs one.
- **Reproducible coverage survey.** Add this survey to `scripts/corpus.py`: count
  face-surface, edge-curve and root types per source from decoded records and keep
  them in the baseline. The existing `unsupported_entity_types` summary names only
  the first rejection per root, so it hides entities behind an earlier failure.

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
slice now implements both interfaces, typed results and the shared CMake package;
mesh import and retained read-only views, selected faceted/planar solid import and
existing-tessellation and presentation import are now implemented. Public schema
decoding, B-rep construction, curved tessellation and discovery entry points remain
pending.
