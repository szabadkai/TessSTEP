#![allow(dead_code)]
use tessstep_curves::{Curve, PlaneFrame};
use tessstep_math::{Angle, Length, ModelTolerance, NumericalTolerance, Point, Vector};
use tessstep_topology::*;
pub fn tolerance() -> ModelTolerance {
    ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap()
}
pub fn plane() -> SurfaceGeometry {
    SurfaceGeometry::Analytic(tessstep_surfaces::Surface::plane(
        PlaneFrame::new(
            Point::new([0.; 3]).unwrap(),
            Vector::new([1., 0., 0.]).unwrap(),
            Vector::new([0., 1., 0.]).unwrap(),
            NumericalTolerance::default(),
        )
        .unwrap(),
    ))
}
pub fn polygons(points: &[[f64; 3]], loops: &[Vec<usize>]) -> RawBrep {
    let mut raw = RawBrep {
        vertices: points
            .iter()
            .map(|&p| Vertex {
                position: Point::new(p).unwrap(),
            })
            .collect(),
        ..RawBrep::default()
    };
    for indices in loops {
        let mut cs = vec![];
        for j in 0..indices.len() {
            let a = indices[j];
            let b = indices[(j + 1) % indices.len()];
            let existing = raw.edges.iter().position(|e| {
                e.vertices == [VertexId(a), VertexId(b)] || e.vertices == [VertexId(b), VertexId(a)]
            });
            let ei = existing.unwrap_or_else(|| {
                raw.edges.push(Edge {
                    vertices: [VertexId(a), VertexId(b)],
                    curve: CurveGeometry::Analytic(
                        Curve::line(
                            raw.vertices[a].position,
                            raw.vertices[b]
                                .position
                                .difference(raw.vertices[a].position)
                                .unwrap(),
                        )
                        .unwrap(),
                    ),
                    range: [0., 1.],
                });
                raw.edges.len() - 1
            });
            let e = &raw.edges[ei];
            let [v0, v1] = e.vertices;
            let p0 = points[v0.0];
            let p1 = points[v1.0];
            let pcurve = if p0[0] != p1[0] || p0[1] != p1[1] {
                Some(Pcurve {
                    curve: CurveGeometry::Analytic(
                        Curve::line(
                            Point::new([p0[0], p0[1]]).unwrap(),
                            Vector::new([p1[0] - p0[0], p1[1] - p0[1]]).unwrap(),
                        )
                        .unwrap(),
                    ),
                    range: [0., 1.],
                })
            } else {
                None
            };
            cs.push(CoedgeId(raw.coedges.len()));
            raw.coedges.push(Coedge {
                edge: EdgeId(ei),
                orientation: if v0.0 == a {
                    Orientation::Forward
                } else {
                    Orientation::Reversed
                },
                pcurve,
            });
        }
        let outer = WireId(raw.wires.len());
        raw.wires.push(Wire { coedges: cs });
        raw.faces.push(Face {
            surface: plane(),
            outer,
            holes: vec![],
            orientation: Orientation::Forward,
        });
    }
    raw
}
pub fn square() -> RawBrep {
    polygons(
        &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        &[vec![0, 1, 2, 3]],
    )
}
pub fn adjacent() -> RawBrep {
    polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
        ],
        &[vec![0, 1, 2, 3], vec![1, 4, 5, 2]],
    )
}
pub fn tetrahedron() -> RawBrep {
    let mut r = polygons(
        &[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        &[vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
    );
    r.shells.push(Shell {
        faces: (0..4).map(FaceId).collect(),
        closed: true,
    });
    r.solids.push(Solid { shell: ShellId(0) });
    r
}
pub fn disk() -> RawBrep {
    use std::f64::consts::TAU;
    let frame3 = PlaneFrame::new(
        Point::new([0.; 3]).unwrap(),
        Vector::new([1., 0., 0.]).unwrap(),
        Vector::new([0., 1., 0.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    let frame2 = PlaneFrame::new(
        Point::new([0.; 2]).unwrap(),
        Vector::new([1., 0.]).unwrap(),
        Vector::new([0., 1.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    RawBrep {
        vertices: vec![Vertex {
            position: Point::new([1., 0., 0.]).unwrap(),
        }],
        edges: vec![Edge {
            vertices: [VertexId(0); 2],
            curve: CurveGeometry::Analytic(
                Curve::circle(frame3, Length::metres(1.).unwrap()).unwrap(),
            ),
            range: [0., TAU],
        }],
        coedges: vec![Coedge {
            edge: EdgeId(0),
            orientation: Orientation::Forward,
            pcurve: Some(Pcurve {
                curve: CurveGeometry::Analytic(
                    Curve::circle(frame2, Length::metres(1.).unwrap()).unwrap(),
                ),
                range: [0., TAU],
            }),
        }],
        wires: vec![Wire {
            coedges: vec![CoedgeId(0)],
        }],
        faces: vec![Face {
            surface: plane(),
            outer: WireId(0),
            holes: vec![],
            orientation: Orientation::Forward,
        }],
        ..RawBrep::default()
    }
}
pub fn cylinder_seam() -> RawBrep {
    use std::f64::consts::TAU;
    let mut r = RawBrep {
        vertices: vec![
            Vertex {
                position: Point::new([1., 0., 0.]).unwrap(),
            },
            Vertex {
                position: Point::new([1., 0., 1.]).unwrap(),
            },
        ],
        ..RawBrep::default()
    };
    for z in [0., 1.] {
        let frame = PlaneFrame::new(
            Point::new([0., 0., z]).unwrap(),
            Vector::new([1., 0., 0.]).unwrap(),
            Vector::new([0., 1., 0.]).unwrap(),
            NumericalTolerance::default(),
        )
        .unwrap();
        r.edges.push(Edge {
            vertices: [VertexId(z as usize); 2],
            curve: CurveGeometry::Analytic(
                Curve::circle(frame, Length::metres(1.).unwrap()).unwrap(),
            ),
            range: [0., TAU],
        });
    }
    r.edges.push(Edge {
        vertices: [VertexId(0), VertexId(1)],
        curve: CurveGeometry::Analytic(
            Curve::line(r.vertices[0].position, Vector::new([0., 0., 1.]).unwrap()).unwrap(),
        ),
        range: [0., 1.],
    });
    for (edge, orientation, origin, tangent, range) in [
        (0, Orientation::Forward, [0., 0.], [1., 0.], [0., TAU]),
        (2, Orientation::Forward, [TAU, 0.], [0., 1.], [0., 1.]),
        (1, Orientation::Reversed, [0., 1.], [1., 0.], [0., TAU]),
        (2, Orientation::Reversed, [0., 0.], [0., 1.], [0., 1.]),
    ] {
        r.coedges.push(Coedge {
            edge: EdgeId(edge),
            orientation,
            pcurve: Some(Pcurve {
                curve: CurveGeometry::Analytic(
                    Curve::line(Point::new(origin).unwrap(), Vector::new(tangent).unwrap())
                        .unwrap(),
                ),
                range,
            }),
        });
    }
    r.wires.push(Wire {
        coedges: (0..4).map(CoedgeId).collect(),
    });
    let frame = PlaneFrame::new(
        Point::new([0.; 3]).unwrap(),
        Vector::new([1., 0., 0.]).unwrap(),
        Vector::new([0., 1., 0.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    r.faces.push(Face {
        surface: SurfaceGeometry::Analytic(
            tessstep_surfaces::Surface::cylinder(frame, Length::metres(1.).unwrap()).unwrap(),
        ),
        outer: WireId(0),
        holes: vec![],
        orientation: Orientation::Forward,
    });
    r
}

pub fn closed_cylinder() -> RawBrep {
    let mut r = cylinder_seam();
    for (edge, z, orientation) in [
        (0, 0., Orientation::Reversed),
        (1, 1., Orientation::Forward),
    ] {
        let cid = CoedgeId(r.coedges.len());
        let pc = match disk().coedges.remove(0).pcurve {
            Some(pc) => pc,
            None => unreachable!(),
        };
        r.coedges.push(Coedge {
            edge: EdgeId(edge),
            orientation: Orientation::Forward,
            pcurve: Some(pc),
        });
        let wire = WireId(r.wires.len());
        r.wires.push(Wire { coedges: vec![cid] });
        r.faces.push(Face {
            surface: SurfaceGeometry::Analytic(tessstep_surfaces::Surface::plane(
                PlaneFrame::new(
                    Point::new([0., 0., z]).unwrap(),
                    Vector::new([1., 0., 0.]).unwrap(),
                    Vector::new([0., 1., 0.]).unwrap(),
                    NumericalTolerance::default(),
                )
                .unwrap(),
            )),
            outer: wire,
            holes: vec![],
            orientation,
        });
    }
    r.shells.push(Shell {
        faces: (0..3).map(FaceId).collect(),
        closed: true,
    });
    r.solids.push(Solid { shell: ShellId(0) });
    r
}
pub fn nurbs_bump() -> RawBrep {
    let mut r = square();
    let controls: Vec<_> = (0..3)
        .flat_map(|i| {
            (0..3).map(move |j| {
                Point::new([
                    i as f64 * 0.5,
                    j as f64 * 0.5,
                    if i == 1 && j == 1 { 1. } else { 0. },
                ])
                .unwrap()
            })
        })
        .collect();
    r.faces[0].surface = SurfaceGeometry::Nurbs(
        tessstep_surfaces::NurbsSurface::new(
            [2, 2],
            [&[0., 0., 0., 1., 1., 1.]; 2],
            [3, 3],
            &controls,
            &[1.; 9],
            Default::default(),
        )
        .unwrap(),
    );
    r
}
