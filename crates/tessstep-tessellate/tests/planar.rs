#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use support::*;
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_tessellate::*;
use tessstep_topology::*;

fn sampling() -> TessellationTolerance {
    TessellationTolerance::new(Length::metres(1e-3).unwrap(), Angle::radians(0.1).unwrap()).unwrap()
}
fn normalized(raw: RawBrep) -> NormalizedBrep {
    raw.validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize()
}
fn with_holes(points: &[[f64; 3]], loops: &[Vec<usize>]) -> RawBrep {
    let mut r = polygons(points, loops);
    r.faces.truncate(1);
    r.faces[0].holes = (1..loops.len()).map(WireId).collect();
    r
}
fn holes() -> RawBrep {
    with_holes(
        &[
            [0., 0., 0.],
            [8., 0., 0.],
            [8., 6., 0.],
            [0., 6., 0.],
            [1., 1., 0.],
            [1., 2., 0.],
            [2., 2., 0.],
            [2., 1., 0.],
            [4., 2., 0.],
            [4., 4., 0.],
            [6., 4., 0.],
            [6., 2., 0.],
        ],
        &[vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]],
    )
}
fn check(mesh: &PlanarMesh<'_>, n: &NormalizedBrep, expected_area: f64) {
    let trim =
        tessstep_trim::reconstruct(n, mesh.face(), tessstep_trim::Options::default()).unwrap();
    let mut area = 0.;
    let normal = mesh.normal();
    let mut incidence = std::collections::BTreeMap::new();
    for tri in mesh.triangles() {
        let [a, b, c] = tri.map(|i| {
            mesh.vertices()[i as usize]
                .edge_sample
                .position
                .coordinates()
        });
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cross = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let signed = cross.iter().zip(normal).map(|(a, b)| a * b).sum::<f64>();
        assert!(signed > 0.);
        area += signed * 0.5;
        // Independent interior probes must avoid holes and concavities.
        let uv = tri.map(|i| mesh.vertices()[i as usize].uv);
        for weights in [
            [1. / 3.; 3],
            [0.8, 0.1, 0.1],
            [0.1, 0.8, 0.1],
            [0.1, 0.1, 0.8],
        ] {
            let p = [0, 1].map(|k| (0..3).map(|j| uv[j][k] * weights[j]).sum());
            assert_eq!(
                trim.classify(p).unwrap(),
                tessstep_trim::Containment::Inside
            );
        }
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            *incidence.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    assert!(
        (area - expected_area).abs() < 1e-9,
        "{area} != {expected_area}"
    );
    for boundary in mesh.boundaries() {
        for i in 0..boundary.len() {
            let a = boundary[i];
            let b = boundary[(i + 1) % boundary.len()];
            assert_eq!(incidence.remove(&(a.min(b), a.max(b))), Some(1));
        }
    }
    assert!(incidence.values().all(|&n| n == 2));
}
#[test]
fn planar_concave_holes_area_constraints_and_determinism() {
    for (raw, area) in [
        (square(), 1.),
        (
            polygons(
                &[
                    [0., 0., 0.],
                    [3., 0., 0.],
                    [3., 1., 0.],
                    [1., 1., 0.],
                    [1., 3., 0.],
                    [0., 3., 0.],
                ],
                &[vec![0, 1, 2, 3, 4, 5]],
            ),
            5.,
        ),
        (holes(), 43.),
    ] {
        let n = normalized(raw);
        let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
        let a = s
            .triangulate_planar(FaceId(0), PlanarOptions::default())
            .unwrap();
        let b = s
            .triangulate_planar(FaceId(0), PlanarOptions::default())
            .unwrap();
        assert_eq!(a.triangles(), b.triangles());
        assert_eq!(
            a.triangles().len(),
            a.vertices().len() + 2 * (a.boundaries().len() - 1) - 2
        );
        check(&a, &n, area);
    }
}
#[test]
fn planar_preserves_collinear_vertices_and_shared_boundary_positions() {
    let n = normalized(polygons(
        &[
            [0., 0., 0.],
            [0.25, 0., 0.],
            [0.5, 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
        ],
        &[vec![0, 1, 2, 3, 4, 5]],
    ));
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    let m = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    assert_eq!(m.vertices().len(), 6);
    check(&m, &n, 1.);
    let n = normalized(adjacent());
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    let a = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    let b = s
        .triangulate_planar(FaceId(1), PlanarOptions::default())
        .unwrap();
    check(&a, &n, 1.);
    check(&b, &n, 1.);
    let shared = &s.edge(EdgeId(1)).unwrap().samples()[0].position;
    assert!(
        a.vertices()
            .iter()
            .any(|v| std::ptr::eq(&v.edge_sample.position, shared))
    );
    // Coedge joins use the next edge's start. Canonical vertex values are exact
    // even when the endpoint belongs to a different edge's sample allocation.
    for p in s.edge(EdgeId(1)).unwrap().samples() {
        assert!(
            a.vertices()
                .iter()
                .any(|v| v.edge_sample.position == p.position)
        );
        assert!(
            b.vertices()
                .iter()
                .any(|v| v.edge_sample.position == p.position)
        );
    }
}
#[test]
fn planar_circle_uses_cached_samples_and_reverses_face_winding() {
    let mut raw = disk();
    raw.faces[0].orientation = Orientation::Reversed;
    let n = normalized(raw);
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    let m = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    let count = s.edges()[0].samples().len() - 1;
    assert_eq!(m.vertices().len(), count);
    assert_eq!(m.triangles().len(), count - 2);
    assert_eq!(m.normal(), [0., 0., -1.]);
    for (v, sample) in m.vertices().iter().zip(s.edges()[0].samples()) {
        assert!(std::ptr::eq(v.edge_sample, sample));
    }
    check(
        &m,
        &n,
        count as f64 * (std::f64::consts::TAU / count as f64).sin() * 0.5,
    );
}
#[test]
fn planar_limits_and_unsupported_surfaces_are_explicit() {
    let n = normalized(square());
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    for options in [
        PlanarOptions {
            max_vertices: 3,
            ..PlanarOptions::default()
        },
        PlanarOptions {
            max_triangles: 1,
            ..PlanarOptions::default()
        },
        PlanarOptions {
            max_work: 0,
            ..PlanarOptions::default()
        },
    ] {
        assert!(s.triangulate_planar(FaceId(0), options).is_err());
    }
    assert_eq!(
        s.triangulate_planar(
            FaceId(0),
            PlanarOptions {
                max_work: 16,
                ..PlanarOptions::default()
            }
        )
        .unwrap_err(),
        PlanarError::ResourceLimit
    );
    assert_eq!(
        s.triangulate_planar(FaceId(usize::MAX), PlanarOptions::default())
            .unwrap_err(),
        PlanarError::InvalidFace
    );
    assert_eq!(
        s.triangulate_planar(
            FaceId(0),
            PlanarOptions {
                max_vertices: usize::MAX,
                ..PlanarOptions::default()
            }
        )
        .unwrap_err(),
        PlanarError::InvalidOptions
    );
    let n = normalized(cylinder_seam());
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    assert_eq!(
        s.triangulate_planar(FaceId(0), PlanarOptions::default())
            .unwrap_err(),
        PlanarError::UnsupportedSurface
    );
}
#[test]
fn planar_actual_boundary_resolution_is_revalidated() {
    let mut raw = disk();
    let h = polygons(
        &[
            [0.65, 0.65, 0.],
            [0.65, 0.7, 0.],
            [0.7, 0.7, 0.],
            [0.7, 0.65, 0.],
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
    tessstep_trim::reconstruct(&n, FaceId(0), tessstep_trim::Options::default()).unwrap();
    let coarse =
        TessellationTolerance::new(Length::metres(0.4).unwrap(), Angle::radians(1.).unwrap())
            .unwrap();
    let s = sample_edges(&n, coarse, SamplingLimits::default()).unwrap();
    assert!(matches!(
        s.triangulate_planar(FaceId(0), PlanarOptions::default()),
        Err(PlanarError::InvalidPolygon(_))
    ));
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    let m = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    let count = s.edges()[0].samples().len() - 1;
    check(
        &m,
        &n,
        count as f64 * (std::f64::consts::TAU / count as f64).sin() * 0.5 - 0.0025,
    );
}
#[test]
fn planar_shared_interior_knot_samples_keep_identity() {
    use tessstep_curves::spline::{NurbsCurve, SplineLimits};
    use tessstep_math::Point;
    let mut raw = adjacent();
    raw.edges[1].curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            1,
            &[0., 0., 0.5, 1., 1.],
            &[[1., 0., 0.], [1., 0.5, 0.], [1., 1., 0.]].map(|p| Point::new(p).unwrap()),
            &[1.; 3],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    let n = normalized(raw);
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    let sample = &s.edge(EdgeId(1)).unwrap().samples()[1];
    for face in [FaceId(0), FaceId(1)] {
        let mesh = s
            .triangulate_planar(face, PlanarOptions::default())
            .unwrap();
        check(&mesh, &n, 1.);
        assert!(
            mesh.vertices()
                .iter()
                .any(|v| std::ptr::eq(v.edge_sample, sample))
        );
    }
}
#[test]
fn planar_tilted_plane_uses_surface_normal() {
    use tessstep_curves::{Curve, PlaneFrame};
    use tessstep_math::{NumericalTolerance, Point, Vector};
    let mut raw = square();
    for v in &mut raw.vertices {
        let [x, y, z] = v.position.coordinates();
        v.position = Point::new([x + 3., z + 4., y + 5.]).unwrap();
    }
    for e in &mut raw.edges {
        let a = raw.vertices[e.vertices[0].0].position;
        let b = raw.vertices[e.vertices[1].0].position;
        e.curve = CurveGeometry::Analytic(Curve::line(a, b.difference(a).unwrap()).unwrap());
    }
    raw.faces[0].surface = SurfaceGeometry::Analytic(tessstep_surfaces::Surface::plane(
        PlaneFrame::new(
            Point::new([3., 4., 5.]).unwrap(),
            Vector::new([1., 0., 0.]).unwrap(),
            Vector::new([0., 0., 1.]).unwrap(),
            NumericalTolerance::default(),
        )
        .unwrap(),
    ));
    let n = normalized(raw);
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    let mesh = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    assert_eq!(mesh.normal(), [0., -1., 0.]);
    check(&mesh, &n, 1.);
}
#[test]
fn planar_varied_concave_polygons_and_hole_orders() {
    let mut state = 71823u64;
    for nvertices in 5..45 {
        let mut points = Vec::new();
        for i in 0..nvertices {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let radius = 3. + (state >> 32) as f64 / u32::MAX as f64 * 2.;
            let a = std::f64::consts::TAU * i as f64 / nvertices as f64;
            points.push([radius * a.cos(), radius * a.sin(), 0.]);
        }
        points.extend([
            [-0.5, -0.5, 0.],
            [-0.5, 0.5, 0.],
            [0.5, 0.5, 0.],
            [0.5, -0.5, 0.],
        ]);
        let raw = with_holes(
            &points,
            &[
                (0..nvertices).collect(),
                (nvertices..nvertices + 4).collect(),
            ],
        );
        let area = (0..nvertices)
            .map(|i| {
                let a = points[i];
                let b = points[(i + 1) % nvertices];
                (a[0] * b[1] - a[1] * b[0]) * 0.5
            })
            .sum::<f64>()
            - 1.;
        let n = normalized(raw);
        let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
        let m = s
            .triangulate_planar(FaceId(0), PlanarOptions::default())
            .unwrap();
        check(&m, &n, area);
    }
    let mut raw = holes();
    raw.faces[0].holes.reverse();
    let n = normalized(raw);
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    check(
        &s.triangulate_planar(FaceId(0), PlanarOptions::default())
            .unwrap(),
        &n,
        43.,
    );
}
#[test]
fn planar_annulus_preserves_both_sampled_circular_boundaries() {
    use tessstep_curves::{Curve, PlaneFrame};
    use tessstep_math::{NumericalTolerance, Point, Vector};
    let mut raw = disk();
    raw.vertices.push(Vertex {
        position: Point::new([0.4, 0., 0.]).unwrap(),
    });
    let frame3 = PlaneFrame::new(
        Point::new([0.; 3]).unwrap(),
        Vector::new([1., 0., 0.]).unwrap(),
        Vector::new([0., 1., 0.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    let frame2 = PlaneFrame::new(
        Point::new([0.; 2]).unwrap(),
        Vector::new([1., 0.]).unwrap(),
        Vector::new([0., 1.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    raw.edges.push(Edge {
        vertices: [VertexId(1); 2],
        curve: CurveGeometry::Analytic(
            Curve::circle(frame3, Length::metres(0.4).unwrap()).unwrap(),
        ),
        range: [0., std::f64::consts::TAU],
    });
    raw.coedges.push(Coedge {
        edge: EdgeId(1),
        orientation: Orientation::Reversed,
        pcurve: Some(Pcurve {
            curve: CurveGeometry::Analytic(
                Curve::circle(frame2, Length::metres(0.4).unwrap()).unwrap(),
            ),
            range: [0., std::f64::consts::TAU],
        }),
    });
    raw.wires.push(Wire {
        coedges: vec![CoedgeId(1)],
    });
    raw.faces[0].holes.push(WireId(1));
    let n = normalized(raw);
    let s = sample_edges(&n, sampling(), SamplingLimits::default()).unwrap();
    let m = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    let areas = [1., 0.4]
        .into_iter()
        .zip(s.edges())
        .map(|(radius, edge)| {
            let count = edge.samples().len() - 1;
            radius * radius * count as f64 * (std::f64::consts::TAU / count as f64).sin() * 0.5
        })
        .collect::<Vec<_>>();
    check(&m, &n, areas[0] - areas[1]);
}
