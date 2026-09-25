# Geometry foundation

Status: Milestones 6–10 provide independent math, analytic curves/surfaces and NURBS
curves/surfaces. Milestones 11–17 add independent topology, UV reconstruction,
shared-edge sampling, adaptive regular-face tessellation and owned manifold meshes. STEP geometry adaptation and external geometry corpus validation remain
pending. See [CURVES.md](CURVES.md), [SURFACES.md](SURFACES.md) and [NURBS.md](NURBS.md).

`tessstep-math` (also `tessstep::math`) has no STEP or schema dependency. It provides
immutable finite `Point2/3`, `Vector2/3`, normalized `Direction3`, `Length`, `Angle`,
`LengthUnit`, model/tessellation tolerances and typed `Affine3<From, To>` maps. Callers
can construct and test every primitive without a STEP file. See
[NUMERICAL_ROBUSTNESS.md](NUMERICAL_ROBUSTNESS.md) for units and numerical limits.

## Transform contract

For a column-vector point, `p_to = linear * p_from + translation`. The 3x3 matrix is
stored row-major, and the translation uses destination-space units. `a.then(b)` applies
`a` first and `b` second, yielding `b * a`; its intermediate space must match. Inversion
swaps source/destination tags. Singular finite maps may be applied but cannot be inverted.

Point transformation includes translation. Vector transformation excludes it. Direction
transformation applies the linear map and normalizes; collapsing a direction to zero is
an error. Normals use the inverse transpose and normalize, preserving perpendicularity
under nonuniform scale and shear. For reflections, this does not adjust the normal to
match triangle winding; future topology/mesh code must handle orientation explicitly.

`Affine3::from_frame` accepts an explicit origin, z direction and x reference. It builds
y from z cross reference, then x from y cross z, and stores x/y/z as matrix columns.
Parallel/nearly parallel inputs fail. The frame is orthonormal and right-handed to
floating-point accuracy. No STEP axis default or item-defined transformation order is
inferred. `Affine3::rotation` provides an axis-angle Rodrigues rotation about the origin.

```rust
use tessstep_math::{Affine3, Direction3, LocalSpace, ModelSpace, Point3};
let local_to_model = Affine3::<LocalSpace, ModelSpace>::from_frame(
    Point3::new([0.01, 0.02, 0.03])?,
    Direction3::new([0., 0., 1.])?,
    Direction3::new([1., 0., 0.])?,
    Default::default(),
)?;
let model_point = local_to_model.transform_point(Point3::new([1., 0., 0.])?)?;
# Ok::<(), tessstep_math::Error>(())
```

## Next layers

Analytic and NURBS curves/surfaces expose evaluation and derivatives without raw
entity access. Their tests are independent of geometry schema adapters. The Milestone 5 product adapter
now uses this foundation to evaluate placement descriptions and expand bounded assemblies
into world coordinates.
The external corpus therefore gains no geometry/tessellation acceptance claim.

The Rust math API is not a C layout contract. Public C/C++ operations retain explicit C contracts; Rust product/math types are not
exposed directly. See [C_API.md](C_API.md).
