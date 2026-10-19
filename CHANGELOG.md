# Changelog

TessSTEP follows [Semantic Versioning](https://semver.org/). The C ABI version is
independent of the package version; see [docs/C_API.md](docs/C_API.md). Each release's
corpus compatibility record is `corpus/baseline.json` at its tag; see
[docs/VALIDATION.md](docs/VALIDATION.md) for the reviewed totals.

## Unreleased

Parity work against HOOPS Exchange on the PreForm CAD collection. The 646-root
support attachment of that collection, imported as PreForm does (12.5 µm chord, 30°,
sewing and orientation repair), lost 26 bodies before the changes below and loses 4
(a pinched face, a pinched vertex, a vertex 28 µm off its curve, and a plane cutting
a fillet along generators that are no edges), 3 once the C options can set a 0.1 mm
sewing distance; the survey's default import meshes 617 of its roots instead of 545.

- Horn tori, spindle tori written as `TOROIDAL_SURFACE` and far-nappe cones import
  (Milestone 23.5). A `TOROIDAL_SURFACE` whose minor radius exceeds its major radius
  (as OpenCASCADE writes spindles) is the spindle part its face's boundary lies on, and
  equal radii are a horn torus, the limiting spindle whose two axis points meet at one
  apex: `Surface::degenerate_torus` accepts equal radii for the apple, curve ends and
  loop joins at the apex take the singular line they approach, and a face spanning
  both lines gets a second apex vertex. A `CONICAL_SURFACE` face lying only beyond the
  apex (OpenCASCADE writes them) is on the far nappe and gets a kernel cone opening
  the other way. Supplied B-spline pcurves have their control points clamped to the
  surface domain, as fitted images do, so a pcurve written beyond its patch edge no
  longer fails trimming. Under PreForm's settings nine corpus roots in eight
  OpenCASCADE files and one attachment body now mesh; where HOOPS's meshes are closed
  the volumes agree within 0.06 %. Faces that reach a horn apex mesh where the model
  tolerance resolves its cusp. Fixtures:
  `brep-toroidal-apple/lemon`, `brep-horn-zone/seam/half/apple`,
  `brep-far-nappe-cone`.
- `RepairOptions::sewing_distance` sets an absolute sewing tolerance in metres, as
  PreForm asks HOOPS Exchange to sew at 0.1 mm: the sewing tolerance is the larger of
  `sewing_factor` times the model tolerance and the distance, and with sewing the
  curved import accepts edge and vertex deviations up to it, counted like every
  accepted deviation (the bound in a rejection names `the sewing distance`). The
  default 0 changes nothing; `ts_import_options` gains the field in ABI 1.2. At
  0.1 mm, two more corpus roots and one more attachment body mesh, within 0.02 % of
  HOOPS's volumes, and deviations beyond 0.1 mm stay rejected. The `brep` example
  takes `--deviation-floor` and `--sewing-distance`.
- Curved import takes `ImportOptions::deviation_floor`, a distance the accepted edge
  and vertex deviation never falls below whatever the factor makes of the model
  tolerance; a factor of 1 and strict import still accept none. The C API sets it
  from `chord_m` on every import, so an inconsistency the requested mesh cannot show
  no longer rejects a solid (11 of the attachment's bodies, 2 to 9 µm off under a
  0.1 µm floor tolerance). `ImportedSolid::tessellate` no longer refuses a chord below
  the largest accepted deviation: the boundary facets beside such an edge lie up to
  that deviation off the surface in addition to the chord (65 roots of the survey).
  A vertex at the seam of a B-spline written closed without its closed flag takes the
  domain end its edge direction asks for, and an open edge whose `SAME_SENSE`
  contradicts the order of its vertices along the curve follows the vertices, counted
  in `Adaptations::reversed_senses` (strict import rejects it). The `brep` example
  sizes its budgets from the document and takes `--angle`.
- Mesh loop healing for three exporter defects, each counted or located, none
  moving a coordinate. A face that lies on a neighbouring face's surface facing away
  from it (a plane strip written over the strip of a fillet cylinder it cuts 0.03 µm
  below its tangent line: a lens of no thickness, which meshed into the same
  triangles twice) is merged into the neighbour before planning and counted in
  `Adaptations::collapsed_faces`; a duplicate triangle that remains names the face
  of its repeat instead of none. A sliver face whose edges coincide only within the
  working tolerance an accepted vertex deviation implies is collapsed at that
  tolerance, as every later check already used it. Edge sampling no longer samples a
  knot within the model tolerance of the edge's vertex (an exporter's triple knot 3
  pm before it, which doubled a face boundary back on itself at the next edge's image
  of the vertex). Fixtures: `brep-lens-flap.step`, `brep-crescent-sliver.step`.
- Planar triangulation bridges a hole that the holes listed after it enclose. Holes
  were bridged into the face polygon in face order, and a bridge may not cross an
  unbridged hole, so a face whose first hole saw no polygon vertex past the later
  ones (a square through hole between two L-shaped ones, as in four customer solids)
  failed with `unresolved triangulation`. Such a face now bridges its remaining holes
  leftmost first, which always has a visible bridge. Faces whose holes bridged in
  face order keep their triangles (`brep-enclosed-hole.step`).
- Assemblies written by HOOPS Exchange (Shapr3D exports among them) and the CATIA V4
  STEP interface link: an item defined transformation that lists the parent's placement
  before the child's frame is read by membership, counted as
  `ToleratedEncodings::reversed_transform_items` and rejected when strict
  (`tessstep_ap242::adapt_with_policy`, `Policy`). The Shapr3D keycaps sample imports
  as a 265-instance scene; its HOOPS reference volume counts every body twice.
- Tessellation meshes fillet bands whose refinement walked one facet per round
  around the ring (a face that runs out of rounds is refined again with split
  closure), and refines a boundary chord that cuts across a nearby edge, such as one
  tangent at their common vertex, instead of failing the resolution check. Meshes
  that already passed are unchanged; the NIST FTC 06, FTC 11, CTC 04 (AP242) and,
  at a 0.1 mm chord, CTC 02 parts now mesh within 0.06% of the closed HOOPS meshes.
- Curved import collapses loop spurs: two consecutive edges of a face loop that run
  back along each other within the working tolerance (a slab top meeting a fin
  flush with the slab's wall, a B-spline running back over the arc it continues, two
  fitted B-splines leaving a vertex together and crossing again) are split where
  they part, the coincident halves merged as sliver-face edges are, and the
  out-and-back run dropped, counted in the new `Adaptations::collapsed_spurs`;
  strict import keeps the loop. Tangent cusps, corners of bodies whose edges are a
  few tolerances long and three-edge sliver faces are untouched. Two Open CASCADE solids
  of the PreForm collection rejected as tangent edges at a vertex now mesh; the
  third goes on to a pinched face whose lobe is three tolerances wide.
- Curved import heals three face loop defects. A `FACE_OUTER_BOUND` written on a
  clockwise hole loop, beside one counterclockwise loop that encloses the others, now
  bounds a hole: this is counted in the new `Adaptations::relabelled_outer_bounds`,
  and strict import keeps the label. A cone generator whose far `LINE` origin rounds
  its apex end below the cone now keeps its exact image. A fitted edge sampled
  between half and all of the model tolerance off its surface now has its peak
  refined, so an excursion between samples is counted. The NIST STC 06 and STC 07
  parts now mesh within 0.03% of the closed HOOPS meshes.
- Parsing is tolerant by default: string backslashes that start no directive are kept
  literally and `$` may stand for FILE_DESCRIPTION/FILE_NAME strings, each counted in
  `Document::deviations` and reported as one warning; `Conformance::Strict`
  (`parse_with`, `stepdump --strict`) rejects them. `ParseLimits::for_input` sizes
  the entity, record, value and aggregate budgets from the input length (stepdump,
  survey, assembly, generated validators). Parsed documents take a third less memory.
  C ABI 1.1 adds `ts_parse_options_for_input`, `ts_document_parse_with_policy` and
  `ts_document_deviation_count`; `ts_document_parse` stays strict.
- The static C library builds for `aarch64-apple-ios` and `aarch64-apple-ios-sim` with
  an iOS 17 deployment target (`.cargo/config.toml`), and `scripts/ios_sim_run.py` runs
  simulator test binaries through `simctl spawn`; the `tessstep-capi` tests pass in an
  iPad simulator. See [docs/C_API.md](docs/C_API.md#ios).
- CI builds that library for both targets, runs the `tessstep-capi` tests in a booted
  simulator, and packages `TessSTEP.xcframework`, whose slices must carry only objects
  for their platform no newer than iOS 17 and exactly the ABI's C symbols, and against
  which the standalone C11/C++17 consumers link for devices and pass in the simulator.
  Releases ship it as `tessstep-<version>-ios.zip`.
- Releases take their notes from the version's CHANGELOG section, with links pinned to
  the tag, and the **Prepare release** workflow sets the version, dates the Unreleased
  section, pushes the commit and tag to `main` and starts the release from the Actions
  tab. See [docs/RELEASING.md](docs/RELEASING.md).

## 1.0.0 — 2026-10-10

Milestone 22, industrial hardening; see [docs/ROADMAP.md](docs/ROADMAP.md) and the
reviewed evidence in [docs/VALIDATION.md](docs/VALIDATION.md). EXPRESS rules, functions
and WHERE clauses remain unevaluated: 1.0 claims structural AP242/AP214/AP203 coverage,
not AP conformance.

- Tessellation refines in a curvature-anisotropic metric with polar caps, reuses
  evaluations, passed facets and edge incidence across rounds and passes, and can mesh
  a pass's faces on threads (`TessellationOptions::threads`) with identical results.
  Every budget failure names its budget and value. The Onshape export imports as a
  closed solid.
- Curved import accepts edges off their surfaces and vertices off their curves within
  `ImportOptions::deviation_factor` (16 × the model tolerance by default) as counted
  adaptations, and names the distance and bound beyond it. B-spline sides collapsed
  only within the tolerance are regular chart sides.
- UV loop failures name their `EDGE_CURVE`s, vertex or loop and a cause class; the
  importer's chart causes (boundary resolution, polygon-check cost, `.U.` closure,
  sphere charts and others) are fixed, and the triangulator bridges holes at the copy
  of a repeated vertex that faces them, so no root fails on an unclassified polygon
  error.
- Opt-in, tolerance-bounded sewing and orientation repair (`ImportOptions::repair`)
  with a per-solid `RepairReport`; strict rejection stays the default and classifies
  unsewn closed shells.
- `SHELL_BASED_SURFACE_MODEL`s import as open or closed shell meshes, never as solids.
- Assemblies place occurrences by mapped items and 3D Cartesian operators, and
  tolerate counted exporter unit encodings (`ToleratedEncodings`).
- `ImportSession` shares discovery and decoding across a document's roots; a nightly
  benchmark records per-stage time and memory.
- Nightly coverage-guided fuzzing of all fifteen targets under AddressSanitizer with
  assembled seeds, minimized findings and replayed regressions.
- C ABI 1.0 frozen at 95 exports (`abi/exports.txt`): discovery and representation
  selection, single-root solid and surface-model import, extensible import options
  (strictness, budgets, tolerance policy, repair), import sessions with failure text,
  assembly import options and adaptations, and a static library target
  (`TessSTEP::TessSTEP_static`). Consumers now require `find_package(TessSTEP 1.0)`.
- Corpus baseline at 1.0.0: 3,227 compatible inputs, 762 meshed shape roots of 1,197
  (577 at 0.1.0), 799 past geometry and topology, 1,128 meshed surface models and 596
  accepted product structures.

## 0.1.0 — 2026-09-27 (pre-release)

The first tagged build. It exercised the release workflow, the four packaging targets
(Linux x86-64, Windows x86-64, macOS Apple Silicon and Intel) and the installed-package
consumer gate, and was left as a draft GitHub release for review.

- Part 21 streaming parser with source spans, resource budgets and reference
  diagnostics; `stepdump`.
- EXPRESS frontend, Rust bindings and structural instance decoding; `expressc`. The
  AP242, AP214 and AP203 long forms compile and validate the corpus structurally.
  EXPRESS rules, functions and WHERE clauses are not evaluated, so no AP conformance
  is claimed.
- Product, representation, unit and assembly graphs with evaluated placements.
- Independent math, analytic curves and surfaces, and bounded NURBS.
- Structural topology, UV trimming, shared-edge sampling, planar and curved face
  tessellation, and owned manifold shell and solid meshes.
- Mesh scenes with shared assets, nested placements and appearance overrides.
- STEP import: faceted, planar and curved B-rep solids (elementary, B-spline and swept
  surfaces, poles, cavities, supplied or computed pcurves), existing tessellations
  and annotation tessellations, assemblies as coloured scenes, root and context-unit
  discovery, representation selection, and document-sized import budgets with named
  budget diagnostics (Milestones 20, 21 and 22.1).
- C ABI 1 (64 exports) and a C++17 RAII wrapper in the `TessSTEP::TessSTEP` CMake
  package: document inspection, schema selection, planar and faceted solid import,
  existing-tessellation, presentation and assembly import, mesh, scene and appearance
  views.
- Corpus baseline at the tag: 3,227 compatible inputs, 577 meshed shape roots and 585
  accepted product structures.
