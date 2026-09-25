# Numerical policy

Status: Milestone 6 checked math foundation implemented in `tessstep-math`.
Exact/adaptive predicates and certified geometric error bounds remain future work.

Physical numeric parsing rejects i64 overflow, non-finite f64 values and nonzero
literals underflowing to zero. It preserves negative zero and representable subnormals.
The math layer independently validates inputs; it does not require the physical parser.

## Finite values and units

Points, vectors, directions, angles, distances, unit scales and transforms have private
storage. Constructors reject NaN/infinity. Fallible operations reject non-finite results;
no unchecked operator overload can silently publish an invalid value. Signed zeros and
subnormals are permitted. A zero vector is valid but cannot define a direction.
Distances are nonnegative metres, angles are signed radians, and explicit length-unit
conversion accepts signed coordinates. Unit/degree conversion rejects a nonzero value
that rounds to zero. General arithmetic permits gradual underflow and rounded zero;
finite storage is not a promise that every tiny contribution survives rounding.

`Point<S, N>` and `Vector<S, N>` carry coordinate-space identities. Zero dimensions are
rejected; the 2D/3D aliases are the intended geometry interfaces. Model/local coordinates
use metres, while parameter coordinates use their evaluator's parameter units. Coordinate
arrays and vector dot/norm results are scalar f64 values in those units; space typing is
not a complete physical-dimension algebra. Points cannot be added to points. Custom frame
tags implement `Space`; no implicit space conversion exists. A caller explicitly supplying
a matrix is responsible for its frame meaning.

## Tolerances

`ModelTolerance` and `TessellationTolerance` are separate types with positive distance
and angular values in (0, pi]. They have no defaults; an eventual evaluator/tessellator
must receive them explicitly. They are value contracts, not an implemented tessellator.
`NumericalTolerance` is dimensionless in (0, 1), defaults to 64 machine epsilons, and is
used only for scaled inversion pivots and the sine between frame axes. It is never
substituted for a modeling distance or tessellation error.

## Algorithms and limitations

Vector norms accumulate `hypot`, avoiding premature overflow/underflow from squaring.
Direction normalization first divides by the largest absolute component, so both
subnormal vectors and vectors with a norm exceeding f64::MAX can normalize. The norm
itself returns an error when its result cannot be represented. Direction angles use
`atan2(norm(cross), dot)` to retain small angles better than `acos(dot)`.

Affine matrices use row-major storage and column-vector application. Inversion scales
by the largest absolute linear entry, then uses partial-pivot Gauss-Jordan elimination
on a fixed 3x6 augmented array. A pivot at or below the numerical threshold is rejected;
uniformly tiny or huge scales do not alone cause rejection. The final inverse and inverse
translation must be finite. The threshold is not a condition-number or error certificate;
ill-conditioned inputs may still lose accuracy. No determinant sign predicate is exposed.

Dots, crosses and matrix products use ordinary floating-point multiplication and sums.
They can conservatively reject intermediate overflow even when exact cancellation would
make the final mathematical answer finite. Underflow and cancellation may lose information.
No exactness, global relative-error bound, bitwise cross-platform identity, tolerance
healing or topology guarantee is claimed. No implicit epsilon equality is provided.

Independent tests cover known affine inverses, normal orthogonality under shear/scale,
composition order, explicit frames, near-parallel rejection, reflections, pivot swaps,
finite-range limits, signed zero, subnormals, 2,000 seeded round trips and arbitrary-bit
smoke. Compile-fail examples verify space separation and transform order. See
[GEOMETRY.md](GEOMETRY.md) for the API and [VALIDATION.md](VALIDATION.md) for observed runs.

## Analytic curve extension

Milestone 7 adds line/conic evaluation and two derivatives using these same finite
value contracts. Orthonormal plane frames reject parallel inputs using a dimensionless
sine threshold. Periodic domains retain unwrapped parameters. Span evaluation applies
both derivative chain-rule factors and checks exact endpoints/range. Trigonometric and
hyperbolic functions use standard f64 routines, not certified interval evaluation;
intermediate overflow can be rejected and underflow can erase small contributions.
See [CURVES.md](CURVES.md) for formulas, parameterization and remaining limitations.

## Surface and rational spline extension

Analytic surfaces return five partial vectors and diagnose a singular normal separately.
Exact sphere poles and cone apex preserve their zero azimuth derivative. NURBS uses
bounded iterative basis recurrence and homogeneous quotient derivatives with positive
weights. Active-support scaling reduces common weight magnitudes; zero relative scales
are rejected during construction. Finite checks do not certify conditioning, continuity
at a repeated knot or a global error bound. One-sided knot behavior and conservative
intermediate-overflow failures are part of the contract. See [NURBS.md](NURBS.md) for
published algorithm sources and [SURFACES.md](SURFACES.md) for surface parameterizations.
