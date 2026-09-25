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
