use tessstep_mesh::*;
fn tetra() -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
    (
        vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    )
}
#[test]
fn mesh_closed_incidence_volume_and_owned_storage() {
    let (p, t) = tetra();
    let m = Mesh::from_triangles(p, t, Limits::default()).unwrap();
    m.require_solid().unwrap();
    assert!(m.is_watertight());
    assert_eq!(m.statistics().components, 1);
    assert!((m.statistics().signed_volume - 1. / 6.).abs() < 1e-15);
    assert_eq!(m.data().normals.len(), 4);
    let mut reversed = m.data().triangles.clone();
    for t in &mut reversed {
        t.swap(1, 2);
    }
    assert_eq!(
        Mesh::from_triangles(m.data().positions.clone(), reversed, Default::default())
            .unwrap()
            .require_solid(),
        Err(Error::NonPositiveVolume)
    );
}
#[test]
fn mesh_rejects_bad_indices_degeneracy_duplicates_and_winding() {
    let (p, t) = tetra();
    let mut bad = t.clone();
    bad[0][0] = 99;
    assert_eq!(
        Mesh::from_triangles(p.clone(), bad, Default::default()).unwrap_err(),
        Error::InvalidIndex
    );
    let mut bad = t.clone();
    bad[0][0] = bad[0][1];
    assert_eq!(
        Mesh::from_triangles(p.clone(), bad, Default::default()).unwrap_err(),
        Error::DegenerateTriangle
    );
    let mut bad = t.clone();
    bad.push(t[0]);
    assert_eq!(
        Mesh::from_triangles(p.clone(), bad, Default::default()).unwrap_err(),
        Error::DuplicateTriangle
    );
    let mut bad = t.clone();
    bad[0].swap(1, 2);
    assert_eq!(
        Mesh::from_triangles(p.clone(), bad, Default::default()).unwrap_err(),
        Error::InconsistentWinding
    );
    let mut bad = p.clone();
    bad[0][0] = f64::NAN;
    assert!(Mesh::from_triangles(bad, t.clone(), Default::default()).is_err());
    assert_eq!(
        Mesh::from_triangles(
            p,
            t,
            Limits {
                max_work: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        Error::ResourceLimit
    );
}
#[test]
fn mesh_open_disconnected_and_pinched_vertices_are_distinct() {
    let (p, t) = tetra();
    let open = Mesh::from_triangles(p.clone(), t[..3].to_vec(), Default::default()).unwrap();
    assert_eq!(open.require_solid(), Err(Error::Open));
    assert_eq!(open.statistics().boundary_edges, 3);
    let mut ps = p.clone();
    ps.extend(p.iter().map(|p| [p[0] + 3., p[1], p[2]]));
    let mut ts = t.clone();
    ts.extend(t.iter().map(|t| t.map(|i| i + 4)));
    let two = Mesh::from_triangles(ps, ts, Default::default()).unwrap();
    assert!(two.is_watertight());
    assert_eq!(two.require_solid(), Err(Error::Disconnected));
    let mut ps = p;
    ps.extend([[-1., 0., 0.], [0., -1., 0.], [0., 0., -1.]]);
    let mut ts = t.clone();
    ts.extend(t.iter().map(|t| t.map(|i| if i == 0 { 0 } else { i + 3 })));
    assert_eq!(
        Mesh::from_triangles(ps, ts, Default::default()).unwrap_err(),
        Error::NonManifoldVertex
    );
}
#[test]
fn mesh_attribute_and_unused_vertex_contract() {
    let (p, t) = tetra();
    let mut d = Mesh::from_triangles(p, t, Default::default())
        .unwrap()
        .data()
        .clone();
    d.normals[0][0] = [0.; 3];
    assert_eq!(
        Mesh::new(d, Default::default()).unwrap_err(),
        Error::InvalidAttributes
    );
    let (mut p, t) = tetra();
    p.push([2.; 3]);
    assert_eq!(
        Mesh::from_triangles(p, t, Default::default()).unwrap_err(),
        Error::UnusedVertex
    );
}
