mod support;
use support::*;
use tessstep_topology::*;

const MODEL: f64 = 1e-8;
const SEWING: f64 = 1e-7;
fn defect(r: RawBrep) -> Defect {
    r.validate(tolerance(), ValidationLimits::default())
        .unwrap_err()
        .defect
}
fn closed(mut r: RawBrep) -> RawBrep {
    r.shells.push(Shell {
        faces: (0..r.faces.len()).map(FaceId).collect(),
        closed: true,
    });
    r.solids.push(Solid::new(ShellId(0)));
    r
}
const TETRA: [[f64; 3]; 4] = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
const RINGS: [[usize; 3]; 4] = [[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]];
/// A tetrahedron whose faces use their own vertices, each corner moved by `offset`
/// per face, so no face shares an edge or vertex.
fn unsewn(offset: impl Fn(usize) -> [f64; 3]) -> RawBrep {
    let mut points = Vec::new();
    let mut loops = Vec::new();
    for (f, ring) in RINGS.iter().enumerate() {
        let d = offset(f);
        loops.push((0..3).map(|k| points.len() + k).collect());
        for &c in ring {
            points.push([TETRA[c][0] + d[0], TETRA[c][1] + d[1], TETRA[c][2] + d[2]]);
        }
    }
    closed(polygons(&points, &loops))
}
fn tetra() -> RawBrep {
    closed(polygons(
        &TETRA,
        &RINGS.iter().map(|r| r.to_vec()).collect::<Vec<_>>(),
    ))
}

#[test]
fn repair_unsewn_closed_shells_are_classified_apart_from_open_shells() {
    assert_eq!(defect(unsewn(|_| [0.; 3])), Defect::UnsewnShell);
    // A missing face leaves an open shell whose other faces share edges.
    let open = polygons(
        &TETRA,
        &RINGS[..3].iter().map(|r| r.to_vec()).collect::<Vec<_>>(),
    );
    assert_eq!(defect(closed(open)), Defect::OpenShell);
    // One face alone is never unsewn.
    assert_eq!(defect(closed(square())), Defect::OpenShell);
}

#[test]
fn repair_sews_coincident_boundaries_into_a_valid_closed_shell() {
    let raw = unsewn(|_| [0.; 3]);
    let plan = plan_sewing(&raw, MODEL, SEWING, ValidationLimits::default()).unwrap();
    assert_eq!(
        (
            plan.merged_vertices,
            plan.merged_edges,
            plan.disconnected_shells
        ),
        (8, 6, 1)
    );
    assert_eq!(plan.max_distance, 0.);
    let sewn = plan.apply(raw);
    assert_eq!((sewn.vertices.len(), sewn.edges.len()), (4, 6));
    let n = sewn
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    for e in 0..6 {
        assert_eq!(n.edge_uses(EdgeId(e)).unwrap().len(), 2);
    }
    // Coordinates are never moved: every kept vertex is one of the original corners.
    for v in &n.data().vertices {
        assert!(TETRA.contains(&v.position.coordinates()));
    }
}

#[test]
fn repair_merges_only_within_the_sewing_tolerance() {
    // Faces 0.5 and 2 sewing tolerances apart: the first sews (at the measured
    // distance), the second is left and the gap is diagnosed with its distance.
    let near = unsewn(|f| [0., 0., if f == 1 { 0.5 * SEWING } else { 0. }]);
    let plan = plan_sewing(&near, MODEL, SEWING, ValidationLimits::default()).unwrap();
    assert_eq!((plan.merged_vertices, plan.merged_edges), (8, 6));
    assert!((plan.max_distance - 0.5 * SEWING).abs() < 1e-15);
    // Validation needs the model tolerance plus the merge distance.
    let sewn = plan.apply(near);
    let grown = tessstep_math::ModelTolerance::new(
        tessstep_math::Length::metres(MODEL + plan.max_distance).unwrap(),
        tessstep_math::Angle::radians(1e-8).unwrap(),
    )
    .unwrap();
    assert_eq!(defect(sewn.clone()), Defect::EndpointMismatch);
    sewn.validate(grown, ValidationLimits::default()).unwrap();

    let far = unsewn(|f| [0., 0., if f == 1 { 2. * SEWING } else { 0. }]);
    let plan = plan_sewing(&far, MODEL, SEWING, ValidationLimits::default()).unwrap();
    assert_eq!((plan.merged_vertices, plan.merged_edges), (5, 3));
    let partial = plan.apply(far);
    let report = partial
        .clone()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap_err();
    assert_eq!(report.defect, Defect::OpenShell);
    let gap = diagnose_gap(
        &partial,
        &report,
        MODEL,
        SEWING,
        ValidationLimits::default(),
    )
    .unwrap()
    .unwrap();
    assert!((gap.needed.unwrap() - 2. * SEWING).abs() < 1e-15, "{gap}");
    assert!(gap.edge.is_some() && gap.partner.is_some());
    assert!(gap.to_string().contains("needs 2.000e-7 m"), "{gap}");
}

#[test]
fn repair_never_collapses_an_edge_and_mends_broken_wires() {
    // A 1e-7 m edge (sewing tolerance 1e-6) between two free-edge vertices stays.
    let raw = polygons(
        &[[0., 0., 0.], [1e-7, 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        &[vec![0, 1, 2, 3]],
    );
    let plan = plan_sewing(&raw, MODEL, 1e-6, ValidationLimits::default()).unwrap();
    assert!(plan.is_empty());
    // Duplicated broken-wire vertices merge; the edge between distinct ones does not.
    let mut broken = square();
    broken.vertices.push(broken.vertices[2].clone());
    broken.edges[2].vertices[0] = VertexId(4);
    broken.shells.push(Shell {
        faces: vec![FaceId(0)],
        closed: false,
    });
    assert_eq!(defect(broken.clone()), Defect::BrokenWire);
    let plan = plan_sewing(&broken, MODEL, SEWING, ValidationLimits::default()).unwrap();
    assert_eq!((plan.merged_vertices, plan.merged_edges), (1, 0));
    plan.apply(broken)
        .validate(tolerance(), ValidationLimits::default())
        .unwrap();
}

#[test]
fn repair_orients_faces_by_the_majority_of_each_group() {
    let mut r = tetra();
    r.faces[2].orientation = Orientation::Reversed;
    assert_eq!(defect(r.clone()), Defect::InconsistentOrientation);
    assert_eq!(orient_faces(&mut r, ValidationLimits::default()), Ok(1));
    assert_eq!(r.faces[2].orientation, Orientation::Forward);
    r.clone()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap();
    // A consistent shell is left alone, whichever way it faces.
    for f in &mut r.faces {
        f.orientation = Orientation::Reversed;
    }
    assert_eq!(orient_faces(&mut r, ValidationLimits::default()), Ok(0));
    // Two faces sharing two edges that demand opposite flips cannot be oriented.
    let mut knot = polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
        ],
        &[vec![0, 1, 2, 3], vec![0, 1, 4, 3, 2, 5]],
    );
    knot.shells.push(Shell {
        faces: vec![FaceId(0), FaceId(1)],
        closed: false,
    });
    let e = orient_faces(&mut knot, ValidationLimits::default()).unwrap_err();
    assert_eq!(e.report.defect, Defect::InconsistentOrientation);
    assert_eq!(e.needed, None);
}

#[test]
fn repair_budgets_and_invalid_models_are_explicit() {
    let raw = unsewn(|_| [0.; 3]);
    let e = plan_sewing(
        &raw,
        MODEL,
        SEWING,
        ValidationLimits {
            max_work: 10,
            ..ValidationLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(e.report.defect, Defect::ResourceLimit);
    // Invalid handles are left to validation.
    let mut invalid = unsewn(|_| [0.; 3]);
    invalid.coedges[0].edge = EdgeId(usize::MAX);
    assert!(
        plan_sewing(&invalid, MODEL, SEWING, ValidationLimits::default())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        orient_faces(&mut invalid, ValidationLimits::default()),
        Ok(0)
    );
}
