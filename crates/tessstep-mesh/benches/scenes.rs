use std::{hint::black_box, sync::Arc, time::Instant};
use tessstep_mesh::{Mesh, scene::*};
fn main() {
    let mesh = Arc::new(
        Mesh::from_triangles(
            vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            vec![[0, 1, 2]],
            Default::default(),
        )
        .unwrap(),
    );
    let assets = vec![Asset {
        id: AssetId(1),
        mesh,
    }];
    let nodes: Vec<_> = (1..=10_000)
        .map(|i| Instance {
            id: InstanceId(i),
            parent: if i == 1 { None } else { Some(InstanceId(1)) },
            asset: Some(AssetId(1)),
            local_transform: Transform::identity(),
        })
        .collect();
    let start = Instant::now();
    for _ in 0..20 {
        black_box(Scene::new(assets.clone(), nodes.clone(), Default::default()).unwrap());
    }
    println!(
        "20 scenes x 10,000 shared occurrences: {:?}",
        start.elapsed()
    );
    let scene = Scene::new(assets, nodes, Default::default()).unwrap();
    let start = Instant::now();
    for i in 1..=10_000 {
        black_box(scene.bake(InstanceId(i), Default::default()).unwrap());
    }
    println!("10,000 explicit triangle bakes: {:?}", start.elapsed());
}
