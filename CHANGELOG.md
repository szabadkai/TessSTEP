# Changelog

TessSTEP follows [Semantic Versioning](https://semver.org/). The C ABI version is
independent of the package version; see [docs/C_API.md](docs/C_API.md). Each release's
corpus compatibility record is `corpus/baseline.json` at its tag; see
[docs/VALIDATION.md](docs/VALIDATION.md) for the reviewed totals.

## Unreleased

Milestone 22 (industrial hardening); see [docs/ROADMAP.md](docs/ROADMAP.md).

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
