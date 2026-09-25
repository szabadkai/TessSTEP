use crate::{BoundaryError, BoundarySample, SharedEdges};
use std::collections::BTreeMap;
use tessstep_surfaces::SurfaceKind;
use tessstep_topology::{FaceId, SurfaceGeometry};

#[derive(Clone, Copy, Debug)]
pub struct PlanarOptions {
    /// Independent reconstruction/validation budget for supplied pcurves.
    pub trim: tessstep_trim::Options,
    pub max_vertices: usize,
    pub max_triangles: usize,
    /// Work budget for polygon validation and, separately, triangulation.
    pub max_work: usize,
}
impl Default for PlanarOptions {
    fn default() -> Self {
        Self {
            trim: tessstep_trim::Options::default(),
            max_vertices: 100_000,
            max_triangles: 200_000,
            max_work: 10_000_000,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum PlanarError {
    InvalidOptions,
    InvalidFace,
    UnsupportedSurface,
    Boundary(BoundaryError),
    InvalidPolygon(tessstep_trim::Error),
    ResourceLimit,
    /// Finite predicates cannot resolve a valid nondegenerate triangulation.
    UnresolvedGeometry,
}
impl std::fmt::Display for PlanarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "planar triangulation: {self:?}")
    }
}
impl std::error::Error for PlanarError {}

/// Immutable face-local triangles borrowing the canonical edge cache. Every vertex
/// is a retained boundary sample; no Steiner vertices, welding or boundary edits.
/// This is a Rust face result, not a public C layout or an assembled solid mesh.
///
/// A mesh cannot outlive its shared-edge cache:
/// ```compile_fail
/// use tessstep_tessellate::{PlanarMesh, PlanarOptions, SharedEdges};
/// use tessstep_topology::FaceId;
/// fn escape<'a>(cache: &SharedEdges<'a>) -> PlanarMesh<'a> {
///     cache.triangulate_planar(FaceId(0), PlanarOptions::default()).unwrap()
/// }
/// ```
#[derive(Debug)]
pub struct PlanarMesh<'a> {
    face: FaceId,
    vertices: Vec<BoundarySample<'a>>,
    triangles: Vec<[u32; 3]>,
    boundaries: Vec<Vec<u32>>,
    normal: [f64; 3],
}
impl<'a> PlanarMesh<'a> {
    pub fn face(&self) -> FaceId {
        self.face
    }
    pub fn vertices(&self) -> &[BoundarySample<'a>] {
        &self.vertices
    }
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// Outer loop followed by holes, in supplied UV winding (even for reversed faces).
    pub fn boundaries(&self) -> &[Vec<u32>] {
        &self.boundaries
    }
    /// Unit normal with the face orientation applied.
    pub fn normal(&self) -> [f64; 3] {
        self.normal
    }
}

impl SharedEdges<'_> {
    /// Triangulate an analytic plane using the already sampled shared edges. Concave
    /// boundaries and disjoint holes are supported. Other surface kinds fail explicitly.
    pub fn triangulate_planar(
        &self,
        face: FaceId,
        options: PlanarOptions,
    ) -> Result<PlanarMesh<'_>, PlanarError> {
        if options.max_vertices > 1_000_000 || options.max_triangles > 2_000_000 {
            return Err(PlanarError::InvalidOptions);
        }
        let f = self
            .brep
            .data()
            .faces
            .get(face.0)
            .ok_or(PlanarError::InvalidFace)?;
        if !matches!(&f.surface, SurfaceGeometry::Analytic(s) if s.kind() == SurfaceKind::Plane) {
            return Err(PlanarError::UnsupportedSurface);
        }
        // Each use includes a repeated endpoint; the map's budget includes those too.
        let boundary = self
            .face_boundary(face, options.trim, options.max_vertices.saturating_mul(2))
            .map_err(PlanarError::Boundary)?;
        let mut vertices = Vec::new();
        let mut boundaries = Vec::new();
        for uses in std::iter::once(&boundary.outer).chain(&boundary.holes) {
            let mut indices = Vec::new();
            for use_ in uses {
                for sample in &use_.samples[..use_.samples.len() - 1] {
                    if vertices.len() >= options.max_vertices {
                        return Err(PlanarError::ResourceLimit);
                    }
                    indices.push(vertices.len() as u32);
                    vertices.push(*sample);
                }
            }
            boundaries.push(indices);
        }
        let uv: Vec<_> = vertices.iter().map(|s| s.uv).collect();
        let mut triangles = triangulate_uv(&uv, &boundaries, options)?;
        let jet = f
            .surface
            .evaluate(vertices[0].uv)
            .map_err(|_| PlanarError::UnresolvedGeometry)?;
        let mut normal = jet
            .normal(tessstep_math::NumericalTolerance::default())
            .map_err(|_| PlanarError::UnresolvedGeometry)?
            .vector()
            .components();
        // Canonical vertex snapping must not collapse or invert a triangle in 3D.
        for tri in &triangles {
            let [a, b, c] = tri.map(|i| vertices[i as usize].edge_sample.position);
            let cross = b
                .difference(a)
                .and_then(|ab| c.difference(a).and_then(|ac| ab.cross(ac)))
                .map_err(|_| PlanarError::UnresolvedGeometry)?;
            let dot = cross
                .components()
                .iter()
                .zip(normal)
                .map(|(a, b)| a * b)
                .sum();
            if finite(dot)? <= 0. {
                return Err(PlanarError::UnresolvedGeometry);
            }
        }
        if f.orientation.is_reversed() {
            normal = normal.map(|x| -x);
            for tri in &mut triangles {
                tri.swap(1, 2);
            }
        }
        Ok(PlanarMesh {
            face,
            vertices,
            triangles,
            boundaries,
            normal,
        })
    }
}

pub(crate) fn triangulate_uv(
    uv: &[[f64; 2]],
    boundaries: &[Vec<u32>],
    options: PlanarOptions,
) -> Result<Vec<[u32; 3]>, PlanarError> {
    let polygons: Vec<Vec<_>> = boundaries
        .iter()
        .map(|poly| poly.iter().map(|&i| uv[i as usize]).collect())
        .collect();
    tessstep_trim::validate_polygons(
        &polygons[0],
        &polygons[1..],
        options.trim.uv_tolerance,
        options.max_work,
    )
    .map_err(PlanarError::InvalidPolygon)?;
    let expected = uv
        .len()
        .checked_add(2 * (boundaries.len() - 1))
        .and_then(|n| n.checked_sub(2))
        .ok_or(PlanarError::ResourceLimit)?;
    if expected > options.max_triangles {
        return Err(PlanarError::ResourceLimit);
    }
    let mut engine = Engine {
        uv: uv.to_vec(),
        loops: boundaries,
        eps: options.trim.uv_tolerance,
        work: options.max_work,
    };
    let mut ring = boundaries[0].clone();
    for hole in &boundaries[1..] {
        let mut best: Option<(f64, usize, usize)> = None;
        for (hi, &h) in hole.iter().enumerate() {
            for (ri, &r) in ring.iter().enumerate() {
                engine.charge(1)?;
                let length = distance(engine.p(h), engine.p(r))?;
                if best.is_some_and(|(d, _, _)| length >= d) {
                    continue;
                }
                if engine.visible(h, r, &ring)? {
                    best = Some((length, hi, ri));
                }
            }
        }
        let (_, hi, ri) = best.ok_or(PlanarError::UnresolvedGeometry)?;
        engine.charge(ring.len() + hole.len() + 2)?;
        // Duplicate bridge endpoints in the walk, not in vertex storage.
        let mut joined = Vec::with_capacity(ring.len() + hole.len() + 2);
        joined.extend_from_slice(&ring[..=ri]);
        joined.extend((0..=hole.len()).map(|j| hole[(hi + j) % hole.len()]));
        joined.extend_from_slice(&ring[ri..]);
        ring = joined;
    }
    let mut triangles = Vec::with_capacity(expected);
    while ring.len() > 3 {
        let mut ear = None;
        for i in 0..ring.len() {
            if engine.ear(&ring, i)? {
                ear = Some(i);
                break;
            }
        }
        let i = ear.ok_or(PlanarError::UnresolvedGeometry)?;
        triangles.push([
            ring[(i + ring.len() - 1) % ring.len()],
            ring[i],
            ring[(i + 1) % ring.len()],
        ]);
        engine.charge(ring.len())?;
        ring.remove(i);
    }
    if !engine.ear(&ring, 1)? {
        return Err(PlanarError::UnresolvedGeometry);
    }
    triangles.push([ring[0], ring[1], ring[2]]);
    engine.verify(&triangles, expected)?;
    Ok(triangles)
}

struct Engine<'a> {
    uv: Vec<[f64; 2]>,
    loops: &'a [Vec<u32>],
    eps: f64,
    work: usize,
}
fn finite(x: f64) -> Result<f64, PlanarError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(PlanarError::UnresolvedGeometry)
    }
}
fn distance(a: [f64; 2], b: [f64; 2]) -> Result<f64, PlanarError> {
    finite((b[0] - a[0]).hypot(b[1] - a[1]))
}
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Result<f64, PlanarError> {
    finite((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
}
fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> Result<f64, PlanarError> {
    let length = distance(a, b)?;
    if length == 0. {
        return distance(p, a);
    }
    let d = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
    let t = finite((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1])?.clamp(0., length);
    distance(p, [a[0] + t * d[0], a[1] + t * d[1]])
}
impl Engine<'_> {
    fn p(&self, i: u32) -> [f64; 2] {
        self.uv[i as usize]
    }
    fn charge(&mut self, n: usize) -> Result<(), PlanarError> {
        self.work = self.work.checked_sub(n).ok_or(PlanarError::ResourceLimit)?;
        Ok(())
    }
    fn blocked(&mut self, a: u32, b: u32, c: u32, d: u32) -> Result<bool, PlanarError> {
        self.charge(1)?;
        if (a == c && b == d) || (a == d && b == c) {
            return Ok(true);
        }
        // Incident edges may share only the endpoint, not overlap/backtrack.
        for (p, x, y) in [(a, c, d), (b, c, d), (c, a, b), (d, a, b)] {
            if p != x && p != y && segment_distance(self.p(p), self.p(x), self.p(y))? <= self.eps {
                return Ok(true);
            }
        }
        if a == c || a == d || b == c || b == d {
            return Ok(false);
        }
        let [a, b, c, d] = [a, b, c, d].map(|i| self.p(i));
        Ok(orient(a, b, c)?.signum() != orient(a, b, d)?.signum()
            && orient(c, d, a)?.signum() != orient(c, d, b)?.signum())
    }
    fn inside(&mut self, p: [f64; 2]) -> Result<bool, PlanarError> {
        // Even/odd crossing over the validated outer and disjoint hole boundaries.
        let mut inside = false;
        for poly in self.loops {
            for i in 0..poly.len() {
                self.charge(1)?;
                let a = self.p(poly[i]);
                let b = self.p(poly[(i + 1) % poly.len()]);
                if segment_distance(p, a, b)? <= self.eps {
                    return Ok(false);
                }
                if (a[1] > p[1]) != (b[1] > p[1]) && (orient(a, b, p)? > 0.) == (b[1] > a[1]) {
                    inside = !inside;
                }
            }
        }
        Ok(inside)
    }
    fn visible(&mut self, a: u32, b: u32, ring: &[u32]) -> Result<bool, PlanarError> {
        for poly in self.loops {
            for i in 0..poly.len() {
                if self.blocked(a, b, poly[i], poly[(i + 1) % poly.len()])? {
                    return Ok(false);
                }
            }
        }
        for i in 0..ring.len() {
            if self.blocked(a, b, ring[i], ring[(i + 1) % ring.len()])? {
                return Ok(false);
            }
        }
        let p = self.p(a);
        let q = self.p(b);
        self.inside([p[0] * 0.5 + q[0] * 0.5, p[1] * 0.5 + q[1] * 0.5])
    }
    fn ear(&mut self, ring: &[u32], i: usize) -> Result<bool, PlanarError> {
        self.charge(1)?;
        let ids = [
            ring[(i + ring.len() - 1) % ring.len()],
            ring[i],
            ring[(i + 1) % ring.len()],
        ];
        let [a, b, c] = ids.map(|j| self.p(j));
        let lengths = [distance(a, b)?, distance(b, c)?, distance(c, a)?];
        if orient(a, b, c)? <= self.eps * lengths.into_iter().fold(0., f64::max) {
            return Ok(false);
        }
        // Boundary points on a proposed diagonal block the ear: retaining them
        // prevents a T-junction instead of silently dropping collinear samples.
        for &j in ring {
            self.charge(1)?;
            if ids.contains(&j) {
                continue;
            }
            let p = self.p(j);
            if orient(a, b, p)? >= -self.eps * lengths[0]
                && orient(b, c, p)? >= -self.eps * lengths[1]
                && orient(c, a, p)? >= -self.eps * lengths[2]
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn verify(&mut self, triangles: &[[u32; 3]], expected: usize) -> Result<(), PlanarError> {
        if triangles.len() != expected {
            return Err(PlanarError::UnresolvedGeometry);
        }
        let mut edges = BTreeMap::<(u32, u32), (usize, i32)>::new();
        let mut area = 0.;
        for &[a, b, c] in triangles {
            self.charge(3)?;
            area = finite(area + orient(self.p(a), self.p(b), self.p(c))?)?;
            for (u, v) in [(a, b), (b, c), (c, a)] {
                let e = edges.entry((u.min(v), u.max(v))).or_default();
                e.0 += 1;
                e.1 += if u < v { 1 } else { -1 };
            }
        }
        let mut polygon_area = 0.;
        for poly in self.loops {
            for i in 0..poly.len() {
                self.charge(1)?;
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                if edges.remove(&(a.min(b), a.max(b))) != Some((1, if a < b { 1 } else { -1 })) {
                    return Err(PlanarError::UnresolvedGeometry);
                }
                polygon_area =
                    finite(polygon_area + orient(self.p(poly[0]), self.p(a), self.p(b))?)?;
            }
        }
        if edges.values().any(|&e| e != (2, 0))
            || (area - polygon_area).abs()
                > finite(1e-10 * polygon_area.abs() + self.eps * self.eps * expected as f64)?
        {
            return Err(PlanarError::UnresolvedGeometry);
        }
        Ok(())
    }
}
