#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use support::*;
use tessstep_curves::{Curve, PlaneFrame};
use tessstep_math::{Angle, Length, NumericalTolerance, Point, TessellationTolerance, Vector};
use tessstep_surfaces::{NurbsSurface, Surface};
use tessstep_tessellate::*;
use tessstep_topology::*;
fn tol(chord: f64, angle: f64) -> TessellationTolerance {
    TessellationTolerance::new(
        Length::metres(chord).unwrap(),
        Angle::radians(angle).unwrap(),
    )
    .unwrap()
}
fn normalize(raw: RawBrep) -> NormalizedBrep {
    raw.validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize()
}
fn frame(z: f64) -> PlaneFrame<tessstep_math::ModelSpace, 3> {
    PlaneFrame::new(
        Point::new([0., 0., z]).unwrap(),
        Vector::new([1., 0., 0.]).unwrap(),
        Vector::new([0., 1., 0.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap()
}
fn verify(mesh: &tessstep_mesh::Mesh, n: &NormalizedBrep, tolerance: TessellationTolerance) {
    let d = mesh.data();
    for (i, &tri) in d.triangles.iter().enumerate() {
        let points = tri
            .map(|i| Point::<tessstep_math::ModelSpace, 3>::new(d.positions[i as usize]).unwrap());
        let mut normal = points[1]
            .difference(points[0])
            .unwrap()
            .cross(points[2].difference(points[0]).unwrap())
            .unwrap()
            .normalized()
            .unwrap();
        let face = &n.data().faces[d.face_ids[i] as usize];
        if face.orientation.is_reversed() {
            normal = normal.vector().scaled(-1.).unwrap().normalized().unwrap();
        }
        // Independent dense barycentric probes, beyond the implementation's stencil.
        for a in 0..=8 {
            for b in 0..=8 - a {
                let weights = [a as f64 / 8., b as f64 / 8., (8 - a - b) as f64 / 8.];
                let uv = [0, 1].map(|k| (0..3).map(|j| weights[j] * d.uvs[i][j][k]).sum());
                let p = Point::new([0, 1, 2].map(|k| {
                    (0..3)
                        .map(|j| weights[j] * points[j].coordinates()[k])
                        .sum()
                }))
                .unwrap();
                let jet = face.surface.evaluate(uv).unwrap();
                assert!(
                    jet.position.distance(p).unwrap()
                        <= tolerance.chord().as_metres() * 1.02 + 1e-12
                );
                assert!(
                    jet.normal(NumericalTolerance::default())
                        .unwrap()
                        .angle_to(normal)
                        .unwrap()
                        .as_radians()
                        <= tolerance.normal_angle().as_radians() * 1.02 + 1e-12
                );
            }
        }
    }
}
#[test]
fn analytic_cylinder_refines_and_preserves_periodic_seam() {
    let n = normalize(cylinder_seam());
    let t = tol(0.02, 0.3);
    let m = tessellate_faces(&n, &[FaceId(0)], t, TessellationOptions::default()).unwrap();
    assert!(m.data().triangles.len() > 20);
    assert!(!m.is_watertight());
    verify(&m, &n, t);
    let mut seam = false;
    for (i, tri) in m.data().triangles.iter().enumerate() {
        for (k, &index) in tri.iter().enumerate() {
            let p = m.data().positions[index as usize];
            let uv = m.data().uvs[i][k];
            if uv[0] > 6. {
                assert!((p[0] - uv[0].cos()).abs() < 1e-9);
                seam = true;
            }
        }
    }
    assert!(seam);
}
#[test]
fn nurbs_interior_and_straight_boundary_normals_refine() {
    let n = normalize(nurbs_bump());
    let t = tol(0.015, 0.3);
    let m = tessellate_faces(&n, &[FaceId(0)], t, TessellationOptions::default()).unwrap();
    assert!(m.data().positions.len() > 4);
    verify(&m, &n, t);
    let fine = tessellate_faces(
        &n,
        &[FaceId(0)],
        tol(0.005, 0.15),
        TessellationOptions::default(),
    )
    .unwrap();
    assert!(fine.data().triangles.len() > m.data().triangles.len());
}
#[test]
fn closed_cylinder_welds_caps_and_has_positive_volume() {
    let n = normalize(closed_cylinder());
    let t = tol(0.02, 0.3);
    let m = tessellate_solid(&n, SolidId(0), t, TessellationOptions::default()).unwrap();
    assert!(m.is_watertight());
    assert_eq!(m.statistics().components, 1);
    assert!((m.statistics().signed_volume - std::f64::consts::PI).abs() < 0.1);
    verify(&m, &n, t);
    let again = tessellate_solid(&n, SolidId(0), t, TessellationOptions::default()).unwrap();
    assert_eq!(m.data().positions, again.data().positions);
    assert_eq!(m.data().triangles, again.data().triangles);
}
#[test]
fn adaptive_failures_are_explicit_and_bounded() {
    let n = normalize(nurbs_bump());
    let t = tol(0.001, 0.1);
    for o in [
        TessellationOptions {
            max_work: 0,
            ..Default::default()
        },
        TessellationOptions {
            max_evaluations: 0,
            ..Default::default()
        },
        TessellationOptions {
            max_rounds: 0,
            ..Default::default()
        },
        TessellationOptions {
            max_boundary_passes: 0,
            ..Default::default()
        },
    ] {
        assert!(tessellate_faces(&n, &[FaceId(0)], t, o).is_err());
    }
    assert!(matches!(
        tessellate_faces(&n, &[FaceId(0), FaceId(0)], t, Default::default())
            .unwrap_err()
            .kind,
        TessellationErrorKind::DuplicateFace
    ));
    assert!(matches!(
        tessellate_solid(&n, SolidId(0), t, Default::default())
            .unwrap_err()
            .kind,
        TessellationErrorKind::InvalidHandle
    ));
}
fn analytic_patch(kind: usize) -> RawBrep {
    let surface = match kind {
        0 => Surface::sphere(frame(0.), Length::metres(1.).unwrap()).unwrap(),
        1 => Surface::cone(frame(0.), Angle::radians(0.4).unwrap()).unwrap(),
        _ => Surface::torus(
            frame(0.),
            Length::metres(2.).unwrap(),
            Length::metres(0.5).unwrap(),
        )
        .unwrap(),
    };
    let (u0, u1, v0, v1) = (0.2, 1.4, 0.3, 1.1);
    let uv = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
    let mut r = RawBrep {
        vertices: uv
            .map(|p| Vertex {
                position: surface.evaluate(p[0], p[1]).unwrap().position,
            })
            .to_vec(),
        ..Default::default()
    };
    for (i, j, axis, constant, range, orientation) in [
        (0, 1, 0, v0, [u0, u1], Orientation::Forward),
        (1, 2, 1, u1, [v0, v1], Orientation::Forward),
        (3, 2, 0, v1, [u0, u1], Orientation::Reversed),
        (0, 3, 1, u0, [v0, v1], Orientation::Reversed),
    ] {
        let curve = if axis == 0 {
            let (radius, z) = match kind {
                0 => (constant.cos(), constant.sin()),
                1 => (constant * 0.4f64.sin(), constant * 0.4f64.cos()),
                _ => (2. + 0.5 * constant.cos(), 0.5 * constant.sin()),
            };
            Curve::circle(frame(z), Length::metres(radius).unwrap()).unwrap()
        } else if kind == 1 {
            Curve::line(
                Point::new([0.; 3]).unwrap(),
                Vector::new([
                    0.4f64.sin() * constant.cos(),
                    0.4f64.sin() * constant.sin(),
                    0.4f64.cos(),
                ])
                .unwrap(),
            )
            .unwrap()
        } else {
            let radius = if kind == 0 { 1. } else { 0.5 };
            let center = if kind == 0 {
                [0.; 3]
            } else {
                [2. * constant.cos(), 2. * constant.sin(), 0.]
            };
            Curve::circle(
                PlaneFrame::new(
                    Point::new(center).unwrap(),
                    Vector::new([constant.cos(), constant.sin(), 0.]).unwrap(),
                    Vector::new([0., 0., 1.]).unwrap(),
                    NumericalTolerance::default(),
                )
                .unwrap(),
                Length::metres(radius).unwrap(),
            )
            .unwrap()
        };
        let edge = EdgeId(r.edges.len());
        r.edges.push(Edge {
            vertices: [VertexId(i), VertexId(j)],
            curve: CurveGeometry::Analytic(curve),
            range,
        });
        let (origin, direction) = if axis == 0 {
            ([0., constant], [1., 0.])
        } else {
            ([constant, 0.], [0., 1.])
        };
        r.coedges.push(Coedge {
            edge,
            orientation,
            pcurve: Some(Pcurve {
                curve: CurveGeometry::Analytic(
                    Curve::line(Point::new(origin).unwrap(), Vector::new(direction).unwrap())
                        .unwrap(),
                ),
                range,
            }),
        });
    }
    r.wires.push(Wire {
        coedges: (0..4).map(CoedgeId).collect(),
    });
    r.faces.push(Face {
        surface: SurfaceGeometry::Analytic(surface),
        outer: WireId(0),
        holes: vec![],
        orientation: Orientation::Forward,
    });
    r
}
#[test]
fn analytic_sphere_cone_and_torus_regular_patches() {
    for kind in 0..3 {
        let n = normalize(analytic_patch(kind));
        let t = tol(0.01, 0.25);
        let mesh = tessellate_faces(&n, &[FaceId(0)], t, Default::default()).unwrap();
        verify(&mesh, &n, t);
    }
}
#[test]
fn nurbs_rational_and_narrow_knot_spans_are_sampled() {
    let mut r = nurbs_bump();
    if let SurfaceGeometry::Nurbs(s) = &r.faces[0].surface {
        let mut weights = [1.; 9];
        weights[4] = 2.;
        r.faces[0].surface = SurfaceGeometry::Nurbs(
            NurbsSurface::new(
                [2, 2],
                [&[0., 0., 0., 1., 1., 1.]; 2],
                [3, 3],
                s.controls(),
                &weights,
                Default::default(),
            )
            .unwrap(),
        );
    }
    let n = normalize(r);
    let t = tol(0.01, 0.3);
    let m = tessellate_faces(&n, &[FaceId(0)], t, Default::default()).unwrap();
    verify(&m, &n, t);
    let knots = [0., 0., 0., 0.31, 0.31, 0.32, 0.32, 0.33, 0.33, 1., 1., 1.];
    let controls: Vec<_> = (0..9)
        .flat_map(|i| {
            (0..3).map(move |j| {
                Point::new([
                    (knots[i + 1] + knots[i + 2]) * 0.5,
                    j as f64 * 0.5,
                    if i == 4 && j == 1 { 0.001 } else { 0. },
                ])
                .unwrap()
            })
        })
        .collect();
    let mut r = square();
    r.faces[0].surface = SurfaceGeometry::Nurbs(
        NurbsSurface::new(
            [2, 2],
            [&knots, &[0., 0., 0., 1., 1., 1.]],
            [9, 3],
            &controls,
            &[1.; 27],
            Default::default(),
        )
        .unwrap(),
    );
    let n = normalize(r);
    let t = tol(0.0001, 0.5);
    let m = tessellate_faces(&n, &[FaceId(0)], t, Default::default()).unwrap();
    assert!(m.data().positions.iter().any(|p| p[2] > 0.0003));
    verify(&m, &n, t);
}
#[test]
fn solid_torus_welds_both_periodic_seams() {
    let rmajor = 2.;
    let rminor = 0.5;
    let mut r = RawBrep {
        vertices: vec![Vertex {
            position: Point::new([rmajor + rminor, 0., 0.]).unwrap(),
        }],
        ..Default::default()
    };
    let curves = [
        Curve::circle(frame(0.), Length::metres(rmajor + rminor).unwrap()).unwrap(),
        Curve::circle(
            PlaneFrame::new(
                Point::new([rmajor, 0., 0.]).unwrap(),
                Vector::new([1., 0., 0.]).unwrap(),
                Vector::new([0., 0., 1.]).unwrap(),
                NumericalTolerance::default(),
            )
            .unwrap(),
            Length::metres(rminor).unwrap(),
        )
        .unwrap(),
    ];
    for curve in curves {
        r.edges.push(Edge {
            vertices: [VertexId(0); 2],
            curve: CurveGeometry::Analytic(curve),
            range: [0., std::f64::consts::TAU],
        });
    }
    for (edge, orientation, origin, dir) in [
        (0, Orientation::Forward, [0., 0.], [1., 0.]),
        (
            1,
            Orientation::Forward,
            [std::f64::consts::TAU, 0.],
            [0., 1.],
        ),
        (
            0,
            Orientation::Reversed,
            [0., std::f64::consts::TAU],
            [1., 0.],
        ),
        (1, Orientation::Reversed, [0., 0.], [0., 1.]),
    ] {
        r.coedges.push(Coedge {
            edge: EdgeId(edge),
            orientation,
            pcurve: Some(Pcurve {
                curve: CurveGeometry::Analytic(
                    Curve::line(Point::new(origin).unwrap(), Vector::new(dir).unwrap()).unwrap(),
                ),
                range: [0., std::f64::consts::TAU],
            }),
        });
    }
    r.wires.push(Wire {
        coedges: (0..4).map(CoedgeId).collect(),
    });
    r.faces.push(Face {
        surface: SurfaceGeometry::Analytic(
            Surface::torus(
                frame(0.),
                Length::metres(rmajor).unwrap(),
                Length::metres(rminor).unwrap(),
            )
            .unwrap(),
        ),
        outer: WireId(0),
        holes: vec![],
        orientation: Orientation::Forward,
    });
    r.shells.push(Shell {
        faces: vec![FaceId(0)],
        closed: true,
    });
    r.solids.push(Solid { shell: ShellId(0) });
    let n = normalize(r);
    let t = tol(0.03, 0.4);
    let m = tessellate_solid(&n, SolidId(0), t, Default::default()).unwrap();
    assert!(m.is_watertight());
    assert!(
        (m.statistics().signed_volume
            - 2. * std::f64::consts::PI.powi(2) * rmajor * rminor * rminor)
            .abs()
            < 0.7
    );
    verify(&m, &n, t);
}
#[test]
fn adjacent_faces_share_requested_refinement_and_keep_sharp_normals() {
    let mut r = adjacent();
    r.faces[0].surface = nurbs_bump().faces.remove(0).surface;
    let n = normalize(r);
    let t = tol(0.01, 0.25);
    let mesh = tessellate_faces(&n, &[FaceId(0), FaceId(1)], t, Default::default()).unwrap();
    verify(&mesh, &n, t);
    let data = mesh.data();
    let mut shared = 0;
    for (i, p) in data.positions.iter().enumerate() {
        if p[0] == 1. && p[1] > 0. && p[1] < 1. {
            let mut normals = [None, None];
            for (j, tri) in data.triangles.iter().enumerate() {
                if let Some(k) = tri.iter().position(|&v| v as usize == i) {
                    normals[data.face_ids[j] as usize] = Some(data.normals[j][k]);
                }
            }
            assert!(normals.iter().all(Option::is_some));
            assert_ne!(normals[0], normals[1]);
            shared += 1;
        }
    }
    assert!(shared > 0);
}
#[test]
fn adaptive_planar_holes_and_inward_solid_are_explicit() {
    let mut r = polygons(
        &[
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 4., 0.],
            [0., 4., 0.],
            [1., 1., 0.],
            [1., 2., 0.],
            [2., 2., 0.],
            [2., 1., 0.],
        ],
        &[vec![0, 1, 2, 3], vec![4, 5, 6, 7]],
    );
    r.faces.truncate(1);
    r.faces[0].holes.push(WireId(1));
    r.faces[0].orientation = Orientation::Reversed;
    let n = normalize(r);
    let m = tessellate_faces(&n, &[FaceId(0)], tol(0.01, 0.3), Default::default()).unwrap();
    assert_eq!(m.statistics().boundary_edges, 8);
    assert!(m.data().normals.iter().flatten().all(|n| n[2] == -1.));
    let mut r = closed_cylinder();
    for f in &mut r.faces {
        f.orientation = if f.orientation.is_reversed() {
            Orientation::Forward
        } else {
            Orientation::Reversed
        };
    }
    let n = normalize(r);
    assert!(matches!(
        tessellate_solid(&n, SolidId(0), tol(0.02, 0.3), Default::default())
            .unwrap_err()
            .kind,
        TessellationErrorKind::Mesh(tessstep_mesh::Error::NonPositiveVolume)
    ));
}
