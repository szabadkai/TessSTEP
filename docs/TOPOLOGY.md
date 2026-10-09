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
changes coordinates, reverses wires, or silently heals geometry. Repair is a separate,
explicit stage on raw B-reps that callers run before validation and whose every edit
is counted ([opt-in repair](#opt-in-repair)); validation and normalization never call it.

Validation checks finite increasing edge ranges, evaluable endpoints and agreement
with vertices within an explicit model distance tolerance, typed references, exact
vertex-identity wire closure, unique coedge/wire/face ownership, and unused vertices,
edges, coedges and wires. Missing pcurves are retained for the trim stage to diagnose.
Unsewn faces and open shells are permitted. Declared shells must be nonempty and
connected through edges; at most two uses meet each edge and paired effective
orientations oppose. Vertex links must be connected paths or cycles. Declared closed
shells require two uses per edge and cyclic vertex links. Solids reference one uniquely
owned closed shell plus closed, uniquely owned cavity shells (`Solid::voids`). A
solid's shells may not share an edge (`NonManifoldEdge`) or vertex
(`NonManifoldVertex`); cavity containment and nesting are not checked. Evidence:
`topology_cavity_shells_are_closed_and_uniquely_owned`.

A closed shell of several faces that share no edge and no vertex, each face with its
own boundary records, is a *disconnected* (unsewn) closed shell and fails as
`UnsewnShell` rather than `OpenShell`, so that this exporter defect is classified
apart from shells with missing faces or isolated free edges. Evidence:
`repair_unsewn_closed_shells_are_classified_apart_from_open_shells`.

These are structural validity states: validation does **not** certify self-intersection,
face/surface agreement, continuous curve interiors, shell outwardness, positive volume,
or containment. UV reconstruction checks sampled pcurve agreement separately. Surface
singularities are diagnosed by downstream operations; structural acceptance does not
establish regular geometry. No mesh or watertightness claim follows.

## Opt-in repair

`tessstep_topology::repair` holds the tolerance-bounded repairs that an importer may
apply, on request, to a raw B-rep before validating it. None changes a coordinate, a
curve or a surface; each returns what it did, and the caller reports it.

* **Sewing.** `plan_sewing(raw, model, tolerance, limits)` chooses merges and
  `SewPlan::apply` performs them. Candidates are the end vertices of *free* edges (one
  use in all shells, not collapsed) and the two vertices at each wire break, in one
  shell. Candidate pairs within `tolerance` are merged nearest first (constrained single
  linkage): a cluster keeps the position of its lowest-indexed vertex, and a merge is
  refused when a member would lie farther than `tolerance` from it or when it would put
  both ends of an edge into one cluster (no edge ever collapses, however short). Then
  each pair of free edges of different faces that join the same merged vertices and
  whose curves lie within `tolerance` of each other (exactly for two lines, otherwise
  17 samples of each projected onto the other) becomes one edge: the analytic curve
  survives a B-spline, otherwise the lower index. Three or more coincident free edges
  stay unsewn. A moved use flips when the survivor runs the other way, and `apply`
  swaps its pcurve interval, which keeps an affine correspondence only for straight
  edges; an importer with curved pcurves maps the plan onto its own records before it
  computes them. The plan reports merged vertices and edges, the largest merge
  distance and the closed shells that were disconnected before sewing. Because
  positions are kept, a sewn model must be validated, trimmed and tessellated at the
  model tolerance plus the largest merge distance.
* **Face orientation.** `orient_faces(raw, limits)` flips faces so that, in each shell,
  the two uses of every edge shared by two faces have opposite effective orientations.
  Each edge-connected group of faces keeps the orientation of most of its faces (its
  first face on a tie); a group that no flips make consistent is a non-orientable
  `InconsistentOrientation` error. Only `Face::orientation` changes: loops stay
  oriented with their surface. A face whose loops run against its surface is found by
  the trimmer (`WrongOrientation`), so the importers reverse such loops after
  validation, together with the face orientation.
* **Gaps.** When a sewn model still fails as `OpenShell`, `UnsewnShell` or `BrokenWire`,
  `diagnose_gap` measures what sewing would need: the distance from the shell's first
  free edge to the nearest other free edge, or across the wire's first break. A
  `RepairError` names the edges, the needed distance and the tolerance in force.

Loop direction and outward orientation need the trimmer and the algebraic volume of
each closed shell, so the importers decide them after validation (see
[curved import](CURVED_IMPORT.md#opt-in-topology-repair)). Repairs run on models with
valid handles only; any other model is left for validation to diagnose. Every repair
charges the caller's work budget. Evidence: `tests/repair.rs` in this crate
(`repair_sews_coincident_boundaries_into_a_valid_closed_shell`,
`repair_merges_only_within_the_sewing_tolerance`,
`repair_never_collapses_an_edge_and_mends_broken_wires`,
`repair_orients_faces_by_the_majority_of_each_group`,
`repair_budgets_and_invalid_models_are_explicit`).

## Collapsed edges

A closed edge whose curve stays within the model tolerance of its vertex (checked at
the quarter, mid and three-quarter parameters) is *collapsed*
(`NormalizedBrep::is_collapsed`). It represents a singular line of a face chart, such
as a sphere pole or cone apex, not a 3D boundary: it may have a single use in a closed
shell, bounds only the face whose chart it closes, and its two ends form one
vertex-link incidence. Its vertex is *singular* (`NormalizedBrep::is_singular_vertex`):
trimming, boundary mapping and tessellation accept singular surface normals on
collapsed edges and at singular vertices. A closed edge that leaves its vertex is not
collapsed and follows the ordinary two-use rule. Evidence:
`topology_collapsed_edges_close_pole_and_apex_charts`.

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

Vertex links distinguish the two endpoint incidences of a closed edge, even when
both refer to the same vertex. This permits closed circular boundaries with cap/seam
uses and doubly periodic faces while retaining pinched-vertex rejection.
