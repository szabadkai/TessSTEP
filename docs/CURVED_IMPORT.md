# Curved B-rep import

`tessstep_import::import_brep_solid` converts a selected `MANIFOLD_SOLID_BREP` whose
faces lie on elementary or B-spline surfaces into a validated B-rep and owned solid
mesh. It is a superset of the edge-based [planar profile](STEP_IMPORT.md) and uses
the same topology, UV trimming, shared-edge sampling, tessellation and mesh checks.
Mesh positions are metres; triangle `face_ids` are the STEP face entity IDs.

This is a bounded import profile, **not AP203/AP214/AP242 conformance**. The original
reduced [B-rep profile](../corpus/geometry/brep.exp) supplies generated metadata for
named-attribute decoding of the selected root's local reference closure only.
Callers pass the root, a length unit, a plane-angle unit and a model tolerance
explicitly; [`discover_solids`](#root-and-unit-discovery) reads them from the file.
Rust is the only interface so far; C/C++ entry points are pending.

## Supported input

| Contract | Supported |
| --- | --- |
| Root | `MANIFOLD_SOLID_BREP` with one `CLOSED_SHELL` of `ADVANCED_FACE`/`FACE_SURFACE` |
| Surfaces | `PLANE`, `CYLINDRICAL_SURFACE`, `CONICAL_SURFACE`, `SPHERICAL_SURFACE`, ring `TOROIDAL_SURFACE`, `B_SPLINE_SURFACE_WITH_KNOTS`, `QUASI_UNIFORM_SURFACE`, rational forms |
| Edge curves | `LINE`, `CIRCLE`, `ELLIPSE`, `B_SPLINE_CURVE_WITH_KNOTS`, `QUASI_UNIFORM_CURVE`, rational forms; any of these as the 3D curve of `SURFACE_CURVE`, `SEAM_CURVE` or `INTERSECTION_CURVE` |
| Encodings | Simple and complex instances; the curve and surface supertype chains mirror the physical component names of complex B-spline records |
| Loops | `EDGE_LOOP` of `ORIENTED_EDGE`; `FACE_OUTER_BOUND` optional (see below) |
| Units | Explicit `LengthUnit` and `AngleUnit`; the angle unit applies only to `CONICAL_SURFACE.semi_angle` |

Vertex identity is `VERTEX_POINT` identity and edge identity is `EDGE_CURVE`
identity; coincident distinct entities are never welded. Each vertex parameter is
recovered on its edge curve (projection, conic angle or bounded NURBS Newton
iteration) and must lie within the model tolerance. `same_sense` must agree with the
parameter direction. Closed edges on conics span one period; closed B-spline edges
must start and end at the curve's domain ends.

`CONICAL_SURFACE` is moved to its apex, which the kernel cone requires; a semi-angle
outside (0°, 90°) in the supplied angle unit is rejected. Horn and spindle tori are
unsupported. A B-spline axis flagged closed in STEP receives a **periodic chart** when
its two boundary curves agree within the model tolerance; the flag alone is advisory.
Periodic NURBS axes wrap evaluation (see [NURBS](NURBS.md)) so seams, chart shifts and
annular faces behave as on analytic surfaces.

## Computed pcurves

Supplied `PCURVE` geometry is never read: `SURFACE_CURVE.associated_geometry` is a
decoder link slot, retained as entity IDs without decoding its targets. Every coedge
receives a pcurve computed from its 3D curve whose parameter is the edge parameter,
so the trimming and tessellation affine correspondence is the identity.

* **Exact images.** Every curve on a plane (lines, conics and NURBS are projected
  affinely), generators of cylinders and cones, and coaxial circles of revolved
  surfaces map to UV lines or conics. Each exact image is verified at five
  parameters against half the model tolerance and otherwise falls back.
* **Fitted images.** All other pairs (for example ellipses on cylinders, meridians,
  intersection curves and every curve on a B-spline surface) receive a piecewise
  cubic Hermite image built from closed-form elementary inversion or Gauss-Newton
  NURBS inversion. Curve knots and quarter periods seed spans; each span is
  subdivided until its lift agrees with the exact inverse to a quarter of the model
  tolerance at three probes. Control points are clamped to closed surface domains,
  so the image stays inside them. A 3D curve farther than the model tolerance from
  its surface, a crossing of a singular point (pole, apex) and an unresolved span
  are typed errors.

## Face charts

Each loop is classified by its winding in the periodic surface axes.

* **Outer bounds.** An explicit `FACE_OUTER_BOUND` is used when present, and a single
  bound is the outer bound. A face with several bounds and no `FACE_OUTER_BOUND` uses
  its unique counterclockwise loop in the surface chart; otherwise it fails with
  "ambiguous outer bound". Holes on periodic surfaces are placed in the outer loop's
  chart by whole periods ([trimming](TRIMMING.md)).
* **Annular faces.** A face whose two loops wind once in opposite directions around
  one periodic axis (a cylinder, cone, sphere zone or torus band bounded by two
  circles and no seam edge) is cut by an **inserted seam**. This isoparametric edge
  joins a vertex of each loop at the same periodic coordinate, on the side where the
  face lies. It is a line on cylinders and cones and a circle on spheres and tori.
  The two loops and both seam uses become one outer loop; geometry is unchanged.
  Faces without an aligned vertex pair whose seam avoids the holes are unsupported.
* Faces that enclose a pole or apex (one wrapping loop), wrap more than once, or use
  `VERTEX_LOOP` bounds are unsupported.

`ImportedSolid::adaptations()` counts inferred outer bounds and inserted seams. Both
are chart representation choices, not repairs: no coordinate, curve or surface is
modified and no topology is merged.

## Root and unit discovery

`discover_solids(document, options)` finds every `MANIFOLD_SOLID_BREP`,
`BREP_WITH_VOIDS` and `FACETED_BREP` record, the shape representations whose `items`
include it, and the units of their context. Candidates are selected by physical record
name; each containing representation is decoded with the bounded
[context profile](../corpus/geometry/context.exp), with `items` as a link slot. The
context must be a 3D `GEOMETRIC_REPRESENTATION_CONTEXT` with
`GLOBAL_UNIT_ASSIGNED_CONTEXT`. SI prefixes and `CONVERSION_BASED_UNIT` chains (for
example inch or degree) are resolved with dimensional-exponent checks. A length
uncertainty named `DISTANCE_ACCURACY_VALUE` is reported in metres, or else the
smallest length uncertainty.

A root outside every supported shape representation, a context without a length or
plane-angle unit, or representations with different units produce a typed unit
error for that root instead of a default. Discovery never selects a tolerance: using
the uncertainty as the model tolerance is the caller's decision.

## Limits and evidence

`BREP_WITH_VOIDS` cavity shells, open shells, swept surfaces
(`SURFACE_OF_REVOLUTION`, `SURFACE_OF_LINEAR_EXTRUSION`), `TRIMMED_CURVE`, offset
curves and surfaces, degenerate tori, pole and apex charts, edge splitting for
misaligned annuli and healing are unsupported. Profile rejection is not an AP
validity verdict. `ImportOptions::strict` rejects `$` in derived `ORIENTED_EDGE`
endpoint slots; `max_work` separately bounds decoding, adapter and pcurve work.

Authored fixtures are generated by
[`brep_fixtures.py`](../corpus/geometry/brep_fixtures.py) and each has a closed-form
volume: a seamless and an explicitly seamed cylinder, a cone frustum in a degree
context, a seamed ring torus, a spherical zone, a washer without `FACE_OUTER_BOUND`
(including an inward bore), and a cube with a rational B-spline top face and B-spline
edges. Negative fixtures cover an off-surface edge, a misaligned annulus, a
vertex-loop apex and an ambiguous outer bound. `tests/brep.rs` checks volumes against
the chord-dependent bound, adaptations, strict policy, record-order invariance,
budgets, 450 byte mutations, agreement with the planar profile and unit discovery.

`python3 scripts/check_geometry.py --require-external` also checks three unmodified
exporter files, hash-pinned in
[external-brep.json](../corpus/geometry/external-brep.json): an ST-DEVELOPER block
with a seamless hole and implicit outer bounds, a CATIA V5 rod with degree-5 B-spline
geometry, and an I-DEAS part with closed rational B-spline cylinders. Corpus survey
outcomes are in [corpus testing](corpus-testing.md#solid-import-stage).
