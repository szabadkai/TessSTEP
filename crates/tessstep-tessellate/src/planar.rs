use crate::{BoundaryError, BoundarySample, SharedEdges};
use std::collections::{BTreeMap, BinaryHeap};
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
            .map_err(|e| match e {
                BoundaryError::Resolution(e) => PlanarError::InvalidPolygon(e),
                e => PlanarError::Boundary(e),
            })?;
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
        let mut triangles = triangulate_uv(&uv, &boundaries, options, [1.; 2])?;
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

/// Constrained triangulation of UV loops that passed the trim polygon checks at this
/// resolution ([`SharedEdges::face_boundary`]). `metric` scales UV axes when ranking
/// ears by shape; validity predicates always use the unscaled chart.
pub(crate) fn triangulate_uv(
    uv: &[[f64; 2]],
    boundaries: &[Vec<u32>],
    options: PlanarOptions,
    metric: [f64; 2],
) -> Result<Vec<[u32; 3]>, PlanarError> {
    // Polygon validity at the model tolerance is established by trim reconstruction,
    // and the face boundary checks these loops at this resolution. Ear and bridge
    // predicates only need to resolve floating-point sign ambiguity, so their
    // threshold is relative to the polygon extent and coordinate magnitude; the join
    // tolerance would reject every ear of a finely sampled smooth boundary, and the
    // UV tolerance is not a distance in charts whose units are not metres.
    let eps = crate::resolution_tolerance(
        boundaries.iter().flatten().map(|&i| uv[i as usize]),
        options.trim.uv_tolerance,
    )
    .ok_or(PlanarError::UnresolvedGeometry)?;
    // The triangulator still needs simple, correctly nested loops at that resolution.
    let polygons: Vec<Vec<_>> = boundaries
        .iter()
        .map(|poly| poly.iter().map(|&i| uv[i as usize]).collect())
        .collect();
    tessstep_trim::validate_polygons(&polygons[0], &polygons[1..], eps, options.max_work)
        .map_err(PlanarError::InvalidPolygon)?;
    // Every boundary vertex is used once; other entries of `uv` (polar caps) are not.
    let expected = boundaries
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        .checked_add(2 * (boundaries.len() - 1))
        .and_then(|n| n.checked_sub(2))
        .ok_or(PlanarError::ResourceLimit)?;
    if expected > options.max_triangles {
        return Err(PlanarError::ResourceLimit);
    }
    let mut engine = Engine {
        uv: uv.to_vec(),
        loops: boundaries,
        eps,
        work: options.max_work,
    };
    // Holes are bridged into the ring in face order, each to a ring vertex it can
    // see. A bridge may not cross a hole not yet bridged, so a hole that later holes
    // enclose (a pad between two notched pockets, say) sees no ring vertex at all in
    // that order. The remaining holes are then bridged leftmost first: a leftward ray
    // from the leftmost vertex of the leftmost remaining hole reaches the ring, and
    // the ring vertex it finds (the hit edge's endpoint, or the vertex inside that
    // sight triangle at the smallest angle to the ray) is visible from it, since the
    // whole triangle lies at or left of that vertex, where no unbridged hole reaches.
    // Faces whose holes bridge in face order keep that order, and their triangles.
    let mut ring = boundaries[0].clone();
    let mut pending: Vec<&Vec<u32>> = boundaries[1..].iter().collect();
    let mut leftmost_first = false;
    while let Some(&hole) = pending.first() {
        let (hi, ri) = match engine.bridge(hole, &ring) {
            Ok(bridge) => bridge,
            Err(PlanarError::UnresolvedGeometry) if !leftmost_first => {
                leftmost_first = true;
                engine.charge(pending.iter().map(|h| h.len()).sum())?;
                let leftmost = |hole: &[u32]| {
                    hole.iter()
                        .map(|&i| uv[i as usize])
                        .fold([f64::INFINITY; 2], |m, p| if p < m { p } else { m })
                };
                // Ties (a shared leftmost coordinate) keep face order.
                pending.sort_by(|a, b| {
                    leftmost(a)
                        .partial_cmp(&leftmost(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                continue;
            }
            Err(e) => return Err(e),
        };
        pending.remove(0);
        engine.charge(ring.len() + hole.len() + 2)?;
        // Duplicate bridge endpoints in the walk, not in vertex storage.
        let mut joined = Vec::with_capacity(ring.len() + hole.len() + 2);
        joined.extend_from_slice(&ring[..=ri]);
        joined.extend((0..=hole.len()).map(|j| hole[(hi + j) % hole.len()]));
        joined.extend_from_slice(&ring[ri..]);
        ring = joined;
    }
    // Best-first ear clipping: the ear with the largest minimum angle in the metric
    // chart is clipped next, so dense parallel boundaries become strips instead of
    // fans. Ear status only changes at the clipped vertex's neighbours; a full rescan
    // runs if the queue empties early, so no valid ear is ever missed. Ear tests visit
    // only the ring positions a uniform grid places near the ear, not the whole ring.
    // Fitted pcurves of straight or circular edges can be straight in the chart only
    // to about 1e-10 of their length. Clipping across such a run leaves a remainder
    // whose vertices all lie within that wobble of one line: an ear test resolving
    // floating-point sign ambiguity finds no ear in it. Once, the clipping is redone
    // with points within 1e-9 of the boundary extent of a diagonal blocking it, so no
    // diagonal runs that close beside the boundary; no other triangulation changes.
    match clip(&mut engine, &ring, metric, expected) {
        Err(PlanarError::UnresolvedGeometry) => {
            let slack = SLACK * extent(boundaries.iter().flatten().map(|&i| uv[i as usize]));
            if slack.is_nan() || slack <= engine.eps {
                return Err(PlanarError::UnresolvedGeometry);
            }
            engine.eps = slack;
            clip(&mut engine, &ring, metric, expected)
        }
        result => result,
    }
}
/// Retry tolerance of ear clipping, relative to the boundary extent.
const SLACK: f64 = 1e-9;
/// The larger side of the bounding box of `points`.
fn extent(points: impl Iterator<Item = [f64; 2]>) -> f64 {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in points {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    (hi[0] - lo[0]).max(hi[1] - lo[1])
}
/// Best-first ear clipping of a bridged ring into `expected` triangles, verified.
fn clip(
    engine: &mut Engine<'_>,
    ring: &[u32],
    metric: [f64; 2],
    expected: usize,
) -> Result<Vec<[u32; 3]>, PlanarError> {
    let m = ring.len();
    let mut prev: Vec<usize> = (0..m).map(|i| (i + m - 1) % m).collect();
    let mut next: Vec<usize> = (0..m).map(|i| (i + 1) % m).collect();
    let mut alive = vec![true; m];
    engine.charge(m)?;
    let grid = Grid::new(&ring.iter().map(|&i| engine.p(i)).collect::<Vec<_>>());
    let ears = Ears {
        ring,
        grid: &grid,
        metric,
    };
    let mut version = vec![0u32; m];
    let mut remaining = m;
    let mut heap = BinaryHeap::new();
    let rank = |q: f64| q.to_bits();
    for p in 0..m {
        if let Some(q) = engine.ear_quality(&ears, &prev, &next, &alive, p)? {
            heap.push((rank(q), std::cmp::Reverse(p), 0u32));
        }
    }
    let mut triangles = Vec::with_capacity(expected);
    while remaining > 3 {
        let Some((_, std::cmp::Reverse(p), ver)) = heap.pop() else {
            // Removals can expose ears away from the clipped vertex; rescan once.
            let mut found = false;
            for p in 0..m {
                if !alive[p] {
                    continue;
                }
                version[p] += 1;
                if let Some(q) = engine.ear_quality(&ears, &prev, &next, &alive, p)? {
                    heap.push((rank(q), std::cmp::Reverse(p), version[p]));
                    found = true;
                }
            }
            if !found {
                return Err(PlanarError::UnresolvedGeometry);
            }
            continue;
        };
        if !alive[p] || ver != version[p] {
            continue;
        }
        let (a, b) = (prev[p], next[p]);
        triangles.push([ring[a], ring[p], ring[b]]);
        engine.charge(1)?;
        alive[p] = false;
        remaining -= 1;
        next[a] = b;
        prev[b] = a;
        for x in [a, b] {
            version[x] += 1;
            if remaining > 3 {
                if let Some(q) = engine.ear_quality(&ears, &prev, &next, &alive, x)? {
                    heap.push((rank(q), std::cmp::Reverse(x), version[x]));
                }
            }
        }
    }
    let Some(first) = (0..m).find(|&p| alive[p]) else {
        return Err(PlanarError::UnresolvedGeometry);
    };
    let last = [ring[prev[first]], ring[first], ring[next[first]]];
    let [a, b, c] = last.map(|i| engine.p(i));
    let lengths = [distance(a, b)?, distance(b, c)?, distance(c, a)?];
    if orient(a, b, c)? <= engine.eps * lengths.into_iter().fold(0., f64::max) {
        return Err(PlanarError::UnresolvedGeometry);
    }
    triangles.push(last);
    engine.verify(&triangles, expected)?;
    Ok(triangles)
}

/// A bridge candidate: length, hole position, ring position, in that total order.
#[derive(Clone, Copy, PartialEq)]
struct Bridge(f64, usize, usize);
impl Eq for Bridge {}
impl PartialOrd for Bridge {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Bridge {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .total_cmp(&other.0)
            .then(self.1.cmp(&other.1))
            .then(self.2.cmp(&other.2))
    }
}
/// The ring being clipped, its position grid and the ear-ranking metric.
struct Ears<'a> {
    ring: &'a [u32],
    grid: &'a Grid,
    metric: [f64; 2],
}
/// The bounding box of the points within `eps` of all three edge lines of a
/// counterclockwise triangle, with a safety factor; `None` when a corner is too sharp
/// to bound reliably (the caller then scans the whole ring).
fn widened(a: [f64; 2], b: [f64; 2], c: [f64; 2], eps: f64) -> Option<([f64; 2], [f64; 2])> {
    let mut reach = 0_f64;
    for (p, q, r) in [(a, b, c), (b, c, a), (c, a, b)] {
        let u = [q[0] - p[0], q[1] - p[1]];
        let v = [r[0] - p[0], r[1] - p[1]];
        let cos = (u[0] * v[0] + u[1] * v[1]) / (u[0].hypot(u[1]) * v[0].hypot(v[1]));
        let half = (0.5 * (1. - cos)).max(0.).sqrt();
        if half.is_nan() || half <= 1e-6 {
            return None;
        }
        reach = reach.max(eps / half);
    }
    let reach = 2. * reach + eps;
    let lo = [0, 1].map(|k| a[k].min(b[k]).min(c[k]) - reach);
    let hi = [0, 1].map(|k| a[k].max(b[k]).max(c[k]) + reach);
    (lo.iter().chain(&hi).all(|x| x.is_finite())).then_some((lo, hi))
}
/// A uniform grid of ring positions over their bounding box, about one per cell.
struct Grid {
    origin: [f64; 2],
    size: [f64; 2],
    dims: [usize; 2],
    cells: Vec<Vec<usize>>,
}
impl Grid {
    fn new(points: &[[f64; 2]]) -> Self {
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for p in points {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let extent = [0, 1].map(|k| (hi[k] - lo[k]).max(0.));
        let side = (points.len() as f64).sqrt().ceil().max(1.);
        let longest = extent[0].max(extent[1]);
        let dims = [0, 1].map(|k| {
            if longest > 0. && extent[k] > 0. {
                ((side * extent[k] / longest).ceil() as usize).clamp(1, 4096)
            } else {
                1
            }
        });
        let size = [0, 1].map(|k| {
            if extent[k] > 0. {
                extent[k] / dims[k] as f64
            } else {
                1.
            }
        });
        let mut grid = Self {
            origin: lo,
            size,
            dims,
            cells: vec![Vec::new(); dims[0] * dims[1]],
        };
        for (q, p) in points.iter().enumerate() {
            let [i, j] = grid.index(*p);
            grid.cells[j * dims[0] + i].push(q);
        }
        grid
    }
    fn index(&self, p: [f64; 2]) -> [usize; 2] {
        [0, 1].map(|k| {
            let t = ((p[k] - self.origin[k]) / self.size[k]).floor();
            if t.is_finite() && t > 0. {
                (t as usize).min(self.dims[k] - 1)
            } else {
                0
            }
        })
    }
    fn cells(&self, lo: [f64; 2], hi: [f64; 2]) -> impl Iterator<Item = &Vec<usize>> {
        let [i0, j0] = self.index(lo);
        let [i1, j1] = self.index(hi);
        (j0..=j1).flat_map(move |j| (i0..=i1).map(move |i| &self.cells[j * self.dims[0] + i]))
    }
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
    /// The shortest visible bridge from a hole vertex to the current ring, ties broken
    /// by the lowest hole then ring position. Candidates are tested for visibility in
    /// that order, in bounded batches of the shortest untested ones, so the search
    /// stops at the first visible bridge instead of testing every shorter candidate
    /// met in index order (each visibility test is linear in the boundary size).
    fn bridge(&mut self, hole: &[u32], ring: &[u32]) -> Result<(usize, usize), PlanarError> {
        const BATCH: usize = 32;
        let mut tested: Option<Bridge> = None;
        // Vertices an earlier bridge put in the ring twice: their copies are the same
        // point, so visibility cannot tell them apart.
        self.charge(ring.len())?;
        let mut repeated = BTreeMap::<u32, bool>::new();
        for &r in ring {
            repeated
                .entry(r)
                .and_modify(|twice| *twice = true)
                .or_insert(false);
        }
        loop {
            let mut batch = BinaryHeap::with_capacity(BATCH + 1);
            for (hi, &h) in hole.iter().enumerate() {
                for (ri, &r) in ring.iter().enumerate() {
                    self.charge(1)?;
                    let candidate = Bridge(distance(self.p(h), self.p(r))?, hi, ri);
                    if tested.is_some_and(|t| candidate <= t) {
                        continue;
                    }
                    if batch.len() == BATCH {
                        if batch.peek().is_some_and(|worst| candidate >= *worst) {
                            continue;
                        }
                        batch.pop();
                    }
                    batch.push(candidate);
                }
            }
            if batch.is_empty() {
                return Err(PlanarError::UnresolvedGeometry);
            }
            for candidate in batch.into_sorted_vec() {
                let (h, ri) = (hole[candidate.1], candidate.2);
                // At a repeated vertex the bridge must leave the copy whose interior
                // angle contains it; another copy would splice the hole into the ring
                // across a neighbouring bridge or loop, and the ring would fold there.
                let enters = !repeated[&ring[ri]] || self.enters(ring, ri, self.p(h))?;
                if enters && self.visible(h, ring[ri], ring)? {
                    return Ok((candidate.1, candidate.2));
                }
                tested = Some(candidate);
            }
        }
    }
    /// Whether the direction from ring position `ri` towards `x` lies strictly inside
    /// the ring's interior angle there: swept counterclockwise from the edge to the
    /// next position to the edge to the previous one.
    fn enters(&mut self, ring: &[u32], ri: usize, x: [f64; 2]) -> Result<bool, PlanarError> {
        self.charge(1)?;
        let n = ring.len();
        let v = self.p(ring[ri]);
        let next = self.p(ring[(ri + 1) % n]);
        let prev = self.p(ring[(ri + n - 1) % n]);
        let [to_next, to_prev] = [orient(v, next, x)?, orient(v, prev, x)?];
        Ok(if orient(v, next, prev)? > 0. {
            // A convex angle: left of the next edge and right of the previous one.
            to_next > 0. && to_prev < 0.
        } else {
            // A reflex (or straight) angle: outside its convex complement.
            to_next > 0. || to_prev < 0.
        })
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
    /// Ear test at ring position `p` of the linked ring; returns the ear's minimum
    /// angle in the metric chart (radians) when it is a valid ear.
    fn ear_quality(
        &mut self,
        ears: &Ears<'_>,
        prev: &[usize],
        next: &[usize],
        alive: &[bool],
        p: usize,
    ) -> Result<Option<f64>, PlanarError> {
        self.charge(1)?;
        let ring = ears.ring;
        let ids = [ring[prev[p]], ring[p], ring[next[p]]];
        let [a, b, c] = ids.map(|j| self.p(j));
        let lengths = [distance(a, b)?, distance(b, c)?, distance(c, a)?];
        if orient(a, b, c)? <= self.eps * lengths.into_iter().fold(0., f64::max) {
            return Ok(None);
        }
        // Boundary points on a proposed diagonal block the ear: retaining them
        // prevents a T-junction instead of silently dropping collinear samples.
        // Blocking points lie within `eps` of every edge line, hence in the ear's
        // bounding box widened by each corner's offset `eps / sin(angle / 2)`.
        let mut blocked = false;
        let test = |engine: &mut Self, q: usize| -> Result<bool, PlanarError> {
            engine.charge(1)?;
            let j = ring[q];
            if ids.contains(&j) {
                return Ok(false);
            }
            let x = engine.p(j);
            Ok(orient(a, b, x)? >= -engine.eps * lengths[0]
                && orient(b, c, x)? >= -engine.eps * lengths[1]
                && orient(c, a, x)? >= -engine.eps * lengths[2])
        };
        match widened(a, b, c, self.eps) {
            Some((lo, hi)) => {
                for cell in ears.grid.cells(lo, hi) {
                    self.charge(1)?;
                    for &q in cell {
                        if alive[q] && q != prev[p] && q != p && q != next[p] && test(self, q)? {
                            blocked = true;
                            break;
                        }
                    }
                    if blocked {
                        break;
                    }
                }
            }
            None => {
                let mut q = next[next[p]];
                while q != prev[p] {
                    if test(self, q)? {
                        blocked = true;
                        break;
                    }
                    q = next[q];
                }
            }
        }
        if blocked {
            return Ok(None);
        }
        let metric = ears.metric;
        let [a, b, c] = [a, b, c].map(|v| [v[0] * metric[0], v[1] * metric[1]]);
        let angle = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| -> Result<f64, PlanarError> {
            let u = [q[0] - p[0], q[1] - p[1]];
            let v = [r[0] - p[0], r[1] - p[1]];
            let cross = finite(u[0] * v[1] - u[1] * v[0])?;
            let dot = finite(u[0] * v[0] + u[1] * v[1])?;
            Ok(cross.abs().atan2(dot))
        };
        let quality = angle(a, b, c)?.min(angle(b, c, a)?).min(angle(c, a, b)?);
        Ok(Some(finite(quality)?))
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

#[cfg(test)]
mod tests {
    use super::{PlanarOptions, triangulate_uv};

    /// Loops of points as one vertex array and index loops.
    fn indexed(loops: &[Vec<[f64; 2]>]) -> (Vec<[f64; 2]>, Vec<Vec<u32>>) {
        let mut uv = Vec::new();
        let mut boundaries = Vec::new();
        for l in loops {
            let first = uv.len() as u32;
            uv.extend_from_slice(l);
            boundaries.push((first..uv.len() as u32).collect());
        }
        (uv, boundaries)
    }
    fn area(uv: &[[f64; 2]], triangles: &[[u32; 3]]) -> f64 {
        triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| uv[i as usize]);
                0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
            })
            .sum()
    }

    #[test]
    fn a_hole_bridged_to_a_repeated_vertex_leaves_the_copy_that_faces_it() {
        // Hole 1 is bridged from its corner V = (14, 1) to the outer corner (20, 0),
        // so V is in the ring twice. Hole 2's nearest vertex is (14.5, 1), east of V:
        // only the copy of V whose interior angle faces east can take its bridge.
        let outer = vec![[0., 0.], [20., 0.], [20., 6.], [0., 6.]];
        let first = vec![[12., 1.], [12., 4.5], [14., 4.5], [14., 1.]];
        let second = vec![[14.5, 1.], [14.5, 4.5], [17., 4.5], [17., 1.]];
        let (uv, boundaries) = indexed(&[outer, first, second]);
        let triangles =
            triangulate_uv(&uv, &boundaries, PlanarOptions::default(), [1.; 2]).unwrap();
        // Twelve vertices and two holes: 12 + 2 * 2 - 2 triangles covering the area.
        assert_eq!(triangles.len(), 14);
        let expected = 120. - 7. - 8.75;
        assert!((area(&uv, &triangles) - expected).abs() < 1e-9);
    }

    #[test]
    fn a_boundary_straight_to_its_fitting_noise_still_triangulates() {
        // A strip 1.4 wide and 0.0026 high (a cylinder band in radians and metres):
        // its bottom edge, a fitted pcurve of a circle, bends by 3e-11 over its
        // length (9 samples); its top edge is exactly straight (7 samples). Clipping
        // across the bottom leaves a remainder within that bend of one line, in which
        // the first pass finds no ear.
        let mut strip = Vec::new();
        for i in 0..=8 {
            let x = 1.4 * i as f64 / 8.;
            strip.push([x, -2e-10 - 3e-11 * x * (2.8 - x) / 1.96]);
        }
        for i in (0..=6).rev() {
            strip.push([1.4 * i as f64 / 6., 0.0026]);
        }
        let (uv, boundaries) = indexed(&[strip]);
        let triangles =
            triangulate_uv(&uv, &boundaries, PlanarOptions::default(), [1e-3, 1.]).unwrap();
        assert_eq!(triangles.len(), 14);
        assert!((area(&uv, &triangles) - 1.4 * 0.0026).abs() < 1e-9);
    }

    #[test]
    fn a_hole_its_later_holes_enclose_is_bridged_after_them() {
        // A 2 x 2 hole in the middle of a 10 x 10 face, listed first, between an L
        // along the left and bottom sides and an L along the right and top sides:
        // from the middle hole no outer corner is visible past the Ls, so in face
        // order it has no bridge. Bridged leftmost first (the Ls, then it) it has one.
        let outer = vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]];
        let middle = vec![[4., 4.], [4., 6.], [6., 6.], [6., 4.]];
        let lower = vec![[1., 1.], [1., 7.], [2., 7.], [2., 2.], [9., 2.], [9., 1.]];
        let upper = vec![[9., 9.], [9., 3.], [8., 3.], [8., 8.], [1., 8.], [1., 9.]];
        let (uv, boundaries) = indexed(&[outer, middle, lower, upper]);
        let triangles =
            triangulate_uv(&uv, &boundaries, PlanarOptions::default(), [1.; 2]).unwrap();
        // Twenty vertices and three holes: 20 + 2 * 3 - 2 triangles covering the area.
        assert_eq!(triangles.len(), 24);
        assert!((area(&uv, &triangles) - (100. - 4. - 13. - 13.)).abs() < 1e-9);
    }
}
