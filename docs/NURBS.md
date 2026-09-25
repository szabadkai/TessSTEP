# NURBS curves and surfaces

Milestones 9–10 implement independent positive-weight rational B-spline curves and
tensor-product surfaces. `tessstep::curves::spline` exposes `KnotVector`, `NurbsCurve`,
`KnotSide`, `SplineLimits` and typed failures. `tessstep::surfaces::NurbsSurface` reuses
the curve crate's knot/basis implementation. No STEP adapter, trimming, projection,
intersection, fitting, tessellation, knot removal or degree elevation is implemented.

## Published algorithms

The implementation directly uses the iterative form of the Cox–de Boor basis recurrence
and its derivative recurrence in Patrikalakis, Maekawa and Cho, *Shape Interrogation for
Computer Aided Design and Manufacturing*, [§1.4.1, equations 1.59 and 1.61](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node16.html).
A triangular table computes values and two derivatives on local support. It is not a
recursive traversal and does not claim to implement the book's separate de Boor evaluator.

Single-knot refinement uses [Boehm insertion, §1.4.3, equations 1.75–1.76](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node18.html),
with weighted control coordinates and positive weights. Surface evaluation combines the
two bases as a [tensor product (§1.4.4)](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node19.html).
Homogeneous division uses the [rational curve/surface representations (§1.5, equations 1.87 and 1.91)](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node20.html).
First/second derivatives follow by differentiating the product w P = H, including the
mixed term for surfaces. Code is independently implemented here; no external kernel or
numerical dependency is linked.

## Construction and budgets

The supported degree range is 1–16. Curves have at least degree+1 controls in dimension
2 or 3. Surface controls are a rectangular 3D net with explicit `[nu,nv]` shape;
control (i,j) is at flat index `i*nv+j`. Dimension products are checked for overflow.
Polynomial B-splines use all-one weights; every other weight must be finite and positive.
Zero/negative/projective weights are unsupported. A common positive scale normalizes
stored weights; getters expose that normalized scale. If relative weights underflow to
zero, construction fails rather than silently deleting a weighted control.

Knots are supplied expanded, including repetitions. The knot count must equal control
count + degree + 1 per axis. Knots must be finite and nondecreasing, with multiplicity
at most degree+1. The full knot range must have finite width; the active interval
`[U[degree], U[control_count]]` must have positive width. Nonclamped knot vectors and
interior discontinuities of multiplicity degree+1 are accepted. Duplicate knot values
are compared exactly; no tolerance merges distinct knots.

`SplineLimits` defaults to 100,000 controls and 100,034 knots. Control storage is also
hard-capped at 1,000,000 entries; per-axis knots are capped at that plus 17. For surfaces,
`max_controls` bounds the entire net and `max_knots` bounds the sum of both vectors.
Counts, weights and knot structure are checked before copying the corresponding storage;
an invalid second surface axis can leave a bounded temporary first-axis vector to drop.
Insertion checks output sizes before allocating. These are logical limits, not an exact
RSS guarantee or recoverable process-allocation-failure promise.

## Evaluation and knot sides

Curve evaluation returns position, first and second derivatives. Surface evaluation
returns position, du, dv, duu, duv and dvv. Parameters must be finite and inside the
closed active domain; they are never clamped, extrapolated or implicitly wrapped.
Interior knots default to their right-hand value/derivatives. `evaluate_on_side` and
`evaluate_on_sides` select left or right explicitly. Both endpoints always use their
inward side, including nonclamped and repeated terminal knots. A reported derivative
at a break is one-sided, not a claim of continuity there.

The basis table starts on the chosen nonempty span and treats zero-denominator
repeated-knot terms as zero. Binary span selection plus fixed degree bounds make
curve evaluation O(log n + p²); a surface adds O(log m + q² + (p+1)(q+1)). Evaluation
uses stack scratch with no heap allocation. Constructors and single insertion copy
O(n) curve or O(nu*nv) surface storage. There is no recursive basis evaluation.

For a curve, homogeneous sums H and w give P=H/w, P'=(H'-w'P)/w and
P''=(H''-2w'P'-w''P)/w. Surface mixed differentiation uses
Puv=(Huv-wu Pv-wv Pu-wuv P)/w. Weights on active support are scaled again by their
local maximum before summation; this changes no rational geometry. Every intermediate
sum and output is checked. A zero/underflowed denominator, non-finite derivative or
unrepresentable arithmetic is a typed failure. Ordinary rounding/cancellation and
underflow can still lose information; no exact predicate or accuracy certificate is
claimed, and even a mathematically finite expression can fail on intermediate overflow.

Periodic control/knot representations are accepted as supplied and tested at their seam;
closure and continuity are not inferred or certified. Callers must supply the repeated
controls/extended knots for their intended period. Automatic periodic construction,
period detection and wrapping remain unsupported.

## Refinement

`insert_knot` inserts one strictly interior parameter, preserving geometry and its
parameterization to floating-point accuracy. Its existing multiplicity must be below
the degree. Endpoint insertion, removal, degree elevation and multi-knot batch insertion
are unsupported. The original object is immutable, and a failure publishes no partial
replacement. Repeated single insertions are possible within the same output budgets.

Surface insertion applies the same homogeneous blend to every affected row or column
and keeps their shared weight scale. Independently renormalizing rows would change the
surface, so that approach is deliberately avoided. Only the final complete net receives
a common normalization. Tests compare positions and all derivatives before/after both
axis insertions, including unequal weights across rows.

Evidence includes a rational quarter circle and cylindrical patch, known weighted
bilinear partials, finite differences, one-sided knot breaks, clamped/unclamped endpoints,
periodic seams, degree-16 basis behavior, affine and weight-scale invariance, output
budgets and deterministic mutation/prefix/random-bit smoke. See [VALIDATION.md](VALIDATION.md)
for observed checks. External STEP corpus geometry remains `not_implemented` until
schema-to-geometry adaptation and a real checker are available. The C/C++ ABI remains
physical-document-only and does not expose these Rust layouts.
