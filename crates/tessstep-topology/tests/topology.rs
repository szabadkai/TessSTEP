mod support;
use support::*;
use tessstep_topology::*;
fn defect(r: RawBrep) -> Defect {
    r.validate(tolerance(), ValidationLimits::default())
        .unwrap_err()
        .defect
}
#[test]
fn topology_states_preserve_shared_identity_and_geometry() {
    let r = adjacent();
    let original = r.clone();
    let n = r
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    assert_eq!(n.data(), &original);
    let e = n
        .data()
        .edges
        .iter()
        .position(|e| e.vertices == [VertexId(1), VertexId(2)])
        .unwrap();
    let uses = n.edge_uses(EdgeId(e)).unwrap();
    assert_eq!(uses.len(), 2);
    assert_ne!(
        n.data().coedges[uses[0].0].orientation,
        n.data().coedges[uses[1].0].orientation
    );
    assert!(
        tetrahedron()
            .validate(tolerance(), ValidationLimits::default())
            .is_ok()
    );
}
#[test]
fn topology_rejects_invalid_references_ownership_and_endpoints() {
    let mut r = square();
    r.edges[0].vertices[0] = VertexId(usize::MAX);
    assert_eq!(defect(r), Defect::InvalidHandle);
    let mut r = square();
    r.wires[0].coedges.swap(1, 2);
    assert_eq!(defect(r), Defect::BrokenWire);
    let mut r = square();
    r.faces[0].holes.push(WireId(0));
    assert_eq!(defect(r), Defect::DuplicateOwnership);
    let mut r = square();
    r.edges[0].range = [0., 0.5];
    assert_eq!(defect(r), Defect::EndpointMismatch);
    let mut r = square();
    r.edges[0].range = [0., f64::NAN];
    assert_eq!(defect(r), Defect::InvalidRange);
    let mut r = square();
    r.vertices.push(r.vertices[0].clone());
    assert_eq!(defect(r), Defect::UnusedRecord);
}
#[test]
fn topology_shells_check_closure_connectivity_and_orientation() {
    let mut r = square();
    r.shells.push(Shell {
        faces: vec![FaceId(0)],
        closed: true,
    });
    assert_eq!(defect(r), Defect::OpenShell);
    let mut r = adjacent();
    r.faces[1].orientation = Orientation::Reversed;
    r.shells.push(Shell {
        faces: vec![FaceId(0), FaceId(1)],
        closed: false,
    });
    assert_eq!(defect(r), Defect::InconsistentOrientation);
    let mut r = polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [2., 0., 0.],
            [3., 0., 0.],
            [2., 1., 0.],
        ],
        &[vec![0, 1, 2], vec![3, 4, 5]],
    );
    r.shells.push(Shell {
        faces: vec![FaceId(0), FaceId(1)],
        closed: false,
    });
    assert_eq!(defect(r), Defect::DisconnectedShell);
    let mut r = adjacent();
    r.shells.push(Shell {
        faces: vec![FaceId(0), FaceId(1)],
        closed: false,
    });
    r.solids.push(Solid::new(ShellId(0)));
    assert_eq!(defect(r), Defect::OpenShell);
    let r = square();
    assert_eq!(
        r.validate(
            tolerance(),
            ValidationLimits {
                max_records: 0,
                ..ValidationLimits::default()
            }
        )
        .unwrap_err()
        .defect,
        Defect::ResourceLimit
    );
    let r = square();
    assert_eq!(
        r.validate(
            tolerance(),
            ValidationLimits {
                max_work: 2,
                ..ValidationLimits::default()
            }
        )
        .unwrap_err()
        .defect,
        Defect::ResourceLimit
    );
}
#[test]
fn topology_rejects_nonmanifold_edges_and_pinched_vertices() {
    let mut r = polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., -1., 0.],
            [1., 1., 0.],
        ],
        &[vec![0, 1, 2], vec![1, 0, 3], vec![0, 1, 4]],
    );
    r.shells.push(Shell {
        faces: (0..3).map(FaceId).collect(),
        closed: false,
    });
    assert_eq!(defect(r), Defect::NonManifoldEdge);
    // A connected strip whose ends meet only at vertex 0 has a disconnected link there.
    let mut r = polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [2., 1., 0.],
            [2., 0., 0.],
        ],
        &[vec![0, 1, 2], vec![2, 1, 3], vec![3, 1, 4], vec![3, 4, 0]],
    );
    r.shells.push(Shell {
        faces: (0..4).map(FaceId).collect(),
        closed: false,
    });
    assert_eq!(defect(r), Defect::NonManifoldVertex);
}
#[test]
fn topology_closed_edges_keep_two_endpoint_incidences() {
    let n = closed_cylinder()
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    assert_eq!(n.data().vertices.len(), 2);
    assert_eq!(n.edge_uses(EdgeId(0)).unwrap().len(), 2);
    assert_eq!(n.edge_uses(EdgeId(2)).unwrap().len(), 2);
}
#[test]
fn topology_collapsed_edges_close_pole_and_apex_charts() {
    for (raw, collapsed, pole) in [(hemisphere(), 2, 1), (apex_cone(), 0, 0)] {
        let n = raw
            .clone()
            .validate(tolerance(), ValidationLimits::default())
            .unwrap()
            .normalize();
        // The collapsed edge has one use in a closed shell and its vertex is singular.
        assert!(n.is_collapsed(EdgeId(collapsed)));
        assert_eq!(n.edge_uses(EdgeId(collapsed)).unwrap().len(), 1);
        assert!(n.is_singular_vertex(VertexId(pole)));
        assert!(!n.is_singular_vertex(VertexId(1 - pole)));
        assert!((0..3).filter(|&e| n.is_collapsed(EdgeId(e))).count() == 1);
        // A closed edge that leaves its vertex is not collapsed: its single use opens
        // the shell.
        let mut open = raw;
        let at = open.vertices[pole].position.coordinates();
        open.edges[collapsed].curve = CurveGeometry::Analytic(
            tessstep_curves::Curve::circle(
                tessstep_curves::PlaneFrame::new(
                    tessstep_math::Point::new([at[0] - 1e-3, at[1], at[2]]).unwrap(),
                    tessstep_math::Vector::new([1., 0., 0.]).unwrap(),
                    tessstep_math::Vector::new([0., 1., 0.]).unwrap(),
                    tessstep_math::NumericalTolerance::default(),
                )
                .unwrap(),
                tessstep_math::Length::metres(1e-3).unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(defect(open), Defect::OpenShell);
    }
}
#[test]
fn topology_cavity_shells_are_closed_and_uniquely_owned() {
    // Two disjoint tetrahedra: the second shell is the first solid's cavity.
    let two = |closed: bool| {
        let mut r = polygons(
            &[
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [0., 0., 1.],
                [5., 0., 0.],
                [6., 0., 0.],
                [5., 1., 0.],
                [5., 0., 1.],
            ],
            &[
                vec![0, 2, 1],
                vec![0, 1, 3],
                vec![1, 2, 3],
                vec![2, 0, 3],
                vec![4, 6, 5],
                vec![4, 5, 7],
                vec![5, 6, 7],
                vec![6, 4, 7],
            ],
        );
        r.shells.push(Shell {
            faces: (0..4).map(FaceId).collect(),
            closed: true,
        });
        r.shells.push(Shell {
            faces: (4..8).map(FaceId).collect(),
            closed,
        });
        r.solids.push(Solid {
            shell: ShellId(0),
            voids: vec![ShellId(1)],
        });
        r
    };
    let n = two(true)
        .validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize();
    assert_eq!(n.data().solids[0].voids, [ShellId(1)]);
    assert_eq!(defect(two(false)), Defect::OpenShell);
    let mut shared = tetrahedron();
    shared.solids[0].voids.push(ShellId(0));
    assert_eq!(defect(shared), Defect::DuplicateOwnership);
    // A cavity shell may not share an edge with the outer shell: the second
    // tetrahedron is the first turned half a revolution about their common x edge.
    let mut touching = polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [0., -1., 0.],
            [0., 0., -1.],
        ],
        &[
            vec![0, 2, 1],
            vec![0, 1, 3],
            vec![1, 2, 3],
            vec![2, 0, 3],
            vec![0, 4, 1],
            vec![0, 1, 5],
            vec![1, 4, 5],
            vec![4, 0, 5],
        ],
    );
    for (faces, closed) in [(0..4, true), (4..8, true)] {
        touching.shells.push(Shell {
            faces: faces.map(FaceId).collect(),
            closed,
        });
    }
    touching.solids.push(Solid {
        shell: ShellId(0),
        voids: vec![ShellId(1)],
    });
    assert_eq!(defect(touching), Defect::NonManifoldEdge);
}
