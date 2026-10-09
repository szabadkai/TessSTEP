# UV trimming

Milestone 12 provides bounded supplied-pcurve reconstruction in `tessstep-trim` /
`tessstep::trim`. `reconstruct` takes a `NormalizedBrep`, a face handle and explicit
parameter-space tolerance/resource options. It returns an immutable outer polygon,
hole polygons, oriented per-coedge samples, recorded periodic chart shifts and signed
areas. Point classification returns inside, outside or boundary in that UV chart.

Each coedge must supply a pcurve whose interval maps affinely to the canonical edge
interval. Reversal changes traversal, not that correspondence. Analytic and NURBS
pcurves are supported. Knot spans and quarter-period analytic intervals seed iterative
subdivision; seeds within 1e-9 of the parameter range of each other or of its ends (a
knot written a few ulps from an edge end) are dropped, as in shared-edge sampling, so
they add no segment of zero length. Quarter/mid/three-quarter probes measure UV chord
error, which must stay within the UV tolerance and within a quarter of the chord (down
to a thousandth of the tolerance), so a loop much smaller than the UV tolerance (a
sub-micrometre face in a metre chart) keeps the shape of its curves; each probe also
checks its surface lift against the corresponding 3D edge point using the model distance
tolerance. Singular surface normals and evaluation failures are errors. One-sided knot
sampling diagnoses discontinuous pcurves. General nonlinear parameter correspondence,
missing-pcurve reconstruction and arbitrary 3D projection are not implemented here;
the [curved STEP importer](CURVED_IMPORT.md#computed-pcurves) computes pcurves before
trimming.

## Periodic charts and loops

Pcurve sweeps remain unwrapped, including full periods. At a coedge join, periodic
surface axes permit translating the whole next use by an integer period to meet the
previous endpoint. The shift is recorded and both uses of a seam remain distinct.
Interior pcurve samples are never individually wrapped. A hole whose first point is
not inside the outer polygon is translated by whole periods of the periodic axes into
the outer loop's chart (the nearest-center shift and its neighbours are tried); the
shift is recorded in every use of the hole and no geometry changes. A noncontractible loop that fails to close in the unwrapped plane is rejected. A full
cylinder strip works when its two explicit seam uses close the UV rectangle.

Joining uses must agree within the UV tolerance, or within the model tolerance in
metres: chart axes have unrelated units (radians on a millimetre tube beside metres
along a cylinder), so chart offsets are scaled by the largest surface derivative
sampled along each axis, and a join whose ends coincide on the surface within the
model tolerance is accepted even when the pcurves miss each other in UV (a vertex
may sit that far from its curve, and a seam meets such a vertex with the same gap).
Singular points never qualify, and a closure gap near a whole period stays a
non-contractible loop. Per-use endpoint samples are retained; the polygon uses the
next coedge's start at a join, so its chord can differ by that joining tolerance.
This does not edit the source pcurve. Collapsed polygon segments, backtracking,
self-intersections, boundary contacts and degenerate loops are rejected; when the
surface gives an axis scale these checks run in metres with a thousandth of the
model tolerance, so a corner a few nanometres wide in one axis is not a backtrack.
In that metric chart a conflict within one loop that is confined to the model
tolerance is a wobble, not a defect: two adjacent segments may double back when one
of them is at most the model tolerance long, and two other segments may touch when
the part of the loop between them stays within a box of that diagonal (a fitted
pcurve's hook at a vertex the edge reaches within the tolerance). Outer winding must
be counterclockwise and hole winding clockwise in the surface chart; face orientation
is separately retained in topology. Holes must be strictly inside the outer polygon,
mutually disjoint and unnested, decided in the face's metric chart (the larger scale
of its loops per axis). No automatic winding repair occurs.

Every failure is located where it can be: `Error::coedge` names the coedge of the
failing use or segment, `Error::other` the coedge of the second segment of a crossing,
touching or backtracking pair, `Error::wire` the loop (for a conflict between two
loops, the later one), and `Error::budget` the exhausted budget with its configured
value (`Budget::MaxSamples` or `Budget::MaxWork`). The
[curved importer](CURVED_IMPORT.md#uv-loop-failures) maps these to STEP entities and a
cause class.

Segment proximity and orientation use finite f64 arithmetic with an explicit UV distance
tolerance. Tests establish polygon behavior, not exact predicates. The chord checks are
sampled estimates, not certified global error bounds for arbitrary high-degree rational
curves; crossings or lift disagreements between probes may remain undetected. Increasing
sampling quality does not replace exact geometric validity checks. Classification applies
to the reconstructed polygons, not the exact continuous curves.

Logical sample/work limits and a hard subdivision depth ceiling of 48 bound execution.
`Options::max_samples` (pcurve evaluations across the face's loops) defaults to its
ceiling of one million: a plane with a thousand sampled holes needs a few hundred
thousand. Self-intersection and hole tests sweep segment (and hole) boxes along the
wider chart axis and compare only boxes that overlap within the tolerance, charging
one work unit per comparison; the worst case (every box overlapping) stays quadratic
and bounded by the budget, but spread-out boundaries no longer pay for every pair (a
4,096-sample loop used to need 16.7 million units).
Failure to satisfy a tolerance within depth/evaluation limits returns a typed error;
no partial face is published. Tests include planar holes, curved analytic/NURBS boundaries,
full cylinder seams, wrapped pcurve chart alignment, missing/mismatched pcurves, singular
surfaces, winding, intersections, noncontractible loops and resource exhaustion.

`validate_polygons(outer, holes, uv_tolerance, max_work)` exposes the same winding,
intersection, containment and hole-relationship checks for callers with an existing
polygon resolution, in the unscaled chart at the given tolerance and without the
wobble allowance. `TrimmedFace::validate_resolution(loops, uv_tolerance, max_work)`
runs them on a face's loops at another resolution whose points carry their coedges,
and locates failures like reconstruction. The shared-edge face boundary uses it at
floating-point resolution ([tessellation](TESSELLATION.md)); passing trim
reconstruction at a different resolution is not sufficient. Neither function changes
vertices or evaluates source curves, and both charge a finite work budget.
