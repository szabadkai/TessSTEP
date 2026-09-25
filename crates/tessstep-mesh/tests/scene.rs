use std::sync::Arc;
use tessstep_math::{ModelSpace, Point3};
use tessstep_mesh::{Mesh, scene::*};
fn mesh() -> Arc<Mesh> {
    let mut d = Mesh::from_triangles(
        vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        Default::default(),
    )
    .unwrap()
    .data()
    .clone();
    for (i, uv) in d.uvs.iter_mut().enumerate() {
        *uv = [[i as f64, 0.], [1., 0.], [0., 1.]];
    }
    d.face_ids = vec![11, 22, 33, 44];
    Arc::new(Mesh::new(d, Default::default()).unwrap())
}
fn node(id: u64, parent: Option<u64>, asset: Option<u64>, t: Transform) -> Instance {
    Instance {
        id: InstanceId(id),
        parent: parent.map(InstanceId),
        asset: asset.map(AssetId),
        local_transform: t,
    }
}
fn translation(x: f64, y: f64, z: f64) -> Transform {
    Transform::new([[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]], [x, y, z]).unwrap()
}
#[test]
fn scene_nested_reuse_preserves_buffers_and_composition_order() {
    let mesh = mesh();
    let rotate =
        Transform::new([[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]], [10., 0., 0.]).unwrap();
    let scene = Scene::new(
        vec![Asset {
            id: AssetId(u64::MAX),
            mesh: mesh.clone(),
        }],
        vec![
            node(7, Some(42), Some(u64::MAX), translation(2., 0., 0.)),
            node(42, None, None, rotate),
            node(9, None, Some(u64::MAX), translation(-3., 0., 0.)),
        ],
        Default::default(),
    )
    .unwrap();
    assert!(Arc::ptr_eq(&scene.assets()[0].mesh, &mesh));
    assert_eq!(scene.instances()[0].source.id, InstanceId(7));
    let child = scene.instance(InstanceId(7)).unwrap();
    assert_eq!(child.depth, 2);
    assert_eq!(
        child
            .world_transform
            .transform_point(Point3::<ModelSpace>::new([1., 0., 0.]).unwrap())
            .unwrap()
            .coordinates(),
        [10., 3., 0.]
    );
    assert_eq!(
        scene.bake(InstanceId(42), Default::default()).unwrap_err(),
        Error::NoAsset(InstanceId(42))
    );
    let baked = scene.bake(InstanceId(7), Default::default()).unwrap();
    assert_eq!(baked.data().positions[1], [10., 3., 0.]);
    assert_eq!(baked.data().face_ids, mesh.data().face_ids);
    baked.require_solid().unwrap();
    drop(scene);
    assert_eq!(baked.data().positions[0], [10., 2., 0.]);
    assert_eq!(mesh.data().positions[0], [0.; 3]);
}
#[test]
fn scene_reflections_nonuniform_scale_and_corner_attributes() {
    let mesh = mesh();
    // determinant -24, with shear; independently expected inverse-transpose normals.
    let transform =
        Transform::new([[-2., 1., 0.], [0., 3., 0.], [0., 0., 4.]], [3., 4., 5.]).unwrap();
    let scene = Scene::new(
        vec![Asset {
            id: AssetId(1),
            mesh: mesh.clone(),
        }],
        vec![
            node(1, None, Some(1), transform),
            node(2, Some(1), Some(1), transform),
        ],
        Default::default(),
    )
    .unwrap();
    assert!(scene.instances()[0].mirrored);
    assert!(!scene.instances()[1].mirrored);
    let baked = scene.bake(InstanceId(1), Default::default()).unwrap();
    baked.require_solid().unwrap();
    assert!((baked.statistics().signed_volume - 4.).abs() < 1e-12);
    for (i, tri) in mesh.data().triangles.iter().enumerate() {
        assert_eq!(baked.data().triangles[i], [tri[0], tri[2], tri[1]]);
        assert_eq!(
            baked.data().uvs[i],
            [
                mesh.data().uvs[i][0],
                mesh.data().uvs[i][2],
                mesh.data().uvs[i][1]
            ]
        );
        let n = mesh.data().normals[i][0];
        let expected = [-n[0] / 2., n[0] / 6. + n[1] / 3., n[2] / 4.];
        let length = expected.iter().map(|v| v * v).sum::<f64>().sqrt();
        for (actual, expected) in baked.data().normals[i][0].iter().zip(expected) {
            assert!((actual - expected / length).abs() < 1e-12);
        }
    }
    scene
        .bake(InstanceId(2), Default::default())
        .unwrap()
        .require_solid()
        .unwrap();
}
#[test]
fn scene_rejects_invalid_graphs_transforms_and_limits() {
    let assets = vec![Asset {
        id: AssetId(1),
        mesh: mesh(),
    }];
    let identity = Transform::identity();
    let build = |nodes| Scene::new(assets.clone(), nodes, Default::default()).unwrap_err();
    assert_eq!(build(vec![node(0, None, None, identity)]), Error::InvalidId);
    assert_eq!(
        build(vec![node(1, None, None, identity); 2]),
        Error::DuplicateInstance(InstanceId(1))
    );
    assert_eq!(
        build(vec![node(1, None, Some(5), identity)]),
        Error::MissingAsset(AssetId(5))
    );
    assert_eq!(
        build(vec![node(1, Some(5), None, identity)]),
        Error::MissingInstance(InstanceId(5))
    );
    assert_eq!(build(vec![node(1, Some(1), None, identity)]), Error::Cycle);
    assert_eq!(
        build(vec![
            node(3, None, None, identity),
            node(1, Some(2), None, identity),
            node(2, Some(1), None, identity)
        ]),
        Error::Cycle
    );
    let singular = Transform::new([[0.; 3]; 3], [0.; 3]).unwrap();
    assert!(matches!(
        build(vec![node(1, None, None, singular)]),
        Error::Transform(InstanceId(1), _)
    ));
    let huge = translation(f64::MAX, 0., 0.);
    assert!(matches!(
        build(vec![
            node(1, None, None, huge),
            node(2, Some(1), None, huge)
        ]),
        Error::Transform(InstanceId(2), _)
    ));
    for limits in [
        Limits {
            max_assets: 0,
            ..Default::default()
        },
        Limits {
            max_instances: 0,
            ..Default::default()
        },
        Limits {
            max_depth: 0,
            ..Default::default()
        },
        Limits {
            max_work: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            Scene::new(
                assets.clone(),
                vec![node(1, None, Some(1), identity)],
                limits
            )
            .unwrap_err(),
            Error::ResourceLimit
        );
    }
    assert_eq!(
        Scene::new(vec![assets[0].clone(); 2], vec![], Default::default()).unwrap_err(),
        Error::DuplicateAsset(AssetId(1))
    );
    let scene = Scene::new(
        assets,
        vec![node(1, None, Some(1), identity)],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        scene.bake(InstanceId(2), Default::default()).unwrap_err(),
        Error::MissingInstance(InstanceId(2))
    );
    for limits in [
        tessstep_mesh::Limits {
            max_vertices: 0,
            ..Default::default()
        },
        tessstep_mesh::Limits {
            max_triangles: 0,
            ..Default::default()
        },
        tessstep_mesh::Limits {
            max_work: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            scene.bake(InstanceId(1), limits).unwrap_err(),
            Error::ResourceLimit
        );
    }
    assert!(
        Scene::new(vec![], vec![], Default::default())
            .unwrap()
            .instances()
            .is_empty()
    );
}
#[test]
fn scene_deep_forests_and_scaled_handedness_are_bounded() {
    let nodes: Vec<_> = (1..=10_000)
        .rev()
        .map(|i| {
            node(
                i,
                if i == 1 { None } else { Some(i - 1) },
                None,
                Transform::identity(),
            )
        })
        .collect();
    assert_eq!(
        Scene::new(vec![], nodes.clone(), Default::default()).unwrap_err(),
        Error::ResourceLimit
    );
    let scene = Scene::new(
        vec![],
        nodes,
        Limits {
            max_depth: 10_000,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(scene.instances()[0].depth, 10_000);
    for scale in [1e-150, 1e150] {
        let t =
            Transform::new([[0., scale, 0.], [scale, 0., 0.], [0., 0., scale]], [0.; 3]).unwrap();
        let scene = Scene::new(vec![], vec![node(1, None, None, t)], Default::default()).unwrap();
        assert!(scene.instances()[0].mirrored);
    }
}
#[test]
fn scene_bounded_mutation_smoke() {
    let assets = vec![Asset {
        id: AssetId(1),
        mesh: mesh(),
    }];
    let mut seed = 0xabcdef0123456789u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..2000 {
        let mut nodes = Vec::new();
        for i in 1..=16 {
            let value = f64::from_bits(next());
            let parent = next() % 18;
            if let Ok(t) = Transform::new([[value, 0., 0.], [0., 1., 0.], [0., 0., 1.]], [0.; 3]) {
                nodes.push(node(
                    i,
                    if parent == 0 { None } else { Some(parent) },
                    Some(1),
                    t,
                ));
            }
        }
        if let Ok(scene) = Scene::new(
            assets.clone(),
            nodes,
            Limits {
                max_work: 1000,
                max_depth: 16,
                ..Default::default()
            },
        ) {
            for instance in scene.instances() {
                if let Ok(mesh) = scene.bake(instance.source.id, Default::default()) {
                    assert!(
                        mesh.data()
                            .positions
                            .iter()
                            .flatten()
                            .all(|v| v.is_finite())
                    );
                }
            }
        }
    }
}
