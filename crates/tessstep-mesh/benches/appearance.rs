use std::{hint::black_box, sync::Arc, time::Instant};
use tessstep_mesh::{
    Mesh,
    appearance::*,
    scene::{Asset, AssetId, Instance, InstanceId, Scene, Transform},
};
fn main() {
    let mesh = Arc::new(
        Mesh::from_triangles(
            vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            vec![[0, 1, 2]],
            Default::default(),
        )
        .unwrap(),
    );
    let nodes = (1..=10_000)
        .map(|id| Instance {
            id: InstanceId(id),
            parent: if id == 1 { None } else { Some(InstanceId(1)) },
            asset: Some(AssetId(1)),
            local_transform: Transform::identity(),
        })
        .collect();
    let scene = Arc::new(
        Scene::new(
            vec![Asset {
                id: AssetId(1),
                mesh,
            }],
            nodes,
            Default::default(),
        )
        .unwrap(),
    );
    let materials = vec![Material {
        id: MaterialId(1),
        color: LinearRgba::new([0.5, 0.2, 0.7, 0.75]).unwrap(),
    }];
    let bindings = vec![
        Binding {
            target: Target::Instance(InstanceId(1)),
            material: MaterialId(1),
        },
        Binding {
            target: Target::AssetFace(AssetId(1), 0),
            material: MaterialId(1),
        },
    ];
    let start = Instant::now();
    for _ in 0..20 {
        black_box(
            Appearance::new(
                scene.clone(),
                materials.clone(),
                bindings.clone(),
                Default::default(),
            )
            .unwrap(),
        );
    }
    println!(
        "20 appearances x 10,000 inherited occurrences: {:?}",
        start.elapsed()
    );
    let appearance = Appearance::new(scene, materials, bindings, Default::default()).unwrap();
    let start = Instant::now();
    for _ in 0..100 {
        for id in 1..=10_000 {
            black_box(appearance.resolve_triangle(InstanceId(id), 0).unwrap());
        }
    }
    println!(
        "1,000,000 allocation-free material queries: {:?}",
        start.elapsed()
    );
}
