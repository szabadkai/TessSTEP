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
edges meeting at a vertex to share identical coordinates. The curve-to-vertex displacement
must also satisfy the chord tolerance or sampling fails. At internal knots, adjacent
spans retain the same stored position; a permitted small discrepancy is included in
chord checks. Raw model geometry is never changed. A full closed circle retains both
parameter endpoints with the same canonical vertex position.

`SharedEdges::face_boundary` reconstructs a face's UV chart, then maps each canonical
sample through each use's pcurve, applying its recorded chart shift and checking the
surface lift again. Its positions are references into the shared edge cache; two seam
uses have distinct UV coordinates and identical position references. Output is ordered
outer/hole boundary uses for later constrained triangulation. Planar triangulation rechecks
UV polygon validity at the actual shared-edge boundary resolution.

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

The method reconstructs the supplied-pcurve chart, maps cached samples, and calls
`tessstep_trim::validate_polygons` on the actual mesh-resolution boundaries. Coedge
joins keep the next use's start, as in trimming. Duplicate closing endpoints are omitted;
all other samples and every boundary segment are retained. A coarse approximation that
moves a hole outside the outer polygon fails explicitly, even when the separately
reconstructed smooth trim passed. Callers may resample edges more finely and retry.

Deterministic visible bridges join holes to the current polygon; bounded ear clipping
then produces triangles without Steiner vertices. Bridge copies use the same vertex
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
searches, ear tests, ring copying and mesh checks charge logical work. Worst-case work
is cubic; budgets are not exact wall-time or RSS caps. No partial mesh is returned.

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

Each face uses a diagonal UV metric: the RMS parametric speeds |S_u| and |S_v| over
(at most 64) boundary samples. Scaling UV by it approximates arc length to first
order, so a short, wide cylinder or a slow NURBS direction does not produce needles.
The initial constrained triangulation clips ears best-first by their minimum angle in
the metric chart (validity predicates use the unscaled chart), turning dense parallel
boundaries into strips instead of fans. Queue-based Lawson flips in the metric chart
then revisit only the edges around each flip while keeping constraints fixed; the
flip count is bounded.

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

Failing triangles request their longest metric UV edge midpoint. Each internal midpoint is
allocated once and inserted into all incident triangles. No hanging nodes are permitted.
When the edge is a boundary constraint, the face requests a canonical edge parameter
midpoint instead. The driver combines requests from all selected faces, retains all
previous sample parameters, reruns shared-edge error checks, and remeshes the faces.
This also refines straight 3D boundaries where surface normals vary. Faces never insert
independent boundary positions. Seams retain distinct UVs while sharing position identity.

Fixed probes and f64 predicates are not certified continuous error bounds or exact
Delaunay predicates. Edge flips have a strict angular margin and a flip ceiling of
64 per triangle; they improve shape without claiming a minimum-angle guarantee.
Refinement is isotropic in the metric chart, so doubly curved faces at very small
chords can exceed the default vertex and work budgets. Singular normals,
collapsed curves, missing/non-affine pcurve correspondence and unresolved tolerances
fail explicitly, except at poles and apices closed by collapsed edges (below).
Automatic singular-edge repair and explicit crease-aligned meshing within a
nonsmooth NURBS face remain unsupported; smooth patches on either side can be
separate faces. Resource-limited failure is not a partial mesh.

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
* Edge lengths for split selection are 3D chords when an edge ends at a pole copy, and
  such an edge is split on the ray to its other end rather than at its UV midpoint.
  Lawson flips skip quadrilaterals that contain a pole copy. Both avoid chasing an
  arbitrary chart point toward the pole with needle triangles.
* Singular corners have no surface normal: probes there check only chord distance, and
  each triangle's corner normal is the limit from inside its polar wedge. Output UVs of
  pole copies are their triangle's polar coordinate.

The hemisphere and apex-cone kernel tests
(`pole_and_apex_charts_close_with_collapsed_edges`) check watertightness, one welded
pole vertex, the chord-dependent volume bound, dense independent polar-wedge probes
(within 5% of the tolerances) and determinism. Charts that reach a singularity on a
NURBS surface side are not yet recognized; see [curved import](CURVED_IMPORT.md).

## Owned shell and solid meshes (Milestone 17)

`tessellate_shell` uses a model-local shell handle and enforces its declared closure
and connectedness. `tessellate_solid` additionally requires a single closed component
with positive algebraic volume; inward orientation is diagnosed instead of repaired.
The existing topology contract supplies one boundary shell, without cavity nesting.

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
