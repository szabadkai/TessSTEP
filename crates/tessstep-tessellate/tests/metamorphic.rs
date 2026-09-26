//! Independent area/incidence oracles over scale, placement and wire start changes.
#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use std::collections::BTreeMap;
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_tessellate::{PlanarOptions, SamplingLimits, sample_edges};
use tessstep_topology::{FaceId, Orientation, ValidationLimits};

#[test]
fn polygon_area_and_incidence_survive_scale_translation_and_wire_rotation() {
    // Concave L, area 5. Uniform scaling should multiply area by scale squared.
    let shape = [[0., 0.], [3., 0.], [3., 1.], [1., 1.], [1., 3.], [0., 3.]];
    for scale in [0.01, 1., 100.] {
        for shift in [[0., 0.], [17., -23.]] {
            for start in 0..shape.len() {
                for reversed in [false, true] {
                    let points =
                        shape.map(|[x, y]| [(x + shift[0]) * scale, (y + shift[1]) * scale, 0.]);
                    let order = (0..shape.len())
                        .map(|i| (i + start) % shape.len())
                        .collect();
                    let mut raw = support::polygons(&points, &[order]);
                    if reversed {
                        raw.faces[0].orientation = Orientation::Reversed;
                    }
                    let brep = raw
                        .validate(support::tolerance(), ValidationLimits::default())
                        .unwrap()
                        .normalize();
                    let tolerance = TessellationTolerance::new(
                        Length::metres(scale * 1e-3).unwrap(),
                        Angle::radians(0.1).unwrap(),
                    )
                    .unwrap();
                    let sampled =
                        sample_edges(&brep, tolerance, SamplingLimits::default()).unwrap();
                    let mesh = sampled
                        .triangulate_planar(FaceId(0), PlanarOptions::default())
                        .unwrap();
                    let mut area = 0.;
                    let mut edges = BTreeMap::new();
                    for triangle in mesh.triangles() {
                        let [a, b, c] = triangle.map(|i| {
                            mesh.vertices()[i as usize]
                                .edge_sample
                                .position
                                .coordinates()
                        });
                        let signed =
                            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * 0.5;
                        assert_eq!(signed.is_sign_negative(), reversed);
                        area += signed.abs();
                        for (a, b) in [
                            (triangle[0], triangle[1]),
                            (triangle[1], triangle[2]),
                            (triangle[2], triangle[0]),
                        ] {
                            let entry = edges.entry((a.min(b), a.max(b))).or_insert((0, 0));
                            entry.0 += 1;
                            entry.1 += if a < b { 1 } else { -1 };
                        }
                    }
                    assert!((area / (scale * scale) - 5.).abs() < 1e-9);
                    assert_eq!(mesh.triangles().len(), 4);
                    assert_eq!(edges.values().filter(|e| e.0 == 1).count(), 6);
                    assert!(edges.values().all(|e| e.0 == 1 || *e == (2, 0)));
                    let again = sampled
                        .triangulate_planar(FaceId(0), PlanarOptions::default())
                        .unwrap();
                    assert_eq!(mesh.triangles(), again.triangles());
                }
            }
        }
    }
}
