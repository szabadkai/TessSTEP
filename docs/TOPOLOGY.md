# Topology and validity

Milestone 11 provides independent constructed B-reps in `tessstep-topology` and
`tessstep::topology`. It does not adapt STEP entities or establish AP conformance.

## Ownership and states

`RawBrep` owns vertices, edges, coedges, wires, faces, shells and solids. Each kind
has a distinct typed, model-local index. Indices may be authored freely in raw
inputs; validation checks them before lookup. They are neither physical entity IDs
nor globally unique handles and must not be mixed between models.

Edges own analytic or NURBS 3D geometry and an increasing finite basis interval.
Vertices give canonical endpoint positions in model metres. Each coedge retains
its orientation and its own optional 2D pcurve. Pcurve interval endpoints correspond
to the canonical edge endpoints, even for reversed uses; intervals may decrease.
The initial pcurve contract is an affine correspondence between the two parameter
intervals. Faces own one outer wire and zero or more hole wires plus a surface and
orientation. Effective shell incidence combines coedge and face orientation.

`RawBrep::validate` consumes the raw container without changing geometry and either
returns a located typed first-defect report or a privately constructed `ValidatedBrep`.
Callers needing the raw object after failure can clone before validation.
`ValidatedBrep::normalize` moves it into immutable `NormalizedBrep`, retaining a
canonical edge-to-coedge incidence table. Normalization never merges vertices,
changes coordinates, reverses wires, or silently heals geometry. No healing API is
implemented; any future healing must report each edit and tolerance separately.

Validation checks finite increasing edge ranges, evaluable endpoints and agreement
with vertices within an explicit model distance tolerance, typed references, exact
vertex-identity wire closure, unique coedge/wire/face ownership, and unused vertices,
edges, coedges and wires. Missing pcurves are retained for the trim stage to diagnose.
Unsewn faces and open shells are permitted. Declared shells must be nonempty and
connected through edges; at most two uses meet each edge and paired effective
orientations oppose. Vertex links must be connected paths or cycles. Declared closed
shells require two uses per edge and cyclic vertex links. Solids reference one uniquely
owned closed shell. Cavity shells and nesting semantics are not implemented.

These are structural validity states: validation does **not** certify self-intersection,
face/surface agreement, continuous curve interiors, shell outwardness, positive volume,
or containment. UV reconstruction checks sampled pcurve agreement separately. Collapsed
curves and surface singularities are diagnosed by downstream operations; structural
acceptance does not establish regular geometry. No mesh or watertightness claim follows.

## Bounds and evidence

Validation uses iterative walks and ordered incidence/link maps, with a hard maximum
of one million records and caller-supplied record/work budgets. Nested memberships
consume work before traversal. Logical budgets do not bound caller-owned raw allocation,
allocator failure or exact RSS. Geometry evaluation inherits the existing spline limits.

Tests cover a closed tetrahedral incidence graph, adjacent faces, open and disconnected
shells, orientation conflicts, nonmanifold edges, a pinched vertex link, invalid handles,
unused records, endpoint mismatch and budgets. The tetrahedral test checks topology;
it does not imply surface/volume validation. Compile-fail examples protect handle kinds
and the raw-to-normalized boundary. A shared bounded mutation harness exercises all
three new layers. Public C/C++ geometry/topology operations remain pending under
[C_API.md](C_API.md); no Rust layout is part of that ABI.
