# Tessellation

Milestone 13 provides shared-edge sampling in `tessstep-tessellate` /
`tessstep::tessellate`. Face triangulation, interior refinement and watertight solid
meshes (Milestones 14–17) remain unimplemented. This API emits boundary polylines.

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
outer/hole boundary uses for later constrained triangulation. This does not replace
UV polygon validity checks at a future triangulator's actual boundary resolution.

## Bounds and limitations

Logical output/evaluation limits and a depth ceiling of 48 fail explicitly without
publishing a partial cache. Periodic seeding and knot boundaries avoid common aliasing,
but fixed interior probes do **not** certify the global error of arbitrary rational
curves between probes. No exact predicates or certified Hausdorff bound are claimed.
Small tolerances may fail through depth, floating-point spacing or resource limits.
Collapsed curves are diagnosed; automatic singular-edge repair is not implemented.

Tests independently verify circle sagitta/tangent bounds, tighter-tolerance refinement,
line minimal sampling, canonical vertex identity, zero-copy reversed/shared/seam views,
NURBS knot corners and discontinuities, UV mappings, degenerate curves and hard failures.
A deterministic mutation harness, libFuzzer driver and boundary benchmark cover the
combined pipeline. STEP adaptation and external geometry/tessellation corpus stages
remain pending. Exporters belong outside the kernel. Public C/C++ mesh views will follow
[C_API.md](C_API.md) when actual mesh storage and consumer tests exist.
