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
    r.solids.push(Solid { shell: ShellId(0) });
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
