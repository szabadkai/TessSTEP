//! Milestone 22.2: curvature-anisotropic refinement, polar caps, planar cost and
//! named tessellation budgets, on authored geometry.
#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use support::*;
use tessstep_curves::{Curve, PlaneFrame};
use tessstep_math::{
    Angle, Length, ModelSpace, NumericalTolerance, Point, TessellationTolerance, Vector,
};
use tessstep_surfaces::Surface;
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
fn frame3(o: [f64; 3], x: [f64; 3], y: [f64; 3]) -> PlaneFrame<ModelSpace, 3> {
    PlaneFrame::new(
        Point::new(o).unwrap(),
        Vector::new(x).unwrap(),
        Vector::new(y).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap()
}
fn line2(origin: [f64; 2], tangent: [f64; 2], range: [f64; 2]) -> Option<Pcurve> {
    Some(Pcurve {
        curve: CurveGeometry::Analytic(
            Curve::line(Point::new(origin).unwrap(), Vector::new(tangent).unwrap()).unwrap(),
        ),
        range,
    })
}
fn circle2(center: [f64; 2], radius: f64) -> Option<Pcurve> {
    let frame = PlaneFrame::new(
        Point::new(center).unwrap(),
        Vector::new([1., 0.]).unwrap(),
        Vector::new([0., 1.]).unwrap(),
        NumericalTolerance::default(),
    )
    .unwrap();
    Some(Pcurve {
        curve: CurveGeometry::Analytic(
            Curve::circle(frame, Length::metres(radius).unwrap()).unwrap(),
        ),
        range: [0., TAU],
    })
}
fn coedge(edge: usize, forward: bool, pcurve: Option<Pcurve>) -> Coedge {
    Coedge {
        edge: EdgeId(edge),
        orientation: if forward {
            Orientation::Forward
        } else {
            Orientation::Reversed
        },
        pcurve,
    }
}

/// Dense independent probes of every triangle: distance to the facet plane and
/// angle between surface and facet normals, within `slack` of the tolerances.
/// Corners at a singular point (pole) are probed by chord only.
fn verify_within(
    mesh: &tessstep_mesh::Mesh,
    n: &NormalizedBrep,
    t: TessellationTolerance,
    slack: f64,
) {
    let d = mesh.data();
    for (i, tri) in d.triangles.iter().enumerate() {
        let points = tri.map(|k| Point::<ModelSpace, 3>::new(d.positions[k as usize]).unwrap());
        let face = &n.data().faces[d.face_ids[i] as usize];
        let mut normal = points[1]
            .difference(points[0])
            .unwrap()
            .cross(points[2].difference(points[0]).unwrap())
            .unwrap()
            .normalized()
            .unwrap();
        if face.orientation.is_reversed() {
            normal = normal.vector().scaled(-1.).unwrap().normalized().unwrap();
        }
        // Corners at a pole (no surface normal there): the chart's first axis runs
        // along the pole line, so such a facet is the polar wedge between the rays to
        // its other corners.
        let singular: Vec<usize> = (0..3)
            .filter(|&k| {
                face.surface
                    .evaluate(d.uvs[i][k])
                    .unwrap()
                    .normal(NumericalTolerance::default())
                    .is_err()
            })
            .collect();
        for a in 0..=6 {
            for b in 0..=6 - a {
                let w = [a as f64 / 6., b as f64 / 6., (6 - a - b) as f64 / 6.];
                let mut uv: [f64; 2] = [0, 1].map(|k| (0..3).map(|j| w[j] * d.uvs[i][j][k]).sum());
                if let [s] = singular.as_slice() {
                    let rest = 1. - w[*s];
                    if rest > 0. {
                        uv[0] = (0..3)
                            .filter(|j| j != s)
                            .map(|j| w[j] * d.uvs[i][j][0])
                            .sum::<f64>()
                            / rest;
                    }
                }
                let jet = face.surface.evaluate(uv).unwrap();
                let offset = jet
                    .position
                    .difference(points[0])
                    .unwrap()
                    .dot(normal.vector())
                    .unwrap()
                    .abs();
                assert!(
                    offset <= t.chord().as_metres() * slack + 1e-12,
                    "chord {offset}"
                );
                let Ok(surface) = jet.normal(NumericalTolerance::default()) else {
                    continue;
                };
                let angle = surface.angle_to(normal).unwrap().as_radians();
                assert!(
                    angle <= t.normal_angle().as_radians() * slack + 1e-12,
                    "angle {angle}"
                );
            }
        }
    }
}

/// A quarter-round fillet ring: the torus band v in [0, pi/2] around the whole
/// axis, bounded by its outer equator and top circles and one seam use per side.
/// Its minor radius is seven times smaller than its major radius, so the normal
/// turns seven times faster across the band than along it (the Onshape export's
/// band, scaled by 1000).
fn torus_band(major: f64, minor: f64) -> RawBrep {
    let mut r = RawBrep {
        vertices: vec![
            Vertex {
                position: Point::new([major + minor, 0., 0.]).unwrap(),
            },
            Vertex {
                position: Point::new([major, 0., minor]).unwrap(),
            },
        ],
        ..Default::default()
    };
    let circle = |frame, radius| {
        CurveGeometry::Analytic(Curve::circle(frame, Length::metres(radius).unwrap()).unwrap())
    };
    r.edges = vec![
        Edge {
            vertices: [VertexId(0); 2],
            curve: circle(frame3([0.; 3], [1., 0., 0.], [0., 1., 0.]), major + minor),
            range: [0., TAU],
        },
        Edge {
            vertices: [VertexId(1); 2],
            curve: circle(frame3([0., 0., minor], [1., 0., 0.], [0., 1., 0.]), major),
            range: [0., TAU],
        },
        Edge {
            vertices: [VertexId(0), VertexId(1)],
            curve: circle(frame3([major, 0., 0.], [1., 0., 0.], [0., 0., 1.]), minor),
            range: [0., FRAC_PI_2],
        },
    ];
    r.coedges = vec![
        coedge(0, true, line2([0., 0.], [1., 0.], [0., TAU])),
        coedge(2, true, line2([TAU, 0.], [0., 1.], [0., FRAC_PI_2])),
        coedge(1, false, line2([0., FRAC_PI_2], [1., 0.], [0., TAU])),
        coedge(2, false, line2([0., 0.], [0., 1.], [0., FRAC_PI_2])),
    ];
    r.wires.push(Wire {
        coedges: (0..4).map(CoedgeId).collect(),
    });
    r.faces.push(Face {
        surface: SurfaceGeometry::Analytic(
            Surface::torus(
                frame3([0.; 3], [1., 0., 0.], [0., 1., 0.]),
                Length::metres(major).unwrap(),
                Length::metres(minor).unwrap(),
            )
            .unwrap(),
        ),
        outer: WireId(0),
        holes: vec![],
        orientation: Orientation::Forward,
    });
    r
}

#[test]
fn narrow_torus_band_refines_across_the_band() {
    // Longest-edge bisection in an arc-length metric with Lawson flips walked a
    // sliver along this band one vertex per round until the round limit.
    let n = normalize(torus_band(2.035, 0.3));
    let t = tol(0.0436, 0.1);
    let mesh = tessellate_faces(&n, &[FaceId(0)], t, TessellationOptions::default()).unwrap();
    let triangles = mesh.data().triangles.len();
    // Facets about 0.1 rad across the band and along it near the outer equator; the
    // walk failed after more than 8,500 triangles.
    assert!(triangles < 8_000, "{triangles} triangles");
    verify_within(&mesh, &n, t, 1.05);
    let again = tessellate_faces(&n, &[FaceId(0)], t, TessellationOptions::default()).unwrap();
    assert_eq!(mesh.data().triangles, again.data().triangles);
    assert_eq!(mesh.data().positions, again.data().positions);
}

/// A 10 mm plate with a 6 x 6 grid of 0.3 mm holes, the proportions of the Onshape
/// export's perforated cylinder.
const MM: f64 = 1e-3;
fn perforated_plate() -> RawBrep {
    let mut r = polygons(
        &[
            [0., 0., 0.],
            [10. * MM, 0., 0.],
            [10. * MM, 10. * MM, 0.],
            [0., 10. * MM, 0.],
        ],
        &[vec![0, 1, 2, 3]],
    );
    for i in 0..6 {
        for j in 0..6 {
            let c = [(1. + 1.6 * i as f64) * MM, (1. + 1.6 * j as f64) * MM];
            let v = r.vertices.len();
            r.vertices.push(Vertex {
                position: Point::new([c[0] + 0.3 * MM, c[1], 0.]).unwrap(),
            });
            r.edges.push(Edge {
                vertices: [VertexId(v); 2],
                curve: CurveGeometry::Analytic(
                    Curve::circle(
                        frame3([c[0], c[1], 0.], [1., 0., 0.], [0., 1., 0.]),
                        Length::metres(0.3 * MM).unwrap(),
                    )
                    .unwrap(),
                ),
                range: [0., TAU],
            });
            r.coedges
                .push(coedge(r.edges.len() - 1, false, circle2(c, 0.3 * MM)));
            r.wires.push(Wire {
                coedges: vec![CoedgeId(r.coedges.len() - 1)],
            });
            let hole = WireId(r.wires.len() - 1);
            r.faces[0].holes.push(hole);
        }
    }
    r
}

#[test]
fn planar_face_with_many_holes_meshes_within_default_budgets() {
    // Bridging tested every candidate shorter than the best so far in index order,
    // each a linear visibility scan, and ear tests scanned the whole ring: about
    // 25M work units on such a face against the 10M default.
    let n = normalize(perforated_plate());
    let t = tol(4e-5, 0.05);
    let shared = sample_edges(&n, t, SamplingLimits::default()).unwrap();
    let planar = shared
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    let vertices = planar.vertices().len();
    assert!(vertices > 1_500, "{vertices} boundary samples");
    assert_eq!(planar.triangles().len(), vertices + 2 * 36 - 2);
    let mesh = tessellate_faces(&n, &[FaceId(0)], t, TessellationOptions::default()).unwrap();
    assert_eq!(mesh.statistics().boundary_edges, vertices);
    let area: f64 = (100. - 36. * PI * 0.09) * MM * MM;
    let meshed: f64 = mesh
        .data()
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.data().positions[i as usize]);
            0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
        })
        .sum();
    // Inscribed hole polygons add at most the chord times each hole's perimeter.
    assert!(
        meshed > area && meshed < area + 36. * TAU * 0.3 * MM * 4e-5,
        "{meshed}"
    );
}

#[test]
fn tessellation_budgets_are_named_in_diagnostics() {
    let n = normalize(torus_band(2.035, 0.3));
    let t = tol(0.0436, 0.1);
    let defaults = TessellationOptions::default();
    let mut planar = defaults.planar;
    planar.max_vertices = 50;
    let mut sampling = defaults.sampling;
    sampling.max_samples = 10;
    let mut mesh = defaults.mesh;
    mesh.max_triangles = 100;
    for (options, name, limit, plain) in [
        (
            TessellationOptions {
                max_work: 1_000,
                ..defaults
            },
            "max_work",
            1_000,
            true,
        ),
        (
            TessellationOptions {
                max_evaluations: 300,
                ..defaults
            },
            "max_evaluations",
            300,
            false,
        ),
        (
            TessellationOptions {
                max_rounds: 1,
                ..defaults
            },
            "max_rounds",
            1,
            false,
        ),
        (
            TessellationOptions { planar, ..defaults },
            "planar.max_vertices",
            50,
            false,
        ),
        (
            TessellationOptions { mesh, ..defaults },
            "mesh.max_triangles",
            100,
            true,
        ),
    ] {
        let e = tessellate_faces(&n, &[FaceId(0)], t, options).unwrap_err();
        assert_eq!(e.budget, Some(Budget { name, limit }), "{e}");
        // Nested limits (edge sampling, boundary mapping) keep their own kind.
        if plain {
            assert_eq!(e.kind, TessellationErrorKind::ResourceLimit, "{e}");
        }
        assert!(
            e.to_string()
                .contains(&format!("budget {name} {limit} exhausted")),
            "{e}"
        );
    }
    let e = tessellate_faces(
        &n,
        &[FaceId(0)],
        t,
        TessellationOptions {
            sampling,
            ..defaults
        },
    )
    .unwrap_err();
    assert!(matches!(e.kind, TessellationErrorKind::Sampling(_)), "{e}");
    assert_eq!(
        e.budget,
        Some(Budget {
            name: "sampling.max_samples",
            limit: 10
        })
    );
    // Planar triangulation work, on the perforated plate.
    let plate = normalize(perforated_plate());
    let mut planar = defaults.planar;
    planar.max_work = 20_000;
    let e = tessellate_faces(
        &plate,
        &[FaceId(0)],
        tol(4e-5, 0.05),
        TessellationOptions { planar, ..defaults },
    )
    .unwrap_err();
    assert_eq!(
        e.budget,
        Some(Budget {
            name: "planar.max_work",
            limit: 20_000
        }),
        "{e}"
    );
}

/// A unit sphere from pole to pole, cut along one meridian (the seam) and with a
/// round hole whose edge passes close to the south pole: the shape of the
/// Onshape export's B-spline caps. Both pole lines are collapsed edges.
fn holed_sphere() -> RawBrep {
    let sphere = Surface::sphere(
        frame3([0.; 3], [1., 0., 0.], [0., 1., 0.]),
        Length::metres(1.).unwrap(),
    )
    .unwrap();
    let south = [0., 0., -1.];
    let north = [0., 0., 1.];
    let mut r = RawBrep {
        vertices: vec![
            Vertex {
                position: Point::new(south).unwrap(),
            },
            Vertex {
                position: Point::new(north).unwrap(),
            },
        ],
        ..Default::default()
    };
    // The seam meridian u = 0 from the south to the north pole.
    r.edges.push(Edge {
        vertices: [VertexId(0), VertexId(1)],
        curve: CurveGeometry::Analytic(
            Curve::circle(
                frame3([0.; 3], [1., 0., 0.], [0., 0., 1.]),
                Length::metres(1.).unwrap(),
            )
            .unwrap(),
        ),
        range: [-FRAC_PI_2, FRAC_PI_2],
    });
    r.edges.push(Edge {
        vertices: [VertexId(0); 2],
        curve: collapsed_curve(south, TAU),
        range: [0., TAU],
    });
    r.edges.push(Edge {
        vertices: [VertexId(1); 2],
        curve: collapsed_curve(north, TAU),
        range: [0., TAU],
    });
    // Outer loop: south pole line, seam up at u = 2pi, north pole line back, seam down.
    r.coedges = vec![
        coedge(1, true, line2([0., -FRAC_PI_2], [1., 0.], [0., TAU])),
        coedge(0, true, line2([TAU, 0.], [0., 1.], [-FRAC_PI_2, FRAC_PI_2])),
        coedge(2, true, line2([TAU, FRAC_PI_2], [-1., 0.], [0., TAU])),
        coedge(0, false, line2([0., 0.], [0., 1.], [-FRAC_PI_2, FRAC_PI_2])),
    ];
    r.wires.push(Wire {
        coedges: (0..4).map(CoedgeId).collect(),
    });
    // The hole: the latitude circle v = -1.2 for u in [2, 4], closed by meridian
    // arcs up to v = 1.2 and the latitude circle there, so it reaches within 0.37 rad
    // of the south pole and leaves only a narrow strip beside the seam.
    let (lo, hi, a, b) = (-1.2_f64, 1.2_f64, 2.0_f64, 4.0_f64);
    let corner = |u: f64, v: f64| sphere.evaluate(u, v).unwrap().position;
    let first = r.vertices.len();
    for (u, v) in [(a, lo), (b, lo), (b, hi), (a, hi)] {
        r.vertices.push(Vertex {
            position: corner(u, v),
        });
    }
    let latitude = |v: f64| {
        CurveGeometry::Analytic(
            Curve::circle(
                frame3([0., 0., v.sin()], [1., 0., 0.], [0., 1., 0.]),
                Length::metres(v.cos()).unwrap(),
            )
            .unwrap(),
        )
    };
    let meridian = |u: f64| {
        CurveGeometry::Analytic(
            Curve::circle(
                frame3([0.; 3], [u.cos(), u.sin(), 0.], [0., 0., 1.]),
                Length::metres(1.).unwrap(),
            )
            .unwrap(),
        )
    };
    let e = r.edges.len();
    r.edges.extend([
        Edge {
            vertices: [VertexId(first), VertexId(first + 1)],
            curve: latitude(lo),
            range: [a, b],
        },
        Edge {
            vertices: [VertexId(first + 1), VertexId(first + 2)],
            curve: meridian(b),
            range: [lo, hi],
        },
        Edge {
            vertices: [VertexId(first + 3), VertexId(first + 2)],
            curve: latitude(hi),
            range: [a, b],
        },
        Edge {
            vertices: [VertexId(first), VertexId(first + 3)],
            curve: meridian(a),
            range: [lo, hi],
        },
    ]);
    // Clockwise in the chart: up the u = a meridian, along v = hi, down u = b, back.
    let c = r.coedges.len();
    r.coedges.extend([
        coedge(e + 3, true, line2([a, 0.], [0., 1.], [lo, hi])),
        coedge(e + 2, true, line2([0., hi], [1., 0.], [a, b])),
        coedge(e + 1, false, line2([b, 0.], [0., 1.], [lo, hi])),
        coedge(e, false, line2([0., lo], [1., 0.], [a, b])),
    ]);
    r.wires.push(Wire {
        coedges: (c..c + 4).map(CoedgeId).collect(),
    });
    r.faces.push(Face {
        surface: SurfaceGeometry::Analytic(sphere),
        outer: WireId(0),
        holes: vec![WireId(1)],
        orientation: Orientation::Forward,
    });
    r
}

#[test]
fn pole_caps_keep_fans_monotone_beside_seams_and_holes() {
    // Ear clipping in the raw chart fanned one pole copy to consecutive seam samples
    // (wedges of no angle) or to distant hole samples whose rays run backwards; such
    // folds survived every interior split.
    let n = normalize(holed_sphere());
    assert!(n.is_collapsed(EdgeId(1)) && n.is_collapsed(EdgeId(2)));
    let t = tol(0.01, 0.2);
    let mesh = tessellate_faces(&n, &[FaceId(0)], t, TessellationOptions::default()).unwrap();
    verify_within(&mesh, &n, t, 1.05);
    // Each pole welds to one vertex, and the facets around it face outward.
    for pole in [[0., 0., -1.], [0., 0., 1.]] {
        let at: Vec<_> = (0..mesh.data().positions.len())
            .filter(|&i| mesh.data().positions[i] == pole)
            .collect();
        assert_eq!(at.len(), 1);
        for (k, tri) in mesh.data().triangles.iter().enumerate() {
            if tri.contains(&(at[0] as u32)) {
                let [a, b, c] = tri.map(|i| mesh.data().positions[i as usize]);
                let normal = [
                    (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
                    (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
                    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
                ];
                assert!(normal[2] * pole[2] > 0., "pole facet {k} faces inward");
            }
        }
    }
}
