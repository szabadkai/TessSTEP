#[path = "../../../tessstep-topology/tests/support/mod.rs"]
mod fixtures;
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_tessellate::{
    PlanarOptions, SamplingLimits, TessellationOptions, sample_edges, tessellate_faces,
};
use tessstep_topology::*;
use tessstep_trim::{Options, reconstruct};
pub fn exercise(data: &[u8]) {
    let byte = |i: usize| data.get(i).copied().unwrap_or(0);
    let mut raw = match byte(0) % 6 {
        0 => fixtures::square(),
        1 => fixtures::adjacent(),
        2 => fixtures::disk(),
        3 => fixtures::cylinder_seam(),
        4 => fixtures::closed_cylinder(),
        _ => fixtures::nurbs_bump(),
    };
    let number = |i: usize| {
        let mut bytes = [0; 8];
        for (j, b) in bytes.iter_mut().enumerate() {
            *b = byte(i + j);
        }
        f64::from_bits(u64::from_le_bytes(bytes))
    };
    for chunk in 0..(data.len() / 12).min(8) {
        let offset = chunk * 12;
        let index = byte(offset + 1) as usize;
        match byte(offset + 2) % 8 {
            0 => {
                let i = index % raw.edges.len();
                raw.edges[i].vertices[0] = VertexId(byte(offset + 3) as usize);
            }
            1 => {
                let i = index % raw.coedges.len();
                raw.coedges[i].edge = EdgeId(byte(offset + 3) as usize);
            }
            2 => {
                let i = index % raw.coedges.len();
                raw.coedges[i].orientation = Orientation::Reversed;
            }
            3 => {
                let i = index % raw.edges.len();
                raw.edges[i].range[1] = number(offset + 4);
            }
            4 => {
                let i = index % raw.coedges.len();
                if let Some(pc) = &mut raw.coedges[i].pcurve {
                    pc.range[0] = number(offset + 4);
                }
            }
            5 => {
                let i = index % raw.wires.len();
                raw.wires[i]
                    .coedges
                    .push(CoedgeId(byte(offset + 3) as usize));
            }
            6 => {
                let i = index % raw.coedges.len();
                raw.coedges[i].pcurve = None;
            }
            _ => {
                raw.shells.push(Shell {
                    faces: vec![FaceId(index)],
                    closed: byte(offset + 3) % 2 == 0,
                });
            }
        }
    }
    let result = raw.validate(
        fixtures::tolerance(),
        ValidationLimits {
            max_records: 1024,
            max_work: 4096,
        },
    );
    if let Ok(validated) = result {
        let n = validated.normalize();
        let options = Options {
            uv_tolerance: 1e-3,
            max_depth: 12,
            max_samples: 1024,
            max_work: 100_000,
        };
        for i in 0..n.data().faces.len() {
            if let Ok(trim) = reconstruct(&n, FaceId(i), options) {
                assert!(trim.outer().signed_area().is_finite());
                let _ = trim.classify([number(8), number(16)]);
            }
        }
        let tolerance =
            TessellationTolerance::new(Length::metres(1e-3).unwrap(), Angle::radians(0.1).unwrap())
                .unwrap();
        let faces: Vec<_> = (0..n.data().faces.len()).map(FaceId).collect();
        if let Ok(mesh) = tessellate_faces(
            &n,
            &faces,
            tolerance,
            TessellationOptions {
                planar: PlanarOptions {
                    trim: options,
                    max_vertices: 1024,
                    max_triangles: 2048,
                    max_work: 100_000,
                },
                max_rounds: 8,
                max_boundary_passes: 2,
                max_evaluations: 16_384,
                max_work: 200_000,
                mesh: Default::default(),
                sampling: SamplingLimits {
                    max_samples: 1024,
                    max_evaluations: 8192,
                    max_depth: 12,
                },
            },
        ) {
            assert!(
                mesh.data()
                    .triangles
                    .iter()
                    .flatten()
                    .all(|&i| (i as usize) < mesh.data().positions.len())
            );
            assert!(
                mesh.data()
                    .positions
                    .iter()
                    .flatten()
                    .all(|x| x.is_finite())
            );
        }
        if let Ok(samples) = sample_edges(
            &n,
            tolerance,
            SamplingLimits {
                max_samples: 1024,
                max_evaluations: 8192,
                max_depth: 12,
            },
        ) {
            assert_eq!(samples.edges().len(), n.data().edges.len());
            for edge in samples.edges() {
                assert!(
                    edge.samples()
                        .windows(2)
                        .all(|w| w[0].parameter < w[1].parameter)
                );
            }
            for i in 0..n.data().faces.len() {
                let _ = samples.face_boundary(FaceId(i), options, 2048);
                if let Ok(mesh) = samples.triangulate_planar(
                    FaceId(i),
                    PlanarOptions {
                        trim: options,
                        max_vertices: 1024,
                        max_triangles: 2048,
                        max_work: 100_000,
                    },
                ) {
                    assert!(
                        mesh.triangles()
                            .iter()
                            .flatten()
                            .all(|&i| (i as usize) < mesh.vertices().len())
                    );
                    assert_eq!(
                        mesh.triangles().len(),
                        mesh.vertices().len() + 2 * (mesh.boundaries().len() - 1) - 2
                    );
                }
            }
        }
    }
}
