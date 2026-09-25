# Analytic curves

Status: Milestone 7 independent analytic curve evaluation implemented in
`tessstep-curves`, reexported as `tessstep::curves`. The same crate now provides
[NURBS curves](NURBS.md), and the surface crate supplies
[surface evaluators](SURFACES.md). STEP adapters, projection, intersection, arc-length
solvers and tessellation remain pending.

## Values, frames and evaluation

`Curve2<S>` and `Curve3<S>` store a validated analytic basis with private data.
Only dimensions 2 and 3 are accepted. Coordinates and positive radius/focal lengths
use metres; frame tags identify coordinate frames. A 2D curve is not automatically a
surface pcurve: UV parameter units and STEP pcurve association need future adapters.
Line tangents retain their supplied magnitude, in metres per parameter unit.

`PlaneFrame` uses an origin, x axis and reference-y vector. It normalizes the inputs
with scale-first arithmetic. In 2D it chooses the perpendicular x direction with the
reference's orientation; in 3D it normalizes x cross reference, then constructs y from
normal cross x. The sine between the input axes must exceed `NumericalTolerance`.
Zero/parallel inputs fail, no default axes are inferred, and a left-handed 2D frame is
permitted. In 3D, x cross y defines the plane normal. Basis orthonormality has ordinary
floating-point accuracy, not an exact predicate guarantee.

`evaluate(u)` returns a finite position, first derivative and second derivative,
all with respect to the supplied parameter. It fails atomically if any output or
intermediate is non-finite. Derivatives are not normalized. For a frame with origin O
and unit vectors X,Y, the implemented parameterizations are:

| Basis | Position | First derivative | Second derivative |
|---|---|---|---|
| Line | O + u V | V | 0 |
| Circle | O + r cos(u) X + r sin(u) Y | -r sin(u) X + r cos(u) Y | -r cos(u) X - r sin(u) Y |
| Ellipse | O + a cos(u) X + b sin(u) Y | -a sin(u) X + b cos(u) Y | -a cos(u) X - b sin(u) Y |
| Parabola | O + f u² X + 2 f u Y | 2 f u X + 2 f Y | 2 f X |
| Hyperbola | O + a cosh(u) X + b sinh(u) Y | a sinh(u) X + b cosh(u) Y | a cosh(u) X + b sinh(u) Y |

Ellipse axes are never reordered, including when b > a. Parabola focus is O + f X;
the hyperbola represents the positive-X branch. Choose another oriented frame to
represent the other branch. These are explicit library parameterizations, not a claim
that raw STEP parameters can already be evaluated without a semantic adapter.

Circle/ellipse parameters are radians with period 2 pi and fundamental interval
[0, 2 pi). Other bases are unbounded. Every finite parameter can be requested for any
basis; representability still limits successful evaluation. Periodic inputs are passed
to `sin_cos` without manual wrapping, retaining the caller's unwrapped parameter.

`Evaluation<S,3>::transformed` applies an affine map to the position and its linear
part to both derivatives. It preserves parameterization under scale/shear/reflection;
it does not relabel a transformed circle as a circle. Singular maps may collapse a
derivative to zero. No regularity or topology validity is inferred from a finite result.

## Oriented spans

`CurveSpan::new(basis, start, end)` stores distinct finite endpoints with a finite
nonzero difference. Its public evaluation parameter s lies in [0,1], mapping to
u = start + s (end - start). Exact s=0/1 selects the stored endpoints directly.
First/second derivatives use the chain-rule factors delta and delta². Reversal swaps
the endpoints, negates the first derivative at the matching point and preserves the
second. No clamping, modulo reduction, shortest-arc choice or point projection occurs.

For example, a circle span from 3 pi / 2 to 5 pi / 2 crosses the parameter seam in
positive orientation; 0 to 4 pi explicitly traverses twice. A reversed interval is
valid. This represents a curve parameter span, not STEP TRIMMED_CURVE semantics,
shared topological edges or UV trimming loops. Domain checks use exact comparisons;
no modeling tolerance is silently used to admit out-of-range parameters.

## Numerical limits and evidence

Constructors reject zero tangents and nonpositive radii/focal lengths. Math types reject
non-finite coordinate/length inputs. Evaluation may conservatively reject intermediate
overflow even when exact cancellation/scaling would give a finite final result. In
particular, hyperbolic functions must themselves be representable before radius scaling.
Parabola evaluation multiplies the focal scale before the second parameter factor to
avoid unnecessary u² overflow. Gradual underflow and rounded zero follow the math policy;
large angles can lose phase precision and transcendental results may vary by platform.
No certified error bound, adaptive predicate, geometric healing or exact arithmetic is
claimed. See [NUMERICAL_ROBUSTNESS.md](NUMERICAL_ROBUSTNESS.md).

Tests independently check known coordinates, analytic conic loci, finite differences
of positions/first derivatives, frame orientation and arbitrary scales, periodicity,
span reversal/chain rule, affine covariance and invalid/extreme inputs. Stable arbitrary-bit
smoke shares a fixed-work harness with the nightly fuzz target. See
[VALIDATION.md](VALIDATION.md) for observed runs and [TESTING.md](TESTING.md) for commands.

The C/C++ API still exposes physical documents only. These Rust types are not ABI layouts;
future curve operations must follow [C_API.md](C_API.md). No external corpus geometry
acceptance is claimed until a real schema-to-geometry adapter and checker exist.
