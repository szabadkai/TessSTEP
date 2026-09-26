#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use support::*;
use tessstep_topology::*;
use tessstep_trim::*;
fn trimmed(r: RawBrep) -> Result<TrimmedFace, Error> {
    let n = r
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    reconstruct(&n, FaceId(0), Options::default())
}
#[test]
fn uv_trimming_reconstructs_outer_holes_and_classifies() {
    let mut r = polygons(
        &[
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 4., 0.],
            [0., 4., 0.],
            [1., 1., 0.],
            [1., 2., 0.],
            [2., 2., 0.],
            [2., 1., 0.],
        ],
        &[vec![0, 1, 2, 3], vec![4, 5, 6, 7]],
    );
    r.faces.pop();
    r.faces[0].holes.push(WireId(1));
    let t = trimmed(r).unwrap();
    assert_eq!(t.outer().signed_area(), 16.);
    assert_eq!(t.holes()[0].signed_area(), -1.);
    assert_eq!(t.classify([0.5, 0.5]).unwrap(), Containment::Inside);
    assert_eq!(t.classify([1.5, 1.5]).unwrap(), Containment::Outside);
    assert_eq!(t.classify([1., 1.5]).unwrap(), Containment::Boundary);
    assert_eq!(t.classify([5., 5.]).unwrap(), Containment::Outside);
    assert!(t.classify([f64::NAN, 0.]).is_err());
}
#[test]
fn uv_trimming_preserves_cylinder_seam_uses_and_curved_boundary() {
    let t = trimmed(cylinder_seam()).unwrap();
    let points = t.outer().polygon();
    assert_eq!(points.len(), 4);
    assert!((t.outer().signed_area() - std::f64::consts::TAU).abs() < 1e-12);
    assert_eq!(t.outer().uses()[1].coedge, CoedgeId(1));
    assert_eq!(t.outer().uses()[3].coedge, CoedgeId(3));
    let d = trimmed(disk()).unwrap();
    assert!((d.outer().signed_area() - std::f64::consts::PI).abs() < 1e-4);
    assert!(d.outer().polygon().len() > 100);
}
#[test]
fn uv_trimming_diagnoses_missing_mismatched_and_invalid_loops() {
    let mut r = square();
    r.coedges[0].pcurve = None;
    assert_eq!(trimmed(r).unwrap_err().kind, ErrorKind::MissingPcurve);
    let mut r = square();
    r.coedges[0].pcurve.as_mut().unwrap().range = [0., 0.5];
    assert_eq!(
        trimmed(r).unwrap_err().kind,
        ErrorKind::CurveSurfaceMismatch
    );
    let r = polygons(
        &[[0., 0., 0.], [1., 1., 0.], [0., 1., 0.], [1., 0., 0.]],
        &[vec![0, 1, 2, 3]],
    );
    assert_eq!(trimmed(r).unwrap_err().kind, ErrorKind::SelfIntersection);
    let r = polygons(
        &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        &[vec![3, 2, 1, 0]],
    );
    assert_eq!(trimmed(r).unwrap_err().kind, ErrorKind::WrongOrientation);
    let mut r = polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [2., 2., 0.],
            [2., 3., 0.],
            [3., 3., 0.],
            [3., 2., 0.],
        ],
        &[vec![0, 1, 2, 3], vec![4, 5, 6, 7]],
    );
    r.faces.pop();
    r.faces[0].holes.push(WireId(1));
    assert_eq!(trimmed(r).unwrap_err().kind, ErrorKind::HoleOutside);
}
#[test]
fn uv_trimming_limits_and_degenerate_boundaries_fail_explicitly() {
    let n = disk()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    for o in [
        Options {
            max_samples: 2,
            ..Options::default()
        },
        Options {
            max_work: 1,
            ..Options::default()
        },
    ] {
        assert_eq!(
            reconstruct(&n, FaceId(0), o).unwrap_err().kind,
            ErrorKind::Limit
        );
    }
    assert_eq!(
        reconstruct(
            &n,
            FaceId(0),
            Options {
                max_depth: 0,
                ..Options::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::UnresolvedTolerance
    );
    assert_eq!(
        reconstruct(
            &n,
            FaceId(0),
            Options {
                uv_tolerance: f64::NAN,
                ..Options::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::InvalidOptions
    );
    let r = polygons(
        &[[0., 0., 0.], [1., 0., 0.], [2., 0., 0.]],
        &[vec![0, 1, 2]],
    );
    assert!(trimmed(r).is_err());
}
#[test]
fn uv_trimming_aligns_wrapped_pcurve_charts_without_projection() {
    use tessstep_curves::Curve;
    use tessstep_math::{Point, Vector};
    let mut r = cylinder_seam();
    // The right seam's pcurve is supplied at u=0, equivalent to u=2pi.
    r.coedges[1].pcurve.as_mut().unwrap().curve = CurveGeometry::Analytic(
        Curve::line(
            Point::new([0., 0.]).unwrap(),
            Vector::new([0., 1.]).unwrap(),
        )
        .unwrap(),
    );
    let t = trimmed(r).unwrap();
    assert_eq!(
        t.outer().uses()[1].period_shift,
        [std::f64::consts::TAU, 0.]
    );
}
#[test]
fn uv_trimming_samples_nurbs_pcurves_and_rejects_singular_surfaces() {
    use tessstep_curves::spline::{NurbsCurve, SplineLimits};
    use tessstep_math::Point;
    let mut r = square();
    let knots = [0., 0., 0., 1., 1., 1.];
    r.edges[0].curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            2,
            &knots,
            &[
                Point::new([0., 0., 0.]).unwrap(),
                Point::new([0.5, -0.5, 0.]).unwrap(),
                Point::new([1., 0., 0.]).unwrap(),
            ],
            &[1.; 3],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    r.coedges[0].pcurve.as_mut().unwrap().curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            2,
            &knots,
            &[
                Point::new([0., 0.]).unwrap(),
                Point::new([0.5, -0.5]).unwrap(),
                Point::new([1., 0.]).unwrap(),
            ],
            &[1.; 3],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    let t = trimmed(r).unwrap();
    assert!((t.outer().signed_area() - 7. / 6.).abs() < 1e-5);
    let mut r = square();
    r.faces[0].surface = SurfaceGeometry::Nurbs(
        tessstep_surfaces::NurbsSurface::new(
            [1, 1],
            [&[0., 0., 1., 1.]; 2],
            [2, 2],
            &[Point::new([0., 0., 0.]).unwrap(); 4],
            &[1.; 4],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(trimmed(r).unwrap_err().kind, ErrorKind::SingularSurface);
}
#[test]
fn uv_trimming_rejects_noncontractible_periodic_loops() {
    let mut r = cylinder_seam();
    r.edges.truncate(1);
    r.vertices.truncate(1);
    r.coedges.truncate(1);
    r.wires[0].coedges.truncate(1);
    assert_eq!(trimmed(r).unwrap_err().kind, ErrorKind::NonContractibleLoop);
}
#[test]
fn uv_trimming_uses_one_sided_knots_and_decreasing_pcurve_parameters() {
    use tessstep_curves::{
        Curve,
        spline::{NurbsCurve, SplineLimits},
    };
    use tessstep_math::{Point, Vector};
    let mut r = square();
    r.coedges[0].pcurve = Some(Pcurve {
        curve: CurveGeometry::Analytic(
            Curve::line(
                Point::new([1., 0.]).unwrap(),
                Vector::new([-1., 0.]).unwrap(),
            )
            .unwrap(),
        ),
        range: [1., 0.],
    });
    assert_eq!(trimmed(r).unwrap().outer().signed_area(), 1.);
    let mut r = square();
    let knots = [0., 0., 0.5, 0.5, 1., 1.];
    r.edges[0].curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            1,
            &knots,
            &[
                Point::new([0., 0., 0.]).unwrap(),
                Point::new([0.4, 0., 0.]).unwrap(),
                Point::new([0.6, 0., 0.]).unwrap(),
                Point::new([1., 0., 0.]).unwrap(),
            ],
            &[1.; 4],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    r.coedges[0].pcurve.as_mut().unwrap().curve = CurveGeometry::Nurbs(
        NurbsCurve::new(
            1,
            &knots,
            &[
                Point::new([0., 0.]).unwrap(),
                Point::new([0.4, 0.]).unwrap(),
                Point::new([0.6, 0.]).unwrap(),
                Point::new([1., 0.]).unwrap(),
            ],
            &[1.; 4],
            SplineLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(trimmed(r).unwrap_err().kind, ErrorKind::DiscontinuousPcurve);
}
#[test]
fn uv_polygon_validation_checks_actual_resolution_and_limits() {
    let outer = vec![[0., 0.], [4., 0.], [4., 4.], [0., 4.]];
    let hole = vec![[1., 1.], [1., 2.], [2., 2.], [2., 1.]];
    validate_polygons(&outer, std::slice::from_ref(&hole), 1e-6, 1000).unwrap();
    assert_eq!(
        validate_polygons(&outer, &[], 0., 1000).unwrap_err().kind,
        ErrorKind::InvalidOptions
    );
    assert_eq!(
        validate_polygons(&outer, &[], 1e-6, 0).unwrap_err().kind,
        ErrorKind::Limit
    );
    assert_eq!(
        validate_polygons(&[], &[], 1e-6, 1000).unwrap_err().kind,
        ErrorKind::DegenerateLoop
    );
    let mut reversed = outer.clone();
    reversed.reverse();
    assert_eq!(
        validate_polygons(&reversed, &[], 1e-6, 1000)
            .unwrap_err()
            .kind,
        ErrorKind::WrongOrientation
    );
    assert_eq!(
        validate_polygons(&outer, &[hole.clone(), hole], 1e-6, 1000)
            .unwrap_err()
            .kind,
        ErrorKind::IntersectingLoops
    );
    let outside = vec![[5., 1.], [5., 2.], [6., 2.], [6., 1.]];
    assert_eq!(
        validate_polygons(&outer, &[outside], 1e-6, 1000)
            .unwrap_err()
            .kind,
        ErrorKind::HoleOutside
    );
    let mut bad = outer;
    bad[0][0] = f64::NAN;
    assert_eq!(
        validate_polygons(&bad, &[], 1e-6, 1000).unwrap_err().kind,
        ErrorKind::Geometry
    );
}
#[test]
fn uv_trimming_places_holes_in_the_outer_chart_by_whole_periods() {
    use std::f64::consts::{PI, TAU};
    use tessstep_curves::{Curve, PlaneFrame};
    use tessstep_math::{Length, NumericalTolerance, Point, Vector};
    let mut r = cylinder_seam();
    // A UV square hole around u=pi, whose pcurves are supplied one period lower.
    let (u0, u1, z0, z1) = (PI - 0.2, PI + 0.2, 0.3, 0.7);
    let at = |u: f64, z: f64| Point::new([u.cos(), u.sin(), z]).unwrap();
    let first = r.vertices.len();
    for (u, z) in [(u0, z0), (u0, z1), (u1, z1), (u1, z0)] {
        r.vertices.push(Vertex { position: at(u, z) });
    }
    let v = |i: usize| VertexId(first + i);
    let circle = |z: f64| {
        let frame = PlaneFrame::new(
            Point::new([0., 0., z]).unwrap(),
            Vector::new([1., 0., 0.]).unwrap(),
            Vector::new([0., 1., 0.]).unwrap(),
            NumericalTolerance::default(),
        )
        .unwrap();
        CurveGeometry::Analytic(Curve::circle(frame, Length::metres(1.).unwrap()).unwrap())
    };
    let line = |p: Point<tessstep_math::ModelSpace, 3>| {
        CurveGeometry::Analytic(Curve::line(p, Vector::new([0., 0., 1.]).unwrap()).unwrap())
    };
    let e = r.edges.len();
    r.edges.push(Edge {
        vertices: [v(0), v(3)],
        curve: circle(z0),
        range: [u0, u1],
    });
    r.edges.push(Edge {
        vertices: [v(1), v(2)],
        curve: circle(z1),
        range: [u0, u1],
    });
    r.edges.push(Edge {
        vertices: [v(0), v(1)],
        curve: line(at(u0, z0)),
        range: [0., z1 - z0],
    });
    r.edges.push(Edge {
        vertices: [v(3), v(2)],
        curve: line(at(u1, z0)),
        range: [0., z1 - z0],
    });
    let pcurve = |origin: [f64; 2], tangent: [f64; 2], range: [f64; 2]| {
        Some(Pcurve {
            curve: CurveGeometry::Analytic(
                Curve::line(Point::new(origin).unwrap(), Vector::new(tangent).unwrap()).unwrap(),
            ),
            range,
        })
    };
    let c = r.coedges.len();
    // Clockwise in UV: up the left side, along the top, down the right, back along the bottom.
    for (edge, orientation, pc) in [
        (
            e + 2,
            Orientation::Forward,
            pcurve([u0 - TAU, z0], [0., 1.], [0., z1 - z0]),
        ),
        (
            e + 1,
            Orientation::Forward,
            pcurve([-TAU, z1], [1., 0.], [u0, u1]),
        ),
        (
            e + 3,
            Orientation::Reversed,
            pcurve([u1 - TAU, z0], [0., 1.], [0., z1 - z0]),
        ),
        (
            e,
            Orientation::Reversed,
            pcurve([-TAU, z0], [1., 0.], [u0, u1]),
        ),
    ] {
        r.coedges.push(Coedge {
            edge: EdgeId(edge),
            orientation,
            pcurve: pc,
        });
    }
    r.wires.push(Wire {
        coedges: (c..c + 4).map(CoedgeId).collect(),
    });
    r.faces[0].holes.push(WireId(r.wires.len() - 1));
    let t = trimmed(r).unwrap();
    assert_eq!(t.holes().len(), 1);
    for u in t.holes()[0].uses() {
        assert_eq!(u.period_shift, [TAU, 0.]);
    }
    assert!((t.holes()[0].signed_area() + 0.4 * 0.4).abs() < 1e-9);
    assert_eq!(t.classify([PI, 0.5]).unwrap(), Containment::Outside);
    assert_eq!(t.classify([PI + 1., 0.5]).unwrap(), Containment::Inside);
}
