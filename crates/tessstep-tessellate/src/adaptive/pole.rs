//! Polar caps: an explicit fan around each pole line (Milestone 22.2).
//!
//! All samples of a collapsed edge are copies of one singular vertex, and a triangle
//! with one pole copy is the polar wedge between the rays to its two other corners.
//! The 3D fan around the pole is consistent only when those rays advance
//! monotonically around it. Ear clipping in the raw chart does not know that: next
//! to a seam that runs out of the pole it fans one pole copy to consecutive seam
//! samples (wedges of no angle along the seam), or it fans a pole copy to distant
//! samples whose rays run backwards, and such folds survive any interior split.
//!
//! A cap therefore replaces each run of pole copies by a ring of interior vertices
//! parallel to the pole line, through the first boundary samples beside the run and
//! at the run's interior pole-copy coordinates. Between the pole line and the ring
//! the cap is zipped explicitly: each pole-copy interval becomes a collapsed sliver
//! and a wedge between consecutive ring rays, so the fan is monotone by
//! construction. The rest of the face, bounded by the ring, is ear clipped as before.
//! Splitting a pole edge on its ray and splitting ring edges both keep the fan
//! monotone, and flips never touch it. A ring that would leave the face or cross a
//! loop is not built; that run keeps the plain triangulation.
use super::{FaceMemory, Failure, Source, Vertex, Work, orient};
use crate::metric::Sizing;
use tessstep_topology::{FaceId, SurfaceGeometry};

/// A run of pole copies inside a boundary loop and its regular neighbours.
struct Run {
    /// Loop index of the run's first pole copy; the run continues cyclically.
    start: usize,
    len: usize,
}

/// Runs of at least two consecutive pole copies of one pole with regular neighbours.
fn runs(vertices: &[Vertex], ring: &[u32]) -> Vec<Run> {
    let n = ring.len();
    let pole = |i: usize| {
        let v = vertices[ring[i % n] as usize];
        v.free.map(|_| v.source)
    };
    let mut out = Vec::new();
    if n < 4 || (0..n).all(|i| pole(i).is_some()) {
        return out;
    }
    // Start scanning just after a regular vertex so no run wraps the scan start.
    let first = (0..n)
        .find(|&i| pole(i).is_none())
        .expect("a regular vertex")
        + 1;
    let mut i = 0;
    while i < n {
        let at = first + i;
        let Some(source) = pole(at) else {
            i += 1;
            continue;
        };
        let mut len = 1;
        while len < n && pole(at + len) == Some(source) {
            len += 1;
        }
        if len >= 2 && pole(at + len).is_none() {
            out.push(Run { start: at % n, len });
        }
        i += len;
    }
    out
}

/// Cap triangles, and the boundary loops with every capped run replaced by its ring.
pub(super) struct Caps {
    pub triangles: Vec<[u32; 3]>,
    pub loops: Vec<Vec<u32>>,
}
/// Caps every pole run it can. New ring vertices are appended to `vertices`; the
/// caller discards them if the remaining polygon turns out invalid.
#[allow(clippy::too_many_arguments)]
pub(super) fn caps(
    surface: &SurfaceGeometry,
    face: FaceId,
    vertices: &mut Vec<Vertex>,
    boundaries: &[Vec<u32>],
    sizing: Sizing,
    max_vertices: usize,
    memory: &mut FaceMemory,
    work: &mut Work,
) -> Result<Caps, Failure> {
    let mut triangles = Vec::new();
    let mut loops = Vec::with_capacity(boundaries.len());
    for ring in boundaries {
        work.charge(ring.len())?;
        let mut replaced: Vec<Option<Vec<u32>>> = vec![None; ring.len()];
        let mut removed = vec![false; ring.len()];
        let n = ring.len();
        for run in runs(vertices, ring) {
            let at = |j: usize| ring[(run.start + j) % n];
            let a = ring[(run.start + n - 1) % n];
            let b = at(run.len);
            let poles: Vec<u32> = (0..run.len).map(at).collect();
            let p0 = vertices[poles[0] as usize];
            let k = p0.free.expect("pole copy");
            let line = p0.uv[1 - k];
            let (va, vb) = (vertices[a as usize].uv, vertices[b as usize].uv);
            let (da, db) = (va[1 - k] - line, vb[1 - k] - line);
            // Both neighbours must lie off the pole line on the same side.
            let side = da * db;
            if side.is_nan() || side <= 0. {
                continue;
            }
            let offset = if da.abs() <= db.abs() { da } else { db };
            // Ring coordinates: the interior pole copies', strictly between the
            // neighbours' and monotone with the run.
            let along: Vec<f64> = poles[1..poles.len() - 1]
                .iter()
                .map(|&p| vertices[p as usize].uv[k])
                .collect();
            let chain: Vec<f64> = std::iter::once(va[k])
                .chain(along.iter().copied())
                .chain(std::iter::once(vb[k]))
                .collect();
            let rising = chain[chain.len() - 1] > chain[0];
            if !chain
                .windows(2)
                .all(|w| if rising { w[1] > w[0] } else { w[1] < w[0] })
            {
                continue;
            }
            if vertices.len() + along.len() > max_vertices {
                return Err(work.exhaust("planar.max_vertices", max_vertices));
            }
            let mut ring_vertices = vec![a];
            for &value in &along {
                let mut uv = [0.; 2];
                uv[k] = value;
                uv[1 - k] = line + offset;
                let point = memory.regular(surface, uv, sizing, work)?;
                let i = vertices.len();
                vertices.push(Vertex {
                    uv,
                    position: point.position,
                    lifted: point.position,
                    normal: Some(point.normal),
                    source: Source::Interior(face, i),
                    free: None,
                    metric: point.metric,
                });
                ring_vertices.push(i as u32);
            }
            ring_vertices.push(b);
            // Zip: (q_i, p_i, p_{i+1}) is a collapsed sliver, (q_i, p_{i+1}, q_{i+1})
            // the wedge between consecutive ring rays.
            let mut cap = Vec::with_capacity(2 * (poles.len() - 1));
            for i in 0..poles.len() - 1 {
                cap.push([ring_vertices[i], poles[i], poles[i + 1]]);
                cap.push([ring_vertices[i], poles[i + 1], ring_vertices[i + 1]]);
            }
            work.charge(cap.len())?;
            if !cap.iter().all(|t| {
                let [p, q, r] = t.map(|i| vertices[i as usize].uv);
                orient(p, q, r) > 0.
            }) {
                vertices.truncate(vertices.len() - along.len());
                continue;
            }
            triangles.extend(cap);
            for j in 0..run.len {
                removed[(run.start + j) % n] = true;
            }
            replaced[run.start] = Some(ring_vertices[1..ring_vertices.len() - 1].to_vec());
        }
        let mut out = Vec::with_capacity(n);
        for (i, &v) in ring.iter().enumerate() {
            if let Some(inner) = &replaced[i] {
                out.extend_from_slice(inner);
            }
            if !removed[i] {
                out.push(v);
            }
        }
        loops.push(out);
    }
    Ok(Caps { triangles, loops })
}
