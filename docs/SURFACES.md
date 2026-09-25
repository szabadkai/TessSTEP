# Independent surfaces

Milestone 8 supplies analytic planes, cylinders, cones, spheres and ring tori through
`tessstep-surfaces` / `tessstep::surfaces`. Milestone 10 adds `NurbsSurface`, described
in [NURBS.md](NURBS.md). These evaluators consume constructed geometry, not STEP records.
No topology, trimming, tessellation or schema-to-geometry adapter is implied.

## Analytic parameterization

An explicit `PlaneFrame<S,3>` supplies origin O and unit axes X,Y. Z is the normalized
X cross Y. Coordinates/lengths are metres. Let E(u)=cos(u) X + sin(u) Y; u is unwrapped
azimuth in radians, with period 2 pi. Constructors provide these surfaces:

| Surface | Position | Parameter v |
|---|---|---|
| Plane | O + u X + v Y | Both u and v are unbounded distances in metres |
| Cylinder | O + r E(u) + v Z | Unbounded axial distance in metres |
| Cone | O + v sin(a) E(u) + v cos(a) Z | Nonnegative slant distance; a is semi-angle in (0, pi/2) |
| Sphere | O + r cos(v) E(u) + r sin(v) Z | Latitude in closed [-pi/2, pi/2] |
| Ring torus | O + (R + r cos(v)) E(u) + r sin(v) Z | Unwrapped angle, period 2 pi; R > r > 0 |

Radii must be positive and finite. Spindle/horn tori and degenerate cone angles are
explicitly unsupported. A cone's frame origin is its vertex, not a STEP reference-radius
placement. A future adapter must convert schema parameter conventions explicitly.
No axes or modeling tolerances are defaulted by the geometry evaluator.

`evaluate(u,v)` returns position, du, dv, duu, duv and dvv atomically; all stored
coordinates and derivatives must be finite. Periodic parameters are accepted without
manual wrapping. Sphere latitude and cone slant domains are checked exactly. Standard
floating-point trig, cancellation/underflow and conservative overflow failures follow
[NUMERICAL_ROBUSTNESS.md](NUMERICAL_ROBUSTNESS.md); no certified error is claimed.

## Normals, singularities and transforms

`Evaluation::normal(tolerance)` normalizes both first partials before computing their
cross product. Zero partials or a sine separation at/below the dimensionless numerical
tolerance produce `SingularNormal`; successful evaluation alone does not mean regularity.
At the cone apex du is exactly zero. At either exact sphere-latitude endpoint the cosine
factor is set to zero, so numerical sin/cos residue cannot hide the pole singularity.
Near a pole, nonzero partials remain legitimate; no modeling-distance cutoff is inferred.

`Evaluation::transformed` applies an affine map to the position and the linear part to
all five derivative vectors. A singular map may collapse the surface. The resulting
normal follows transformed du cross dv, including reflected orientation. It is a
parametric normal, not proof of B-rep face orientation, manifoldness or mesh validity.

Tests check all five derivatives independently using finite differences, known points,
implicit loci, periodic seams, poles/apex, rational patch derivatives, refinement and
affine covariance. Public C/C++ surface operations are pending under [C_API.md](C_API.md);
no Rust surface representation crosses that boundary.
