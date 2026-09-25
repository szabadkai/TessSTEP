# UV trimming

Milestone 12 provides bounded supplied-pcurve reconstruction in `tessstep-trim` /
`tessstep::trim`. `reconstruct` takes a `NormalizedBrep`, a face handle and explicit
parameter-space tolerance/resource options. It returns an immutable outer polygon,
hole polygons, oriented per-coedge samples, recorded periodic chart shifts and signed
areas. Point classification returns inside, outside or boundary in that UV chart.

Each coedge must supply a pcurve whose interval maps affinely to the canonical edge
interval. Reversal changes traversal, not that correspondence. Analytic and NURBS
pcurves are supported. Knot spans and quarter-period analytic intervals seed iterative
subdivision. Quarter/mid/three-quarter probes measure UV chord error; each probe also
checks its surface lift against the corresponding 3D edge point using the model distance
tolerance. Singular surface normals and evaluation failures are errors. One-sided knot
sampling diagnoses discontinuous pcurves. General nonlinear parameter correspondence,
missing-pcurve reconstruction and arbitrary 3D projection are not implemented.

## Periodic charts and loops

Pcurve sweeps remain unwrapped, including full periods. At a coedge join, periodic
surface axes permit translating the whole next use by an integer period to meet the
previous endpoint. The shift is recorded and both uses of a seam remain distinct.
Interior pcurve samples are never individually wrapped. Holes must be supplied in the
same explicit chart as the outer boundary; automatic hole-chart placement is pending.
A noncontractible loop that fails to close in the unwrapped plane is rejected. A full
cylinder strip works when its two explicit seam uses close the UV rectangle.

Joining uses must agree within the UV tolerance. Per-use endpoint samples are retained;
the polygon uses the next coedge's start at a join, so its chord can differ by that
joining tolerance. This does not edit the source pcurve. Collapsed polygon segments,
backtracking, self-intersections, boundary contacts and degenerate loops are rejected.
Outer winding must be counterclockwise and hole winding clockwise in the surface chart;
face orientation is separately retained in topology. Holes must be strictly inside the
outer polygon, mutually disjoint and unnested. No automatic winding repair occurs.

Segment proximity and orientation use finite f64 arithmetic with an explicit UV distance
tolerance. Tests establish polygon behavior, not exact predicates. The chord checks are
sampled estimates, not certified global error bounds for arbitrary high-degree rational
curves; crossings or lift disagreements between probes may remain undetected. Increasing
sampling quality does not replace exact geometric validity checks. Classification applies
to the reconstructed polygons, not the exact continuous curves.

Logical sample/work limits and a hard subdivision depth ceiling of 48 bound execution.
Self-intersection tests are quadratic and charge the work budget before comparison.
Failure to satisfy a tolerance within depth/evaluation limits returns a typed error;
no partial face is published. Tests include planar holes, curved analytic/NURBS boundaries,
full cylinder seams, wrapped pcurve chart alignment, missing/mismatched pcurves, singular
surfaces, winding, intersections, noncontractible loops and resource exhaustion.

`validate_polygons(outer, holes, uv_tolerance, max_work)` exposes the same winding,
intersection, containment and hole-relationship checks for callers with an existing
polygon resolution. Planar triangulation uses it on canonical shared-edge samples;
passing trim reconstruction at a different resolution is not sufficient. The function
neither changes vertices nor evaluates source curves, and charges a finite work budget.
