# Changelog

TessSTEP follows [Semantic Versioning](https://semver.org/). The C ABI version is
independent of the package version; see [docs/C_API.md](docs/C_API.md). Each release's
corpus compatibility record is `corpus/baseline.json` at its tag; see
[docs/VALIDATION.md](docs/VALIDATION.md) for the reviewed totals.

## Unreleased

Parity work against HOOPS Exchange on the PreForm CAD collection.

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
