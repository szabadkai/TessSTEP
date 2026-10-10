//! Milestone 22.4: mesh-resolution boundary checks resolve what the triangulator
//! resolves, and their failures name the coedges involved.
#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use support::*;
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_tessellate::*;
use tessstep_topology::*;

fn normalized(raw: RawBrep) -> NormalizedBrep {
    raw.validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize()
}
fn chord(metres: f64) -> TessellationTolerance {
    TessellationTolerance::new(Length::metres(metres).unwrap(), Angle::radians(1.).unwrap())
        .unwrap()
}

#[test]
fn features_below_the_uv_tolerance_triangulate() {
    // A 5 µm edge in a metre chart, below the 1e-5 UV chord tolerance, but far above
    // the floating-point resolution of the polygon and the 10 nm model tolerance.
    let n = normalized(polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [1. - 5e-6, 1., 0.],
            [0., 1., 0.],
        ],
        &[vec![0, 1, 2, 3, 4]],
    ));
    let s = sample_edges(&n, chord(1e-3), SamplingLimits::default()).unwrap();
    let mesh = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    assert_eq!(mesh.triangles().len(), 3);
    let mesh = tessellate_faces(
        &n,
        &[FaceId(0)],
        chord(1e-3),
        TessellationOptions::default(),
    )
    .unwrap();
    let d = mesh.data();
    let area: f64 = d
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| d.positions[i as usize]);
            0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
        })
        .sum();
    assert!((area - 1.).abs() < 1e-12, "{area}");
}

#[test]
fn coarse_boundaries_fail_at_their_coedges() {
    // A unit disk with a square hole: at a 0.4 m chord and a 1 rad angle the rim is a
    // square whose side x + y = 1 cuts through the hole, which the exact rim does not.
    let mut raw = disk();
    let h = polygons(
        &[
            [0.45, 0.45, 0.],
            [0.45, 0.6, 0.],
            [0.6, 0.6, 0.],
            [0.6, 0.45, 0.],
        ],
        &[vec![0, 1, 2, 3]],
    );
    raw.vertices.extend(h.vertices);
    raw.edges.extend(h.edges.into_iter().map(|mut e| {
        e.vertices = e.vertices.map(|v| VertexId(v.0 + 1));
        e
    }));
    raw.coedges.extend(h.coedges.into_iter().map(|mut c| {
        c.edge.0 += 1;
        c
    }));
    raw.wires.push(Wire {
        coedges: (1..5).map(CoedgeId).collect(),
    });
    raw.faces[0].holes.push(WireId(1));
    let n = normalized(raw);
    let s = sample_edges(&n, chord(0.4), SamplingLimits::default()).unwrap();
    let located = |e: &tessstep_trim::Error| {
        assert_eq!(e.kind, tessstep_trim::ErrorKind::IntersectingLoops);
        // The rim (coedge 0) against one of the hole's coedges, in the hole's wire.
        assert_eq!(e.coedge, Some(CoedgeId(0)));
        assert!(matches!(e.other, Some(CoedgeId(1..=4))));
        assert_eq!(e.wire, Some(WireId(1)));
    };
    match s.face_boundary(FaceId(0), tessstep_trim::Options::default(), 10_000) {
        Err(BoundaryError::Resolution(e)) => located(&e),
        other => panic!("{other:?}"),
    }
    match s.triangulate_planar(FaceId(0), PlanarOptions::default()) {
        Err(PlanarError::InvalidPolygon(e)) => located(&e),
        other => panic!("{other:?}"),
    }
}

/// A planar horn at the origin V: a unit arc from V, a line back across, and a
/// clamped cubic B-spline returning to V, its knots dense toward V and its first
/// control leg along the x axis, so that it meets the arc tangentially at V (the
/// NIST CTC 04 sphere face's B-spline edge ends tangent to its arc that way). With
/// `p1` too long the spline dips below the arc and crosses it.
fn horn(p1: f64) -> RawBrep {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_3};
    use tessstep_curves::spline::{NurbsCurve, SplineLimits};
    use tessstep_curves::{Curve, PlaneFrame};
    use tessstep_math::{NumericalTolerance, Point, Space, Vector};
    let a = [FRAC_PI_3.sin(), 1. - FRAC_PI_3.cos()];
    let b = [0.5, 0.5];
    let mut knots = vec![0.; 4];
    knots.extend([1. / 64., 1. / 32., 1. / 16., 1. / 8., 1. / 4., 1. / 2.]);
    knots.extend([1.; 4]);
    let count = knots.len() - 4;
    // Control points on the circle of radius 0.5 tangent to the arc at V, at their
    // Greville abscissae, but the first leg along the common tangent.
    let mut controls: Vec<[f64; 2]> = (0..count)
        .map(|i| {
            let psi = (knots[i + 1] + knots[i + 2] + knots[i + 3]) / 3. * FRAC_PI_2;
            [0.5 * psi.sin(), 0.5 * (1. - psi.cos())]
        })
        .collect();
    controls[1] = [p1, 0.];
    fn spline<S: Space, const N: usize>(knots: &[f64], points: &[Point<S, N>]) -> NurbsCurve<S, N> {
        let weights = vec![1.; points.len()];
        NurbsCurve::new(3, knots, points, &weights, SplineLimits::default()).unwrap()
    }
    fn arc<S: Space, const N: usize>(
        o: Point<S, N>,
        x: Vector<S, N>,
        y: Vector<S, N>,
    ) -> PlaneFrame<S, N> {
        PlaneFrame::new(o, x, y, NumericalTolerance::default()).unwrap()
    }
    let edge = |vertices: [usize; 2], curve, range| Edge {
        vertices: vertices.map(VertexId),
        curve,
        range,
    };
    let pcurve = |curve, range| Some(Pcurve { curve, range });
    let coedge = |edge: usize, orientation, pcurve| Coedge {
        edge: EdgeId(edge),
        orientation,
        pcurve,
    };
    let point3 = |p: [f64; 2]| Point::new([p[0], p[1], 0.]).unwrap();
    RawBrep {
        vertices: [[0., 0.], a, b]
            .into_iter()
            .map(|p| Vertex {
                position: point3(p),
            })
            .collect(),
        edges: vec![
            edge(
                [0, 1],
                CurveGeometry::Analytic(
                    Curve::circle(
                        arc(
                            point3([0., 1.]),
                            Vector::new([0., -1., 0.]).unwrap(),
                            Vector::new([1., 0., 0.]).unwrap(),
                        ),
                        Length::metres(1.).unwrap(),
                    )
                    .unwrap(),
                ),
                [0., FRAC_PI_3],
            ),
            edge(
                [1, 2],
                CurveGeometry::Analytic(
                    Curve::line(point3(a), Vector::new([b[0] - a[0], 0., 0.]).unwrap()).unwrap(),
                ),
                [0., 1.],
            ),
            edge(
                [0, 2],
                CurveGeometry::Nurbs(spline(
                    &knots,
                    &controls.iter().map(|&p| point3(p)).collect::<Vec<_>>(),
                )),
                [0., 1.],
            ),
        ],
        coedges: vec![
            coedge(
                0,
                Orientation::Forward,
                pcurve(
                    CurveGeometry::Analytic(
                        Curve::circle(
                            arc(
                                Point::new([0., 1.]).unwrap(),
                                Vector::new([0., -1.]).unwrap(),
                                Vector::new([1., 0.]).unwrap(),
                            ),
                            Length::metres(1.).unwrap(),
                        )
                        .unwrap(),
                    ),
                    [0., FRAC_PI_3],
                ),
            ),
            coedge(
                1,
                Orientation::Forward,
                pcurve(
                    CurveGeometry::Analytic(
                        Curve::line(
                            Point::new(a).unwrap(),
                            Vector::new([b[0] - a[0], 0.]).unwrap(),
                        )
                        .unwrap(),
                    ),
                    [0., 1.],
                ),
            ),
            coedge(
                2,
                Orientation::Reversed,
                pcurve(
                    CurveGeometry::Nurbs(spline(
                        &knots,
                        &controls
                            .iter()
                            .map(|&p| Point::new(p).unwrap())
                            .collect::<Vec<_>>(),
                    )),
                    [0., 1.],
                ),
            ),
        ],
        wires: vec![Wire {
            coedges: (0..3).map(CoedgeId).collect(),
        }],
        faces: vec![Face {
            surface: plane(),
            outer: WireId(0),
            holes: vec![],
            orientation: Orientation::Forward,
        }],
        ..RawBrep::default()
    }
}

#[test]
fn chords_crossing_a_tangent_edge_are_refined() {
    let n = normalized(horn(0.001));
    let t = chord(0.01);
    // At the first sampling the arc's chord from V cuts across the spline, which is
    // sampled at its knots right beside V.
    let s = sample_edges(&n, t, SamplingLimits::default()).unwrap();
    match s.face_boundary(FaceId(0), tessstep_trim::Options::default(), 10_000) {
        Err(BoundaryError::Resolution(e)) => {
            assert_eq!(e.kind, tessstep_trim::ErrorKind::SelfIntersection);
            assert_eq!((e.coedge, e.other), (Some(CoedgeId(0)), Some(CoedgeId(2))));
        }
        other => panic!("{other:?}"),
    }
    // The adaptive pipeline refines that chord in a boundary pass and meshes the horn.
    let mesh = tessellate_faces(&n, &[FaceId(0)], t, TessellationOptions::default()).unwrap();
    let d = mesh.data();
    let area: Vec<f64> = d
        .triangles
        .iter()
        .map(|tri| {
            let [a, b, c] = tri.map(|i| d.positions[i as usize]);
            0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
        })
        .collect();
    assert!(area.iter().all(|&a| a > 0.), "folded triangles");
    // The exact area from densely sampled curves, along the loop.
    let mut boundary = Vec::new();
    for (edge, reversed) in [(0, false), (1, false), (2, true)] {
        let e = &n.data().edges[edge];
        for k in 0..=4096 {
            let f = if reversed { 4096 - k } else { k } as f64 / 4096.;
            let p = e
                .curve
                .evaluate(e.range[0] + f * (e.range[1] - e.range[0]))
                .unwrap()
                .position
                .coordinates();
            boundary.push([p[0], p[1]]);
        }
    }
    let exact: f64 = (0..boundary.len())
        .map(|i| {
            let (p, q) = (boundary[i], boundary[(i + 1) % boundary.len()]);
            0.5 * (p[0] * q[1] - q[0] * p[1])
        })
        .sum();
    let perimeter: f64 = (0..boundary.len())
        .map(|i| {
            let (p, q) = (boundary[i], boundary[(i + 1) % boundary.len()]);
            (q[0] - p[0]).hypot(q[1] - p[1])
        })
        .sum();
    let total: f64 = area.iter().sum();
    assert!(
        (total - exact).abs() <= 0.01 * perimeter,
        "{total} vs {exact}"
    );
    // A spline that crosses the arc is still rejected, by the trim reconstruction.
    let crossing = normalized(horn(0.01));
    match tessellate_faces(&crossing, &[FaceId(0)], t, TessellationOptions::default()) {
        Err(TessellationError {
            kind: TessellationErrorKind::Boundary(BoundaryError::Trim(e)),
            ..
        }) => assert_eq!(e.kind, tessstep_trim::ErrorKind::SelfIntersection),
        other => panic!("{other:?}"),
    }
}
