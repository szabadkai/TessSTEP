# Tessellation

Milestones 13–17 provide shared-edge sampling, planar and regular curved-face
tessellation, and owned manifold shell/solid meshes in `tessstep-tessellate` /
`tessstep::tessellate`. STEP-to-mesh adaptation is still pending. Error bounds are
sampled estimates and solid closure is topological, not a proof of nonintersection.

`sample_edges` consumes only an immutable `NormalizedBrep` reference, explicit chord /
angular tolerances and resource limits. It samples each topological edge once in its
canonical increasing parameter direction. Every coedge gets a zero-copy forward or
reversed view of the same stored samples. The result borrows its source B-rep, so its
view lifetime and identity are explicit. Coincident but distinct edges are not merged.

Analytic periodic curves are seeded in at most quarter-period intervals; NURBS curves
are split at every distinct interior knot. Iterative subdivision evaluates quarter,
midpoint and three-quarter probes. Acceptance checks their distance and both endpoint
distances from the proposed chord, plus the angle between each sampled tangent and the
chord. The tessellation angular tolerance is used for edge tangents here; no face-normal
error claim is made. Knot endpoints use one-sided derivatives so true corners can be
represented directly. Positional jumps beyond the smaller of model/chord tolerance,
singular tangents and numeric failures produce typed errors.

Canonical first/last positions are the exact stored vertex values, allowing different
edges meeting at a vertex to share identical coordinates. A vertex may sit off its
curve end by up to the model tolerance (the B-rep accepted it that way): the chord
and tangent criteria measure the curve itself, and only the stored end samples move
onto the vertex, so an end segment deviates from the curve by that offset in addition
to the chord. A larger offset fails sampling. Seed parameters within numerical noise
of each other or of the range ends (a range starting at -5e-17 beside the period
break at 0) are dropped, since a span of no length can never satisfy the chord
direction. At internal knots, adjacent spans retain the same stored position; a
permitted small discrepancy is included in chord checks. Raw model geometry is never
changed. A full closed circle retains both parameter endpoints with the same
canonical vertex position.

Face refinement ([below](#curved-faces-and-adaptive-refinement-milestones-1516))
treats boundary samples the same way: a facet is measured through the surface points
of its corners, so a canonical sample up to the model tolerance off the surface does
not tilt the plane its probes are compared with.

`SharedEdges::face_boundary` reconstructs a face's UV chart, then maps each canonical
sample through each use's pcurve, applying its recorded chart shift and checking the
surface lift again. Its positions are references into the shared edge cache; two seam
uses have distinct UV coordinates and identical position references. Output is ordered
outer/hole boundary uses for later constrained triangulation. The mapped loops (each
use's samples but its last) are then checked again with
`TrimmedFace::validate_resolution` at the resolution the triangulator resolves: simple,
correctly nested and wound loops at floating-point resolution (1e-12 of the boundary
extent plus 16 ulps of its coordinate magnitude, at most the UV tolerance). Validity at
the model tolerance is the trim reconstruction's; the UV tolerance is a chord tolerance
in chart units, not a distance (10 µm on a metre plane chart), so it is not used to
reject a 5 µm edge or samples refined closer than it. A failure is
`BoundaryError::Resolution` and names the coedges involved (for example a coarse chord
of one edge crossing a nearby edge).

## Bounds and limitations

Logical output/evaluation limits and a depth ceiling of 48 fail explicitly without
publishing a partial cache. Periodic seeding and knot boundaries avoid common aliasing,
but fixed interior probes do **not** certify the global error of arbitrary rational
curves between probes. No exact predicates or certified Hausdorff bound are claimed.
Small tolerances may fail through depth, floating-point spacing or resource limits.
A curve with a stationary point on an open edge fails with `SingularTangent`. A
*collapsed* edge (see [topology](TOPOLOGY.md#collapsed-edges)) is a point: it emits
its seeds (knots plus requested parameters) as parameter-only samples at its vertex,
without tangent or chord checks, so face charts can place UV samples along a pole or
apex line. Automatic singular-edge repair is not implemented.

Tests independently verify circle sagitta/tangent bounds, tighter-tolerance refinement,
line minimal sampling, canonical vertex identity, zero-copy reversed/shared/seam views,
NURBS knot corners and discontinuities, UV mappings, degenerate curves and hard failures.
A deterministic mutation harness, libFuzzer driver and boundary benchmark cover the
combined pipeline. STEP adaptation and external geometry/tessellation corpus stages
remain pending. Exporters belong outside the kernel. Public C/C++ mesh import and retained read-only views are implemented; see
[C_API.md](C_API.md). C/C++ B-rep construction and tessellation entry points remain pending.

## Planar face triangulation (Milestone 14)

`SharedEdges::triangulate_planar(face, PlanarOptions::default())` consumes an analytic
plane face from the cache's normalized B-rep. It supports concave outer loops, multiple
strictly contained disjoint holes, collinear boundary vertices and sampled curved edges.
NURBS surfaces, including geometrically planar patches, and non-plane analytic surfaces
return `UnsupportedSurface`; no sampled planarity inference occurs.

The method reconstructs the supplied-pcurve chart, maps cached samples, and checks the
actual mesh-resolution boundaries (a face boundary resolution failure is
`InvalidPolygon` here, located at its coedges); the triangulator rechecks them with
`tessstep_trim::validate_polygons` at the same floating-point resolution. Coedge
joins keep the next use's start, as in trimming. Duplicate closing endpoints are omitted;
all other samples and every boundary segment are retained. A coarse approximation that
moves a hole outside the outer polygon fails explicitly, even when the separately
reconstructed smooth trim passed. Callers may resample edges more finely and retry.

Deterministic visible bridges join holes to the current polygon; bounded ear clipping
then produces triangles without Steiner vertices. Each hole takes the shortest visible
bridge to the current polygon (ties by the lowest hole, then polygon position).
Candidates are tested for visibility in that order, in batches of the 32 shortest
untested ones, so the search ends at the first visible bridge; it used to test every
candidate shorter than the best so far in index order, each test linear in the
boundary size. Ear tests visit only the polygon positions a uniform grid (about one
position per cell) places within the ear's triangle widened by the predicate
tolerance (each corner by `eps / sin(angle / 2)`), and scan the whole polygon only
for corners sharper than about 2e-6 rad. Both searches return what the full scans
returned, so triangulations are unchanged; on the Onshape export's cylinder with 36
holes (1,802 boundary samples) bridging fell from 24.7M to 1.8M work units and ear
clipping from 3.4M to 0.25M. Bridge copies use the same vertex
indices, creating ordinary paired interior edges. Collinear vertices are never silently
dropped. Postchecks require every directed boundary segment exactly once, every interior
edge twice in opposing directions, the expected triangle count, matching UV area and
positive 3D triangle orientation. Face reversal flips triangle winding and the unit normal.

`PlanarMesh` is an immutable face-local result with `u32` triangle indices, outer/hole
index loops, UV vertices and a face normal. Each vertex borrows its `Sample` from the
shared cache. Adjacent faces retain identical interior sample references and exact
canonical endpoint coordinates. The mesh cannot outlive that cache. This specialized result is not a welded shell/solid asset. The general tessellation
entry points below return owned `tessstep-mesh::Mesh` assets instead. Public C/C++
mesh import/views use explicit C scalar buffers and ownership contracts, with installed
consumer tests; these borrowed Rust records never cross the ABI.

`PlanarOptions` bounds vertices (at most one million), triangles (at most two million)
and work. Trim reconstruction has its own options/budgets; polygon validation and
triangulation each receive `max_work`. Intersection/containment comparisons, bridge
searches, ear tests, grid cells, ring copying and mesh checks charge logical work.
Worst-case work is still cubic (every bridge candidate invisible, every ear widened
over the whole grid); budgets are not exact wall-time or RSS caps. No partial mesh is
returned.

Predicates use checked finite f64 arithmetic and the explicit UV distance tolerance.
Near-degenerate configurations may return `UnresolvedGeometry`. There is no exact
predicate, Delaunay/minimum-angle quality guarantee, certified continuous curve error,
automatic healing, interior refinement or solid watertightness claim. Canonical boundary
positions may differ from the ideal plane within model tolerance, as already checked
by boundary mapping. Tests cover area/containment, all constraint segments, shared sample
identity, face orientation, tilted planes, holes in both orders, varied concave polygons,
resource failures and invalid actual-resolution polygons. The combined mutation driver
and boundary benchmark now exercise triangulation too.

## Curved faces and adaptive refinement (Milestones 15–16)

`tessellate_faces(brep, &[FaceId(...)], tolerance, options)` accepts an immutable
normalized B-rep and explicit SI chord/angular tolerances. Analytic planes, cylinders,
regular cone/sphere patches, ring tori and positive-weight tensor-product NURBS faces
share the supplied-pcurve chart and constrained UV triangulator. Face orientation is
applied to both triangle winding and per-corner normals. Face IDs follow the input model.

The initial constrained triangulation ranks ears in a diagonal UV metric, the RMS
parametric speeds |S_u| and |S_v| over (at most 64) boundary samples, which
approximates arc length to first order: ears are clipped best-first by their minimum
angle in that chart (validity predicates use the unscaled chart), turning dense
parallel boundaries into strips instead of fans. Refinement and flips then use the
per-vertex curvature metric described [below](#curvature-anisotropic-refinement-milestone-222).
Queue-based Lawson flips revisit only the edges around each flip while keeping
constraints fixed; the flip count is bounded.

Refinement measures the distance from evaluated surface probes to the facet plane
(the geometric chord error) and the angle between the facet normal and evaluated
surface normals. A tangential parametric offset, such as the skew of a UV diagonal
across a cone generator, is not surface deviation and is not measured. Vertex, edge
midpoint, centroid and interior quarter-weight probes must pass. Periodic triangle
extents are limited to a quarter period to avoid simple aliasing. NURBS checks clip
each triangle against every intersected knot cell (repeated per period on periodic
axes) and probe the clipped corners, edge midpoints and center, with one-sided
evaluations at cell boundaries. Narrow knot spans therefore receive checks even when
ordinary triangle probes would miss them.

Failing triangles request the midpoint of their longest edge in the curvature metric.
Each internal midpoint is allocated once and inserted into all incident triangles. No
hanging nodes are permitted. When the edge is a boundary constraint, the face requests
a canonical edge parameter midpoint instead. The driver combines requests from all
selected faces, retains all previous sample parameters, reruns shared-edge error
checks, and remeshes the faces whose edges changed; a face completed in the previous
pass whose edges kept their samples is reused, since meshing is deterministic. This
also refines straight 3D boundaries where surface normals vary. Faces never insert
independent boundary positions. Seams retain distinct UVs while sharing position
identity.

A face does not return at its first boundary request. Interior refinement continues
until no interior split remains or the set of requested boundary segments stops
changing between rounds, so one boundary pass resolves every facet that the coarse
boundary leaves failing (requests used to trickle in a few per pass until the pass
limit). Facets beside an already requested segment would otherwise keep splitting
into slivers along it, hence the second condition. Within a face, a facet that passed
is not probed again in later rounds: its corners determine every probe.

Nothing is evaluated or tested twice when the answer is already known, and each
reuse returns exactly what repeating the work would, so meshes are unchanged:

* A probe at a regular corner is that vertex's own evaluation, a probe at an edge
  midpoint is shared by the two facets on the edge and by the vertex a split of the
  edge places there, and a left-sided knot-cell probe off every knot line is the
  ordinary evaluation.
* Each face keeps, across boundary passes, the evaluations of its vertices and the
  facets between regular corners that passed, keyed by exact chart coordinates;
  meshing a face again after a pass mostly reproduces its interior.
* Triangles keep their slots across rounds and the edge incidence is updated by
  splits and flips instead of rebuilt. After the first round the flip queue holds
  only the edges of triangles the last split created and the edges a flip reaches
  before their turn, at their place in key order: every other edge's quadrilateral
  is unchanged since it was last found settled. A call that stopped at the flip
  ceiling is followed by a full one.

`TessellationOptions::threads` (default 1) meshes the faces of each boundary pass on
that many threads. Each face runs against the budgets left at the start of the pass;
their charges are then applied in face order, and if any face ran out or its charges
exceed what the faces before it left, the whole call is repeated on one thread. The
mesh, the budgets consumed and every diagnostic therefore do not depend on the
thread count, and a budget failure costs a second, single-threaded run. The C entry
points use one thread.

Facets are measured through the surface points (lifts) of their corners, not the
canonical boundary positions, which may sit off the surface by up to the model
tolerance; the mesh then deviates from the surface by that offset in addition to the
sampled error, as edge sampling does at vertices. A facet no wider than the model
tolerance (twice its area over its longest edge) has no plane orientation the model
resolves: it must meet the chord, but its normal is not compared with surface normals,
which may fold within it near a degenerate pole. The metric likewise never asks for a
step below the model tolerance. A corner whose surface normal faces away from its
facet's plane (possible only for such facets or where canonical corners sit off the
surface) carries the plane's normal, so output corner normals always face the facet's
side.

Fixed probes and f64 predicates are not certified continuous error bounds or exact
Delaunay predicates. Edge flips have a strict angular margin and a flip ceiling of
64 per triangle; they improve shape without claiming a minimum-angle guarantee.
Singular normals,
collapsed curves, missing/non-affine pcurve correspondence and unresolved tolerances
fail explicitly, except at poles and apices closed by collapsed edges (below).
Automatic singular-edge repair and explicit crease-aligned meshing within a
nonsmooth NURBS face remain unsupported; smooth patches on either side can be
separate faces. Resource-limited failure is not a partial mesh.

### Curvature-anisotropic refinement (Milestone 22.2)

Every regular vertex carries a symmetric UV metric tensor built from the surface's
second derivatives in an orthonormal tangent frame. Refinement only compares lengths
in it (the longest edge of a failing facet, the Delaunay diagonal of a quadrilateral),
so its shape matters, not its scale:

* **Normal part.** The absolute second fundamental form |II| (principal curvatures
  replaced by their magnitudes), scaled so its largest eigenvalue is the inverse square
  of the step the angle (`angle / k`) or the chord (`sqrt(6 chord / k)`) allows. A
  linear facet approximates a quadratic surface best, in distance and in normal, when
  it is equilateral in |II|. The ratio of normal-turn rates (the third fundamental
  form) is the wrong shape: a facet that long across a narrow torus band sags along its
  long edge by more than its width, and its plane tilts out of the angle.
* **Tangential part.** Parameter lines that curve within the tangent plane (the u
  circles near the top of a torus, polar charts near an apex) bend a long edge
  sideways; the tangential second derivatives add a term that keeps that in-plane
  sagitta of a unit edge below a quarter of the smallest allowed step.
* **Floor and ceiling.** An isotropic floor (the inverse square of the face's 3D
  boundary extent) keeps the tensor positive definite and measures a plane by arc
  length; a ceiling (the inverse square of the model tolerance) caps every eigenvalue.

An edge's length is measured in the mean of its end tensors; an edge ending at a pole
copy is measured as its 3D chord times the finer end's density. Lawson flips decide
each quadrilateral in the Cholesky chart of its four vertices' mean tensor. Splitting
the longest edge in this metric splits a tight torus band across the band rather than
along it: the Onshape export's band (R = 2.035 mm, r = 0.3 mm, 0.1 rad) used to walk a
sliver along the band one vertex per round to the round limit, and now meshes with
1,591 vertices in 0.02 s.

### Poles and apices

A face whose chart reaches a surface singularity (a sphere pole, a cone apex) closes
its UV polygon with a collapsed edge along the singular line. Each sample of that edge
is a distinct UV *pole copy* of one mesh vertex (`Source::Vertex`), so all of them weld
to the singular vertex. A pole copy's coordinate along the pole line is arbitrary, and
the tessellator treats it that way:

* A triangle with one pole copy is probed as a *polar wedge*: its coordinate along the
  pole line follows the opposite edge (barycentric weights renormalized over the other
  two corners). The probed region is the fan the 3D facet spans, and adjacent fans
  share their rays from the pole, whatever pole copy each triangle uses.
* A triangle with two copies of the same pole is a UV sliver with no 3D extent. It is
  always accepted and dropped from the mesh; the fans around it cover that surface.
* Edge lengths for split selection are 3D chords (times the finer end's metric
  density) when an edge ends at a pole copy, and such an edge is split on the ray to its
  other end rather than at its UV midpoint, at the point halfway to that end in 3D
  (a short bisection on the ray parameter keeps the split between 3/8 and 5/8 of the
  chord; a B-spline row can collapse so slowly that the chart midpoint sits next to the
  pole). Edges from different copies of one pole to the same vertex are one 3D edge
  and share that midpoint. Lawson flips skip quadrilaterals that contain a pole copy.
  All of this avoids chasing an arbitrary chart point toward the pole with needles.
* Singular corners have no surface normal: probes there check only chord distance, and
  each triangle's corner normal is the limit from inside its polar wedge. A B-spline
  pole whose control row collapses only within tolerance still evaluates a (noise)
  normal at the pole itself; probes exactly at a singular corner ignore it. Output UVs
  of pole copies are their triangle's polar coordinate.

The 3D fan around a pole is consistent only when the rays of its wedges advance
monotonically around it. Ear clipping in the raw chart does not know that: next to a
seam running out of the pole it fans one pole copy to consecutive seam samples
(wedges of no angle along the seam), or it fans a pole copy to distant samples whose
rays run backwards, and such folds survive every interior split. Each run of two or
more pole copies with regular neighbours is therefore capped explicitly: a ring of
interior vertices parallel to the pole line, through the nearer of the two boundary
samples beside the run and at the run's interior pole-copy coordinates, replaces the
run in the polygon; between pole line and ring each pole-copy interval becomes a
collapsed sliver and a wedge between consecutive ring rays, and the rest of the face,
bounded by the ring, is ear clipped as before. Ray splits and ring-edge splits keep
the fan monotone. A ring that would leave the face or cross a loop is not built, and
that run keeps the plain triangulation; a folded wedge found there requests a pole
sample between its rays (the collapsed edge is parameter-only) and restarts the pass,
and a split whose pole ray coincides with a boundary segment requests that segment
instead. The 36 B-spline faces of the Onshape export with two `VERTEX_LOOP` poles
each used to exhaust their boundary passes and now mesh.

The hemisphere and apex-cone kernel tests
(`pole_and_apex_charts_close_with_collapsed_edges`) check watertightness, one welded
pole vertex, the chord-dependent volume bound, dense independent polar-wedge probes
(within 5% of the tolerances) and determinism; a sphere from pole to pole with one
seam and a hole reaching within 0.37 rad of both poles
(`pole_caps_keep_fans_monotone_beside_seams_and_holes`) checks dense probes and that
every facet at a pole faces outward. Collapsed B-spline sides use the same mechanism;
see [curved import](CURVED_IMPORT.md).

## Owned shell and solid meshes (Milestone 17)

`tessellate_shell` uses a model-local shell handle and enforces its declared closure
and connectedness; STEP surface models mesh each shell this way, never as a solid
([curved import](CURVED_IMPORT.md#surface-models)). `tessellate_solid` additionally requires a single closed component
with positive algebraic volume; inward orientation is diagnosed instead of repaired.
A solid with cavity shells (`Solid::voids`) is meshed in one pass under one budget and
must be watertight with one component per shell, a positive outer volume and a
negative volume for every cavity shell (which faces into its cavity), computed per
shell from triangle face IDs; otherwise `CavityOrientation` or a mesh error is
returned. Cavity containment and disjoint interiors are not checked.

Assembly welds positions by topological vertex identity or canonical edge/sample index.
Distinct coincident edges/vertices are never merged by coordinate proximity. Face-interior
vertices remain local. UVs and unit normals are stored per triangle corner so shared
positions can retain periodic seams and sharp face normals. Final validation checks
finite/nondegenerate data, duplicate triangles, oriented edge incidence, vertex links,
connected components and boundary counts. See [MESH.md](MESH.md).

A watertight result means every mesh edge has two opposing uses and vertex links are
manifold cycles. Positive signed volume is an orientation check, not evidence that
triangles do not intersect or that the mesh bounds the intended material. No exact
intersection, nested-shell, global Hausdorff or full CAD validity claim is made.

`TessellationOptions` bounds refinement rounds (at most 48), boundary restart passes
(at most 24), adaptive/edge evaluation counts, refinement/assembly work and output size.
Shared-edge sample limits apply per pass; UV reconstruction and initial triangulation
retain their separate per-face/per-pass budgets. Mesh limits also conservatively cap
the sum of face-local vertices before welding. Thus `max_evaluations` is not a count of
trim reconstruction's independent evaluations, nor is any logical budget an RSS cap.
The algorithm is deterministic for the same ordered faces, inputs and options.

### Tessellation budgets (Milestone 22.2)

Every `ResourceLimit` names the budget it exhausted and that budget's configured value
in `TessellationError::budget` (a `Budget { name, limit }`, `name` being the option's
path) and in its `Display`, for example `tessellation Some(FaceId(90)):
Polygon(ResourceLimit) (budget planar.max_work 10000000 exhausted)`; nested kinds keep
their own variant (`Sampling`, `Boundary`, `Polygon`, `Mesh`). Refinement rounds and
boundary passes that run out (`UnresolvedTolerance`) name `max_rounds` or
`max_boundary_passes` the same way. The budgets and their defaults:

| Budget | Default | Bounds |
| --- | ---: | --- |
| `max_work` | 50,000,000 | refinement, probes, flips, caps and assembly, shared by all faces and passes |
| `max_evaluations` | 10,000,000 | surface evaluations and edge sampling, shared by all faces and passes |
| `max_rounds` | 32 (at most 48) | refinement rounds per face and pass |
| `max_boundary_passes` | 16 (at most 24) | boundary resampling passes |
| `sampling.max_samples` / `sampling.max_evaluations` | 1,000,000 / 10,000,000 | edge samples and evaluations per pass |
| `planar.max_vertices` / `planar.max_triangles` | 100,000 / 200,000 | vertices and triangles per face (boundary mapping allows twice the vertices) |
| `planar.max_work` | 10,000,000 | polygon validation, and separately bridges and ear clipping, per face and pass |
| `planar.trim.max_work` / `planar.trim.max_samples` | 5,000,000 / 100,000 | trim reconstruction per face and pass |
| `mesh.max_vertices` / `mesh.max_triangles` / `mesh.max_work` | 1,000,000 / 2,000,000 / 50,000,000 | the assembled mesh |

Trim reconstruction reports one `Limit` for its two budgets; on that failure path a
rerun with the sample ceiling raised tells them apart. The defaults are unchanged by
Milestone 22.2: the algorithmic costs were reduced instead (the curvature metric, the
caps, deferred boundary requests, reused faces and facets, bisected knot lines and
skipped single-cell knot probes, the bridge order, the ear grid, and the reuse of
evaluations, facets and edge incidence described above). The whole Onshape export uses
2.5–2.6M of the 10M evaluations and 15–17.5M of the 50M work units. In the corpus,
`bug30790.step` meshes all 26 solids within the defaults: each of its 24 B-spline
solids has 217,000–251,000 triangles and needs 5–7M evaluations and 30–40M work
units, where it needed 10–15M evaluations before the reuse. The roots that still run
out are `bug24593_281946.stp` #30 and `bug26889_export.step` #34 (`max_work`; the
latter then needs more than the 100,000 vertices of one face) and
`bug24024_slow_import.stp` #76237 (`planar.max_work`).
