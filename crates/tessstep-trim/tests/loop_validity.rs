//! Milestone 22.4: located loop checks, sub-quadratic work, named budgets and the
//! sampling rules that keep valid loops valid at the model tolerance.
#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use support::*;
use tessstep_curves::{
    Curve, PlaneFrame,
    spline::{NurbsCurve, SplineLimits},
};
use tessstep_math::{Length, NumericalTolerance, Point, Vector};
use tessstep_topology::*;
use tessstep_trim::*;

fn normalized(r: RawBrep) -> NormalizedBrep {
    r.validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize()
}
fn trimmed(r: RawBrep) -> Result<TrimmedFace, Error> {
    reconstruct(&normalized(r), FaceId(0), Options::default())
}
/// A plane face bounded by one closed circle edge of `radius` around the origin.
fn circle_face(radius: f64) -> RawBrep {
    let mut r = disk();
    let r3 = PlaneFrame::new(
        Point::new([0.; 3]).unwrap(),
        Vector::new([1., 0., 0.]).unwrap(),
        Vector::new([0., 1., 0.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    let r2 = PlaneFrame::new(
        Point::new([0.; 2]).unwrap(),
        Vector::new([1., 0.]).unwrap(),
        Vector::new([0., 1.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    let length = Length::metres(radius).unwrap();
    r.vertices[0].position = Point::new([radius, 0., 0.]).unwrap();
    r.edges[0].curve = CurveGeometry::Analytic(Curve::circle(r3, length).unwrap());
    r.coedges[0].pcurve.as_mut().unwrap().curve =
        CurveGeometry::Analytic(Curve::circle(r2, length).unwrap());
    r
}
/// The unit square with its first edge (from (0, 0) to (1, 0)) replaced by a degree-1
/// B-spline through `points` with `knots`, as 3D curve and identical pcurve.
fn square_with_polyline(points: &[[f64; 2]], knots: &[f64]) -> RawBrep {
    let mut r = square();
    let controls3: Vec<Point<_, 3>> = points
        .iter()
        .map(|p| Point::new([p[0], p[1], 0.]).unwrap())
        .collect();
    let controls2: Vec<Point<_, 2>> = points.iter().map(|&p| Point::new(p).unwrap()).collect();
    let weights = vec![1.; points.len()];
    r.edges[0].curve = CurveGeometry::Nurbs(
        NurbsCurve::new(1, knots, &controls3, &weights, SplineLimits::default()).unwrap(),
    );
    r.edges[0].range = [0., 1.];
    let pc = r.coedges[0].pcurve.as_mut().unwrap();
    pc.curve = CurveGeometry::Nurbs(
        NurbsCurve::new(1, knots, &controls2, &weights, SplineLimits::default()).unwrap(),
    );
    pc.range = [0., 1.];
    r
}

#[test]
fn polygon_failures_name_their_coedges_and_loop() {
    // A bow tie: its first and third edges cross.
    let r = polygons(
        &[[0., 0., 0.], [1., 1., 0.], [0., 1., 0.], [1., 0., 0.]],
        &[vec![0, 1, 2, 3]],
    );
    let e = trimmed(r).unwrap_err();
    assert_eq!(e.kind, ErrorKind::SelfIntersection);
    let pair = [e.coedge.unwrap(), e.other.unwrap()];
    assert!(pair == [CoedgeId(0), CoedgeId(2)] || pair == [CoedgeId(2), CoedgeId(0)]);
    assert_eq!(e.wire, Some(WireId(0)));
    // A clockwise outer loop names the loop only.
    let r = polygons(
        &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        &[vec![3, 2, 1, 0]],
    );
    let e = trimmed(r).unwrap_err();
    assert_eq!(
        (e.kind, e.coedge, e.wire),
        (ErrorKind::WrongOrientation, None, Some(WireId(0)))
    );
    // A hole outside the outer loop names the hole's wire.
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
    let e = trimmed(r).unwrap_err();
    assert_eq!((e.kind, e.wire), (ErrorKind::HoleOutside, Some(WireId(1))));
    // Exhausted budgets are named with their configured values.
    let n = normalized(circle_face(1.));
    for (options, budget) in [
        (
            Options {
                max_samples: 5,
                ..Options::default()
            },
            Budget::MaxSamples(5),
        ),
        (
            Options {
                max_work: 7,
                ..Options::default()
            },
            Budget::MaxWork(7),
        ),
    ] {
        let e = reconstruct(&n, FaceId(0), options).unwrap_err();
        assert_eq!((e.kind, e.budget), (ErrorKind::Limit, Some(budget)));
        assert!(e.to_string().contains("budget"), "{e}");
    }
}

#[test]
fn polygon_checks_charge_overlapping_pairs_not_all_pairs() {
    // 20,000 segments: all-pairs checks would charge 4e8 units.
    let n = 20_000;
    let circle: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / n as f64;
            [t.cos(), t.sin()]
        })
        .collect();
    validate_polygons(&circle, &[], 1e-9, 1_000_000).unwrap();
    // A finely sampled disk reconstructs within the default work budget.
    let fine = Options {
        uv_tolerance: 1e-8,
        ..Options::default()
    };
    let t = reconstruct(&normalized(circle_face(1.)), FaceId(0), fine).unwrap();
    assert!(t.outer().polygon().len() > 5_000);
    // 400 square holes: hole pairs are compared only where their boxes overlap.
    let mut points = vec![[0., 0., 0.], [21., 0., 0.], [21., 21., 0.], [0., 21., 0.]];
    let mut loops = vec![vec![0, 1, 2, 3]];
    for i in 0..20 {
        for j in 0..20 {
            let (x, y) = (1. + i as f64, 1. + j as f64);
            let first = points.len();
            points.extend([
                [x, y, 0.],
                [x, y + 0.5, 0.],
                [x + 0.5, y + 0.5, 0.],
                [x + 0.5, y, 0.],
            ]);
            loops.push((first..first + 4).collect());
        }
    }
    let mut r = polygons(&points, &loops);
    r.faces.truncate(1);
    r.faces[0].holes = (1..loops.len()).map(WireId).collect();
    let options = Options {
        max_work: 200_000,
        ..Options::default()
    };
    let t = reconstruct(&normalized(r), FaceId(0), options).unwrap();
    assert_eq!(t.holes().len(), 400);
}

#[test]
fn sampling_keeps_small_loops_and_drops_noise_seeds() {
    // A knot a few ulps before the edge end adds no zero-length segment.
    let r = square_with_polyline(
        &[[0., 0.], [1. - 1e-15, 0.], [1., 0.]],
        &[0., 0., 1. - 1e-15, 1., 1.],
    );
    trimmed(r).unwrap();
    // A loop much smaller than the UV tolerance (1e-5) keeps the shape of its circle.
    let radius = 1e-7;
    let t = trimmed(circle_face(radius)).unwrap();
    assert!(t.outer().polygon().len() >= 4);
    let area = std::f64::consts::PI * radius * radius;
    assert!((t.outer().signed_area() - area).abs() <= 0.4 * area);
}

#[test]
fn wobbles_within_the_model_tolerance_are_not_defects() {
    // The bottom edge doubles back by 1 nm, below the 10 nm model tolerance.
    let r = square_with_polyline(
        &[[0., 0.], [0.5, 0.], [0.5 - 1e-9, 0.], [1., 0.]],
        &[0., 0., 0.4, 0.6, 1., 1.],
    );
    trimmed(r).unwrap();
    // The same wobble across the loop's closing join: the edge first runs 1 nm back
    // up the left side, along the last segment of the loop.
    let r = square_with_polyline(&[[0., 0.], [0., 1e-9], [1., 0.]], &[0., 0., 0.5, 1., 1.]);
    trimmed(r).unwrap();
    // Doubling back by 0.1 is a self-intersection of that edge's own image.
    let r = square_with_polyline(
        &[[0., 0.], [0.5, 0.], [0.4, 0.], [1., 0.]],
        &[0., 0., 0.4, 0.6, 1., 1.],
    );
    let e = trimmed(r).unwrap_err();
    assert_eq!(e.kind, ErrorKind::SelfIntersection);
    assert_eq!((e.coedge, e.other), (Some(CoedgeId(0)), Some(CoedgeId(0))));
}

#[test]
fn other_resolutions_are_checked_and_located() {
    let n = normalized(square());
    let t = reconstruct(&n, FaceId(0), Options::default()).unwrap();
    let owners: Vec<CoedgeId> = (0..4).map(CoedgeId).collect();
    let loop_ = |points: &[[f64; 2]]| -> Vec<([f64; 2], CoedgeId)> {
        points.iter().copied().zip(owners.iter().copied()).collect()
    };
    t.validate_resolution(
        &[loop_(&[[0., 0.], [1., 0.], [1., 1.], [0., 1.]])],
        1e-12,
        1000,
    )
    .unwrap();
    // Points closer than the UV tolerance are resolved at a finer resolution.
    t.validate_resolution(
        &[loop_(&[[0., 0.], [1., 0.], [1., 1.], [1. - 1e-7, 1.]])],
        1e-12,
        1000,
    )
    .unwrap();
    // Crossing chords name both coedges and the wire.
    let e = t
        .validate_resolution(
            &[loop_(&[[0., 0.], [1., 1.], [1., 0.], [0., 1.]])],
            1e-12,
            1000,
        )
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::SelfIntersection);
    assert!(e.coedge.is_some() && e.other.is_some() && e.coedge != e.other);
    assert_eq!(e.wire, Some(n.data().faces[0].outer));
    assert_eq!(
        t.validate_resolution(&[], 1e-12, 1000).unwrap_err().kind,
        ErrorKind::InvalidFace
    );
}
