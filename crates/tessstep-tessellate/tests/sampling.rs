#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use support::*;
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_tessellate::*;
use tessstep_topology::*;
fn tol(chord: f64, angle: f64) -> TessellationTolerance {
    TessellationTolerance::new(
        Length::metres(chord).unwrap(),
        Angle::radians(angle).unwrap(),
    )
    .unwrap()
}
#[test]
fn shared_edges_are_sampled_once_with_zero_copy_reversed_views() {
    let n = adjacent()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    let s = sample_edges(&n, tol(1e-3, 0.1), SamplingLimits::default()).unwrap();
    assert_eq!(s.edges().len(), 7);
    for e in s.edges() {
        assert_eq!(e.samples().len(), 2);
    }
    let uses = n.edge_uses(EdgeId(1)).unwrap();
    assert_eq!(uses.len(), 2);
    let a: Vec<_> = s.coedge(uses[0]).unwrap().iter().collect();
    let b: Vec<_> = s.coedge(uses[1]).unwrap().iter().rev().collect();
    assert!(a.iter().zip(b).all(|(x, y)| std::ptr::eq(*x, y)));
    assert!(s.coedge(CoedgeId(usize::MAX)).is_none());
}
#[test]
fn shared_edge_circle_obeys_independent_sagitta_and_angle_checks() {
    let n = disk()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    let s = sample_edges(&n, tol(0.001, 0.1), SamplingLimits::default()).unwrap();
    let p = s.edges()[0].samples();
    assert!(p.len() > 32);
    assert_eq!(p.first().unwrap().position, p.last().unwrap().position);
    for w in p.windows(2) {
        let delta = w[1].parameter - w[0].parameter;
        assert!(1. - (delta / 2.).cos() <= 0.001);
        assert!(delta / 2. <= 0.1);
    }
    let fine = sample_edges(&n, tol(0.0001, 0.02), SamplingLimits::default()).unwrap();
    assert!(fine.edges()[0].samples().len() > p.len());
}
#[test]
fn shared_edge_seam_has_one_storage_and_endpoint_vertex_identity() {
    let n = cylinder_seam()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    let s = sample_edges(&n, tol(0.001, 0.1), SamplingLimits::default()).unwrap();
    assert_eq!(s.edges().len(), 3);
    let a = s.coedge(CoedgeId(1)).unwrap().iter().next().unwrap();
    let b = s.coedge(CoedgeId(3)).unwrap().iter().next_back().unwrap();
    assert!(std::ptr::eq(a, b));
    for e in s.edges() {
        let v = n.data().edges[e.edge().0].vertices;
        assert_eq!(e.samples()[0].position, n.data().vertices[v[0].0].position);
        assert_eq!(
            e.samples().last().unwrap().position,
            n.data().vertices[v[1].0].position
        );
    }
}
#[test]
fn shared_edge_limits_never_publish_partial_success() {
    let n = disk()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    for limits in [
        SamplingLimits {
            max_samples: 2,
            ..SamplingLimits::default()
        },
        SamplingLimits {
            max_evaluations: 1,
            ..SamplingLimits::default()
        },
    ] {
        assert_eq!(
            sample_edges(&n, tol(1e-3, 0.1), limits).unwrap_err().kind,
            ErrorKind::ResourceLimit
        );
    }
    assert_eq!(
        sample_edges(
            &n,
            tol(1e-6, 0.01),
            SamplingLimits {
                max_depth: 0,
                ..SamplingLimits::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::UnresolvedTolerance
    );
}
#[test]
fn shared_edge_nurbs_knot_corners_and_discontinuities_are_explicit() {
    use tessstep_curves::spline::{NurbsCurve, SplineLimits};
    use tessstep_math::Point;
    let mut r = square();
    r.edges[0].curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            1,
            &[0., 0., 0.5, 1., 1.],
            &[
                Point::new([0., 0., 0.]).unwrap(),
                Point::new([0.5, 0.5, 0.]).unwrap(),
                Point::new([1., 0., 0.]).unwrap(),
            ],
            &[1.; 3],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    let n = r
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    let s = sample_edges(&n, tol(1e-5, 0.01), SamplingLimits::default()).unwrap();
    assert_eq!(s.edges()[0].samples().len(), 3);
    assert_eq!(s.edges()[0].samples()[1].parameter, 0.5);
    let mut r = square();
    r.edges[0].curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            1,
            &[0., 0., 0.5, 0.5, 1., 1.],
            &[
                Point::new([0., 0., 0.]).unwrap(),
                Point::new([0.3, 0., 0.]).unwrap(),
                Point::new([0.7, 0., 0.]).unwrap(),
                Point::new([1., 0., 0.]).unwrap(),
            ],
            &[1.; 4],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    let n = r
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    assert_eq!(
        sample_edges(&n, tol(1e-5, 0.1), SamplingLimits::default())
            .unwrap_err()
            .kind,
        ErrorKind::DiscontinuousCurve
    );
}
#[test]
fn shared_edge_face_boundaries_keep_identical_positions_and_distinct_seam_uv() {
    let n = cylinder_seam()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    let s = sample_edges(&n, tol(0.001, 0.1), SamplingLimits::default()).unwrap();
    let b = s
        .face_boundary(FaceId(0), tessstep_trim::Options::default(), 10000)
        .unwrap();
    let a = &b.outer[1].samples[0];
    let other = b.outer[3].samples.last().unwrap();
    assert!(std::ptr::eq(a.edge_sample, other.edge_sample));
    assert!((a.uv[0] - other.uv[0] - std::f64::consts::TAU).abs() < 1e-12);
    assert_eq!(
        s.face_boundary(FaceId(0), tessstep_trim::Options::default(), 1)
            .unwrap_err(),
        BoundaryError::ResourceLimit
    );
}
#[test]
fn shared_edge_collapsed_curves_and_endpoint_snaps_are_diagnosed() {
    use tessstep_curves::spline::{NurbsCurve, SplineLimits};
    use tessstep_math::Point;
    let mut r = disk();
    r.edges[0].range = [0., 1.];
    r.edges[0].curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            1,
            &[0., 0., 1., 1.],
            &[r.vertices[0].position; 2],
            &[1.; 2],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    let n = r
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    assert_eq!(
        sample_edges(&n, tol(1e-3, 0.1), SamplingLimits::default())
            .unwrap_err()
            .kind,
        ErrorKind::SingularTangent
    );
    let mut r = square();
    r.vertices[0].position = Point::new([1e-9, 0., 0.]).unwrap();
    let n = r
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    assert_eq!(
        sample_edges(&n, tol(1e-10, 0.1), SamplingLimits::default())
            .unwrap_err()
            .kind,
        ErrorKind::EndpointTolerance
    );
}
#[test]
fn shared_edge_rational_arc_has_dense_independent_error_and_determinism_checks() {
    use tessstep_curves::spline::{NurbsCurve, SplineLimits};
    use tessstep_math::Point;
    let mut r = polygons(
        &[[1., 0., 0.], [0., 1., 0.], [0., 0., 0.]],
        &[vec![0, 1, 2]],
    );
    let curve = NurbsCurve::new(
        2,
        &[0., 0., 0., 1., 1., 1.],
        &[
            Point::new([1., 0., 0.]).unwrap(),
            Point::new([1., 1., 0.]).unwrap(),
            Point::new([0., 1., 0.]).unwrap(),
        ],
        &[1., std::f64::consts::FRAC_1_SQRT_2, 1.],
        SplineLimits::default(),
    )
    .unwrap();
    r.edges[0].curve = CurveGeometry::Nurbs(curve.clone());
    let n = r
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    let a = sample_edges(&n, tol(1e-4, 0.03), SamplingLimits::default()).unwrap();
    let b = sample_edges(&n, tol(1e-4, 0.03), SamplingLimits::default()).unwrap();
    assert_eq!(a.edges()[0].samples(), b.edges()[0].samples());
    for pair in a.edges()[0].samples().windows(2) {
        let x = pair[0].position.coordinates();
        let y = pair[1].position.coordinates();
        let chord_angle = (x[0] * y[0] + x[1] * y[1]).clamp(-1., 1.).acos();
        assert!(1. - (chord_angle / 2.).cos() <= 1e-4 + 1e-14);
        for i in 0..=20 {
            let u = pair[0].parameter + (pair[1].parameter - pair[0].parameter) * i as f64 / 20.;
            let p = curve.evaluate(u).unwrap().position.coordinates();
            let distance = ((y[0] - x[0]) * (x[1] - p[1]) - (x[0] - p[0]) * (y[1] - x[1])).abs()
                / (y[0] - x[0]).hypot(y[1] - x[1]);
            assert!(distance <= 1e-4 + 1e-14);
        }
    }
}
