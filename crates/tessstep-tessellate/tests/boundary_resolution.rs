//! Milestone 22.4: mesh-resolution boundary checks resolve what the triangulator
//! resolves, and their failures name the coedges involved.
#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
use support::*;
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_tessellate::*;
use tessstep_topology::*;

fn normalized(raw: RawBrep) -> NormalizedBrep {
    raw.validate(tolerance(), ValidationLimits::default())
        .unwrap()
        .normalize()
}
fn chord(metres: f64) -> TessellationTolerance {
    TessellationTolerance::new(Length::metres(metres).unwrap(), Angle::radians(1.).unwrap())
        .unwrap()
}

#[test]
fn features_below_the_uv_tolerance_triangulate() {
    // A 5 µm edge in a metre chart, below the 1e-5 UV chord tolerance, but far above
    // the floating-point resolution of the polygon and the 10 nm model tolerance.
    let n = normalized(polygons(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [1. - 5e-6, 1., 0.],
            [0., 1., 0.],
        ],
        &[vec![0, 1, 2, 3, 4]],
    ));
    let s = sample_edges(&n, chord(1e-3), SamplingLimits::default()).unwrap();
    let mesh = s
        .triangulate_planar(FaceId(0), PlanarOptions::default())
        .unwrap();
    assert_eq!(mesh.triangles().len(), 3);
    let mesh = tessellate_faces(
        &n,
        &[FaceId(0)],
        chord(1e-3),
        TessellationOptions::default(),
    )
    .unwrap();
    let d = mesh.data();
    let area: f64 = d
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| d.positions[i as usize]);
            0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
        })
        .sum();
    assert!((area - 1.).abs() < 1e-12, "{area}");
}

#[test]
fn coarse_boundaries_fail_at_their_coedges() {
    // A unit disk with a square hole: at a 0.4 m chord and a 1 rad angle the rim is a
    // square whose side x + y = 1 cuts through the hole, which the exact rim does not.
    let mut raw = disk();
    let h = polygons(
        &[
            [0.45, 0.45, 0.],
            [0.45, 0.6, 0.],
            [0.6, 0.6, 0.],
            [0.6, 0.45, 0.],
        ],
        &[vec![0, 1, 2, 3]],
    );
    raw.vertices.extend(h.vertices);
    raw.edges.extend(h.edges.into_iter().map(|mut e| {
        e.vertices = e.vertices.map(|v| VertexId(v.0 + 1));
        e
    }));
    raw.coedges.extend(h.coedges.into_iter().map(|mut c| {
        c.edge.0 += 1;
        c
    }));
    raw.wires.push(Wire {
        coedges: (1..5).map(CoedgeId).collect(),
    });
    raw.faces[0].holes.push(WireId(1));
    let n = normalized(raw);
    let s = sample_edges(&n, chord(0.4), SamplingLimits::default()).unwrap();
    let located = |e: &tessstep_trim::Error| {
        assert_eq!(e.kind, tessstep_trim::ErrorKind::IntersectingLoops);
        // The rim (coedge 0) against one of the hole's coedges, in the hole's wire.
        assert_eq!(e.coedge, Some(CoedgeId(0)));
        assert!(matches!(e.other, Some(CoedgeId(1..=4))));
        assert_eq!(e.wire, Some(WireId(1)));
    };
    match s.face_boundary(FaceId(0), tessstep_trim::Options::default(), 10_000) {
        Err(BoundaryError::Resolution(e)) => located(&e),
        other => panic!("{other:?}"),
    }
    match s.triangulate_planar(FaceId(0), PlanarOptions::default()) {
        Err(PlanarError::InvalidPolygon(e)) => located(&e),
        other => panic!("{other:?}"),
    }
}
