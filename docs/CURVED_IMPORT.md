# Curved B-rep import

`tessstep_import::import_brep_solid` converts a selected `MANIFOLD_SOLID_BREP` whose
faces lie on elementary, B-spline or swept surfaces into a validated B-rep and owned
solid mesh. It is a superset of the edge-based [planar profile](STEP_IMPORT.md) and uses
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
| Root | `MANIFOLD_SOLID_BREP` with one `CLOSED_SHELL` of `ADVANCED_FACE`/`FACE_SURFACE`, or `BREP_WITH_VOIDS` adding cavity shells; `ORIENTED_CLOSED_SHELL` orientation is applied to its faces |
| Surfaces | `PLANE`, `CYLINDRICAL_SURFACE`, `CONICAL_SURFACE`, `SPHERICAL_SURFACE`, ring `TOROIDAL_SURFACE`, `B_SPLINE_SURFACE_WITH_KNOTS`, `QUASI_UNIFORM_SURFACE`, rational forms; `SURFACE_OF_LINEAR_EXTRUSION` and `SURFACE_OF_REVOLUTION` of any supported curve (see [swept surfaces](#swept-surfaces)) |
| Edge curves | `LINE`, `CIRCLE`, `ELLIPSE`, `B_SPLINE_CURVE_WITH_KNOTS`, `QUASI_UNIFORM_CURVE`, rational forms; any of these as the 3D curve of `SURFACE_CURVE`, `SEAM_CURVE` or `INTERSECTION_CURVE`, or as the basis of a `TRIMMED_CURVE` |
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
outside (0°, 90°) in the supplied angle unit is rejected. `DEGENERATE_TOROIDAL_SURFACE`
(major radius below the minor radius) becomes a spindle torus whose `select_outer`
part is the chart's closed v domain, with the two axis points as singular points
handled like sphere poles; a `TOROIDAL_SURFACE` with major radius at or below the
minor radius (a horn torus, or a spindle without the degenerate entity) is rejected.
A B-spline axis flagged closed in STEP receives a **periodic chart** when
its two boundary curves agree within the model tolerance; the flag alone is advisory.
Periodic NURBS axes wrap evaluation (see [NURBS](NURBS.md)) so seams, chart shifts and
annular faces behave as on analytic surfaces.

## Swept surfaces

`SURFACE_OF_LINEAR_EXTRUSION` and `SURFACE_OF_REVOLUTION` are converted to kernel
surfaces without approximation. Cases that are exactly elementary surfaces use that
chart: an extruded line is a plane, a circle extruded along its normal a cylinder, a
revolved line a cylinder (parallel), plane (perpendicular) or cone (meeting the axis;
the nappe containing the face is selected and a face crossing the apex is
unsupported), and a circle revolved in a plane through the axis a sphere (centre on
the axis) or ring torus. Everything else becomes NURBS: an extrusion is the swept
curve's NURBS (conics as nine-control rational quadratics) swept linearly over the
face's axial extent plus a margin; a revolution is the Piegl-Tiller rational revolve
with a full rational quadratic circle, and a revolved skew line is a segment covering
the face's axial range. Closed swept curves and the revolution angle receive periodic
charts when their closure checks pass, and revolved profile ends on the axis are
collapsed B-spline sides ([pole charts](#face-charts)).

The STEP surface normal (`C'(u) × V` for an extrusion, `(a × (C − A)) × C'(v)` for a
revolution at angle zero, reversed for a trimmed curve whose sense disagrees with its
basis) is compared with the kernel normal at a sampled curve point, and the face
orientation is flipped when they differ, so each face keeps its outward side.
`TRIMMED_CURVE`s contribute their basis curve and sense only: edge vertices bound
every use. A `LINE` whose vector magnitude is zero is read as unit speed, because
only its direction is used. Evidence:
`brep_swept_surfaces_are_exact_elementary_or_nurbs_charts`.

## Computed pcurves

Supplied `PCURVE` geometry is never read: `SURFACE_CURVE.associated_geometry` is a
decoder link slot, retained as entity IDs without decoding its targets. Every coedge
receives a pcurve computed from its 3D curve whose parameter is the edge parameter,
so the trimming and tessellation affine correspondence is the identity.

* **Exact images.** Every curve on a plane (lines, conics and NURBS are projected
  affinely), generators of cylinders and cones, coaxial circles of revolved surfaces
  and meridian arcs of spheres (great circles through the poles, including arcs that
  end at a pole) map to UV lines or conics. Each exact image is verified at five
  parameters against half the model tolerance and otherwise falls back.
* **Fitted images.** All other pairs (for example ellipses on cylinders,
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
  face lies. It is a line on cylinders and cones, a circle on spheres and tori, and
  the isoparametric curve of a NURBS chart (repeated over neighbouring periods when
  the other axis is periodic).
  The two loops and both seam uses become one outer loop; geometry is unchanged.
* **Seam placement.** Candidate isoparametric lines pass through each loop-a vertex,
  through the widest gap between the holes' ranges around the period, and a quarter,
  half and three quarters of the period from each vertex. On each line every
  significant crossing of a loop or hole edge is collected (a crossing counts only
  when the edge moves at least a thousandth of the model tolerance away from the line
  on both sides, so the sub-nanometre wobble of a pcurve fit beside a vertex on the
  line is ignored) together with the vertices on the line, and ordered along it. A
  loop-a hit whose next hit, on the side where the face lies, is a loop-b hit bounds
  a seam that no edge crosses or touches; a hit that is not a vertex splits its edge
  there, and the next planning pass finds the aligned pair. Wavy annuli whose
  vertex-aligned lines cross other edges, holes all around a cylinder, and edges that
  meet their vertex tangentially to the line all resolve this way.
* **Sliver faces.** A face bounded by one loop of two open edges with the same
  vertices whose curves coincide within the model tolerance (typically a B-spline
  written beside the arc it approximates, 0.1 µm apart under a 20 µm uncertainty) is
  removed before planning; every use and entity link of the second edge is retargeted
  onto the first (an analytic edge survives a B-spline), so neighbours share one edge
  and the shell stays closed. The removal is reported as `collapsed_faces`.
* **Edge splits.** When no aligned vertex pair exists (for example two closed circles
  whose vertices sit at different angles), the loop edge that the isoparametric line
  through a vertex of the other loop crosses is split there. Both halves keep the
  original curve, every face that uses the edge receives both halves in traversal
  order, and all faces are planned again. A seam that would cross a hole is not used.
* **Sphere charts.** A spherical face that fails in its STEP frame because its
  boundary passes through or its loop encloses a pole is planned again in rotated
  frames: first axes perpendicular to the mean boundary direction, then lattice
  directions, each at least 1e-3 rad from every sampled boundary direction. The sphere,
  its orientation and every 3D curve are unchanged. Rotated frames are tried before
  pole charts, so a sphere uses a pole chart only when no frame avoids one (for
  example a hemisphere bounded by a great circle). A face with `VERTEX_LOOP` bounds
  first tries the frames whose pole is that vertex.
* **Pole and apex charts.** Sphere poles and the cone apex are singular lines of their
  charts. The chart is closed there by a **collapsed edge**
  ([topology](TOPOLOGY.md#collapsed-edges)): a closed edge whose curve is the singular
  point, with a pcurve along the singular line in the direction that keeps the face on
  its left. Three configurations are recognized:
  * *Joins.* Where consecutive uses of a loop meet at the singular point (a generator
    seam up to the apex and back, two meridians meeting at a pole, a pole-to-pole
    seam), a collapsed edge joins their chart ends.
  * *Enclosed poles.* A single loop that winds once around the periodic axis encloses
    the pole or apex on its left. An isoparametric seam from one of its vertices to
    that point (a cone generator or sphere meridian avoiding the face's holes) and a
    collapsed edge close the chart. A `VERTEX_LOOP` at that point names the pole
    vertex; otherwise a vertex is inserted at the pole.
  * *Full spheres.* A face bounded only by `VERTEX_LOOP`s (and holes) spans the band
    between both poles: a pole-to-pole seam and a collapsed edge at each pole close
    it.

  A `VERTEX_LOOP` away from every pole or apex, a winding loop with no singular point
  on its inner side, and a loop that doubles back along an edge away from a pole are
  unsupported. Faces that wrap more than once and edges that cross a singular point
  in their interior remain unsupported; singular sides of B-spline surfaces are not
  yet recognized.

`ImportedSolid::adaptations()` counts inferred outer bounds, inserted seams (annulus
and pole seams), split edges, re-charted spheres and collapsed edges. All are
representation choices, not repairs: no coordinate, curve or surface is modified and
no topology is merged; the only inserted vertices are split points and poles that no
`VERTEX_LOOP` names.

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

Open shells, offset curves and surfaces, degenerate
tori, reversed `TRIMMED_CURVE`s nested inside other curves, and healing are
unsupported. Profile rejection is not an AP
validity verdict. `ImportOptions::strict` rejects `$` in derived `ORIENTED_EDGE`
endpoint slots; `max_work` separately bounds decoding, adapter and pcurve work.

Authored fixtures are generated by
[`brep_fixtures.py`](../corpus/geometry/brep_fixtures.py) and each has a closed-form
volume: a seamless and an explicitly seamed cylinder, a cone frustum in a degree
context, a seamed ring torus, a spherical zone, a washer without `FACE_OUTER_BOUND`
(including an inward bore), a cube with a rational B-spline top face and B-spline
edges, a cylinder whose loop vertices are a quarter turn apart (one split), a
spherical cap enclosing its STEP pole (one re-chart), a hemisphere (pole seam), a cone
tip closed by a `VERTEX_LOOP` at its apex, a cone tip whose loop runs up a generator
seam to the apex and back (one join), a sphere bounded by one `VERTEX_LOOP` (re-chart,
pole-to-pole seam, two collapsed edges) and a sphere with a pole-to-pole meridian seam
edge (two joins), a tetrahedron whose base is a bilinear B-spline patch with a collapsed
side, a cylinder whose side extrudes its base circle, an elliptic prism extruding a
reversed trimmed ellipse (periodic NURBS, isocurve seam, flipped orientation), a cone
tip revolving a line, a sphere revolving a rational B-spline semicircle (two collapsed
sides) and a torus revolving a circle. Negative fixtures cover an off-surface edge, a
`VERTEX_LOOP` away from the apex and an ambiguous outer bound. Cavity fixtures cover a cube
with a cubical void and a cylinder with a spherical void bounded by a `VERTEX_LOOP`
(each void an outward shell reversed by `ORIENTED_CLOSED_SHELL(.F.)`), and a cube whose
void faces out of its cavity (rejected at tessellation). `tests/brep.rs` checks volumes against the
chord-dependent bound, adaptations, one welded vertex per pole, strict policy,
record-order invariance, budgets, 450 byte mutations, agreement with the planar profile
and unit discovery.

`python3 scripts/check_geometry.py --require-external` also checks three unmodified
exporter files, hash-pinned in
[external-brep.json](../corpus/geometry/external-brep.json): an ST-DEVELOPER block
with a seamless hole and implicit outer bounds, a CATIA V5 rod with degree-5 B-spline
geometry, and an I-DEAS part with closed rational B-spline cylinders. Corpus survey
outcomes are in [corpus testing](corpus-testing.md#solid-import-stage).
