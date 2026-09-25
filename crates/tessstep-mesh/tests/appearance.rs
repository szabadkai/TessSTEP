use std::sync::Arc;
use tessstep_mesh::{
    Mesh,
    appearance::*,
    scene::{Asset, AssetId, Instance, InstanceId, Scene, Transform},
};
fn palette() -> Vec<Material> {
    (1..=6)
        .map(|id| Material {
            id: MaterialId(id),
            color: LinearRgba::new([id as f64 / 6., 0.25, 0.5, if id == 6 { 0. } else { 1. }])
                .unwrap(),
        })
        .collect()
}
fn node(id: u64, parent: Option<u64>, asset: Option<u64>) -> Instance {
    Instance {
        id: InstanceId(id),
        parent: parent.map(InstanceId),
        asset: asset.map(AssetId),
        local_transform: Transform::identity(),
    }
}
fn scene() -> Arc<Scene> {
    let mut data = Mesh::from_triangles(
        vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
        vec![[0, 1, 2], [1, 3, 2]],
        Default::default(),
    )
    .unwrap()
    .data()
    .clone();
    data.face_ids = vec![0, u64::MAX];
    let mesh = Arc::new(Mesh::new(data, Default::default()).unwrap());
    Arc::new(
        Scene::new(
            vec![
                Asset {
                    id: AssetId(1),
                    mesh: mesh.clone(),
                },
                Asset {
                    id: AssetId(2),
                    mesh,
                },
            ],
            vec![
                node(4, Some(3), Some(1)),
                node(3, Some(2), None),
                node(2, None, None),
                node(5, Some(2), Some(1)),
                node(6, None, Some(1)),
                node(7, None, Some(2)),
            ],
            Default::default(),
        )
        .unwrap(),
    )
}
fn binding(target: Target, material: u64) -> Binding {
    Binding {
        target,
        material: MaterialId(material),
    }
}
fn resolved(target: Target, material: u64) -> Option<Resolved> {
    Some(Resolved {
        source: target,
        material: MaterialId(material),
    })
}
#[test]
fn appearance_precedence_inheritance_and_shared_asset_identity() {
    let scene = scene();
    let bindings = vec![
        binding(Target::Asset(AssetId(1)), 1),
        binding(Target::AssetFace(AssetId(1), u64::MAX), 2),
        binding(Target::Instance(InstanceId(2)), 3),
        binding(Target::Instance(InstanceId(3)), 4),
        binding(Target::InstanceFace(InstanceId(4), u64::MAX), 6),
    ];
    let appearance = Appearance::new(
        scene.clone(),
        palette(),
        bindings.clone(),
        Default::default(),
    )
    .unwrap();
    assert!(Arc::ptr_eq(appearance.scene(), &scene));
    assert_eq!(appearance.bindings(), bindings);
    assert_eq!(
        appearance.resolve_triangle(InstanceId(4), 0).unwrap(),
        resolved(Target::Instance(InstanceId(3)), 4)
    );
    assert_eq!(
        appearance.resolve_triangle(InstanceId(4), 1).unwrap(),
        resolved(Target::InstanceFace(InstanceId(4), u64::MAX), 6)
    );
    assert_eq!(
        appearance.resolve_triangle(InstanceId(5), 1).unwrap(),
        resolved(Target::Instance(InstanceId(2)), 3)
    );
    assert_eq!(
        appearance.resolve_triangle(InstanceId(6), 0).unwrap(),
        resolved(Target::Asset(AssetId(1)), 1)
    );
    assert_eq!(
        appearance.resolve_triangle(InstanceId(6), 1).unwrap(),
        resolved(Target::AssetFace(AssetId(1), u64::MAX), 2)
    );
    assert_eq!(appearance.resolve_triangle(InstanceId(7), 0).unwrap(), None);
    assert_eq!(
        appearance
            .material(MaterialId(6))
            .unwrap()
            .color
            .components()[3],
        0.
    );
    // Input binding order cannot change the winning assignments.
    let other = Appearance::new(
        scene,
        palette(),
        bindings.into_iter().rev().collect(),
        Default::default(),
    )
    .unwrap();
    for id in [4, 5, 6, 7] {
        for t in 0..2 {
            assert_eq!(
                appearance.resolve_triangle(InstanceId(id), t),
                other.resolve_triangle(InstanceId(id), t)
            );
        }
    }
}
#[test]
fn appearance_face_zero_shared_faces_and_reflected_bake_provenance() {
    let source = scene();
    let mut data = source.assets()[0].mesh.data().clone();
    data.face_ids = vec![0, 0];
    let mesh = Arc::new(Mesh::new(data, Default::default()).unwrap());
    let mut instance = node(1, None, Some(1));
    instance.local_transform =
        Transform::new([[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]], [0.; 3]).unwrap();
    let scene = Arc::new(
        Scene::new(
            vec![Asset {
                id: AssetId(1),
                mesh: mesh.clone(),
            }],
            vec![instance],
            Default::default(),
        )
        .unwrap(),
    );
    let appearance = Appearance::new(
        scene.clone(),
        palette(),
        vec![binding(Target::AssetFace(AssetId(1), 0), 2)],
        Default::default(),
    )
    .unwrap();
    drop(scene);
    drop(source);
    assert!(Arc::ptr_eq(&appearance.scene().assets()[0].mesh, &mesh));
    let baked = appearance
        .scene()
        .bake(InstanceId(1), Default::default())
        .unwrap();
    for t in 0..2 {
        assert_eq!(
            appearance.resolve_triangle(InstanceId(1), t).unwrap(),
            resolved(Target::AssetFace(AssetId(1), 0), 2)
        );
        assert_eq!(baked.data().face_ids[t], 0);
        let original = mesh.data().triangles[t];
        assert_eq!(
            baked.data().triangles[t],
            [original[0], original[2], original[1]]
        );
    }
}
#[test]
fn appearance_invalid_colors_targets_and_shadowed_assignments_fail() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.001, 1.001] {
        for k in 0..4 {
            let mut color = [0.5; 4];
            color[k] = value;
            assert_eq!(LinearRgba::new(color), Err(Error::InvalidColor));
        }
    }
    assert_eq!(
        LinearRgba::new([0., 1., 0., 1.]).unwrap().components(),
        [0., 1., 0., 1.]
    );
    let scene = scene();
    let check = |bindings| {
        Appearance::new(scene.clone(), palette(), bindings, Default::default()).unwrap_err()
    };
    assert_eq!(
        check(vec![binding(Target::Asset(AssetId(99)), 1)]),
        Error::MissingAsset(AssetId(99))
    );
    assert_eq!(
        check(vec![binding(Target::Instance(InstanceId(99)), 1)]),
        Error::MissingInstance(InstanceId(99))
    );
    assert_eq!(
        check(vec![binding(Target::InstanceFace(InstanceId(2), 0), 1)]),
        Error::NoAsset(InstanceId(2))
    );
    assert_eq!(
        check(vec![binding(Target::Asset(AssetId(1)), 99)]),
        Error::MissingMaterial(MaterialId(99))
    );
    assert_eq!(
        check(vec![binding(Target::Asset(AssetId(1)), 1); 2]),
        Error::DuplicateBinding(Target::Asset(AssetId(1)))
    );
    assert_eq!(
        check(vec![
            binding(Target::AssetFace(AssetId(1), 55), 1),
            binding(Target::Instance(InstanceId(2)), 2)
        ]),
        Error::MissingFace(AssetId(1), 55)
    );
    let mut materials = palette();
    materials[0].id = MaterialId(0);
    assert_eq!(
        Appearance::new(scene.clone(), materials, vec![], Default::default()).unwrap_err(),
        Error::InvalidMaterialId
    );
    assert_eq!(
        Appearance::new(
            scene.clone(),
            vec![palette()[0]; 2],
            vec![],
            Default::default()
        )
        .unwrap_err(),
        Error::DuplicateMaterial(MaterialId(1))
    );
    let appearance = Appearance::new(scene, vec![], vec![], Default::default()).unwrap();
    assert_eq!(appearance.resolve_triangle(InstanceId(6), 0), Ok(None));
    assert_eq!(
        appearance.resolve_triangle(InstanceId(6), 2),
        Err(Error::InvalidTriangle(InstanceId(6), 2))
    );
    assert_eq!(
        appearance.resolve_triangle(InstanceId(2), 0),
        Err(Error::NoAsset(InstanceId(2)))
    );
    assert_eq!(
        appearance.resolve_triangle(InstanceId(99), 0),
        Err(Error::MissingInstance(InstanceId(99)))
    );
}
#[test]
fn appearance_budgets_cover_faces_and_deep_inheritance_without_recursion() {
    let scene = scene();
    let bindings = vec![binding(Target::AssetFace(AssetId(1), 0), 1)];
    for limits in [
        Limits {
            max_materials: 0,
            ..Default::default()
        },
        Limits {
            max_bindings: 0,
            ..Default::default()
        },
        Limits {
            max_work: 0,
            ..Default::default()
        },
        Limits {
            max_work: 8,
            ..Default::default()
        },
    ] {
        assert_eq!(
            Appearance::new(scene.clone(), palette(), bindings.clone(), limits).unwrap_err(),
            Error::ResourceLimit
        );
    }
    let nodes = (1..=10_000)
        .rev()
        .map(|id| node(id, if id == 1 { None } else { Some(id - 1) }, Some(1)))
        .collect();
    let deep = Arc::new(
        Scene::new(
            vec![scene.assets()[0].clone()],
            nodes,
            tessstep_mesh::scene::Limits {
                max_depth: 10_000,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    let appearance = Appearance::new(
        deep,
        palette(),
        vec![binding(Target::Instance(InstanceId(1)), 5)],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        appearance.resolve_triangle(InstanceId(10_000), 1).unwrap(),
        resolved(Target::Instance(InstanceId(1)), 5)
    );
}
#[test]
fn appearance_bounded_mutations_match_independent_precedence_oracle() {
    let scene = scene();
    let targets = [
        Target::Asset(AssetId(1)),
        Target::AssetFace(AssetId(1), 0),
        Target::AssetFace(AssetId(1), u64::MAX),
        Target::Instance(InstanceId(2)),
        Target::Instance(InstanceId(3)),
        Target::Instance(InstanceId(4)),
        Target::InstanceFace(InstanceId(4), u64::MAX),
        Target::Instance(InstanceId(5)),
    ];
    let mut seed = 0xabcdef0123456789u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..2000 {
        let bindings: Vec<_> = targets
            .iter()
            .filter_map(|&target| {
                let v = next();
                (v & 1 == 0).then(|| binding(target, 1 + v % 6))
            })
            .collect();
        let appearance = Appearance::new(
            scene.clone(),
            palette(),
            bindings.clone(),
            Default::default(),
        )
        .unwrap();
        // Test oracle walks original parents per query, independently of the cache.
        for id in [4, 5, 6, 7] {
            for triangle in 0..2 {
                let instance = scene.instance(InstanceId(id)).unwrap();
                let asset = instance.source.asset.unwrap();
                let face = scene.asset(asset).unwrap().mesh.data().face_ids[triangle];
                let mut candidates = vec![Target::InstanceFace(InstanceId(id), face)];
                let mut parent = Some(InstanceId(id));
                while let Some(node) = parent {
                    candidates.push(Target::Instance(node));
                    parent = scene.instance(node).unwrap().source.parent;
                }
                candidates.extend([Target::AssetFace(asset, face), Target::Asset(asset)]);
                let expected = candidates.into_iter().find_map(|target| {
                    bindings
                        .iter()
                        .find(|b| b.target == target)
                        .map(|b| Resolved {
                            source: target,
                            material: b.material,
                        })
                });
                assert_eq!(
                    appearance
                        .resolve_triangle(InstanceId(id), triangle)
                        .unwrap(),
                    expected
                );
            }
        }
        let bits = f64::from_bits(next());
        assert_eq!(
            LinearRgba::new([bits; 4]).is_ok(),
            bits.is_finite() && (0. ..=1.).contains(&bits)
        );
    }
}

#[test]
fn appearance_face_overrides_and_asset_defaults_do_not_inherit() {
    let original = scene();
    let scene = Arc::new(
        Scene::new(
            original.assets().to_vec(),
            vec![
                node(1, None, Some(1)),
                node(2, Some(1), Some(1)),
                node(3, Some(1), Some(2)),
            ],
            Default::default(),
        )
        .unwrap(),
    );
    let appearance = Appearance::new(
        scene,
        palette(),
        vec![
            binding(Target::Asset(AssetId(1)), 1),
            binding(Target::InstanceFace(InstanceId(1), 0), 2),
        ],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        appearance.resolve_triangle(InstanceId(1), 0).unwrap(),
        resolved(Target::InstanceFace(InstanceId(1), 0), 2)
    );
    assert_eq!(
        appearance.resolve_triangle(InstanceId(2), 0).unwrap(),
        resolved(Target::Asset(AssetId(1)), 1)
    );
    assert_eq!(appearance.resolve_triangle(InstanceId(3), 0).unwrap(), None);
}
