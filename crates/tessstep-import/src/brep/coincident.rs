//! Faces that lie on a neighbouring face's surface: the lens between them has no
//! thickness the model resolves, so the face is merged into its neighbour before
//! planning and counted as a collapsed face.
use super::*;
use crate::pcurve::{cross, surface_jet};

/// Samples per edge of the coincidence and area tests.
const SAMPLES: usize = 5;

/// One merge: face `face` lies on the surface of `neighbour`, whose loop
/// `loop_index` runs the shared chain at positions `run_start..run_start + run_len`
/// (cyclic); `chain` is the rest of the face's loop, which replaces the run, and
/// `removed` are the shared chain's edges, used by no face afterwards.
struct Merge {
    face: usize,
    neighbour: usize,
    loop_index: usize,
    run_start: usize,
    run_len: usize,
    chain: Vec<Use>,
    removed: Vec<usize>,
}

impl Builder {
    /// Merge every face bounded by one loop that splits into a chain `S` shared with
    /// one neighbour `C` and a chain `T` shared with other faces, whose sampled
    /// boundary and interior lie within the model `tolerance` of `C`'s surface: `C`'s loop then runs
    /// along `T` instead of `S`, the face and the edges of `S` are removed, and the
    /// shell stays closed with `T`'s edges shared by `C` and their other faces. The
    /// solid loses only the lens between the face and `C`, which is thinner than the
    /// tolerance; no coordinate, curve or surface changes. An exporter wrote a 49 µm
    /// strip of a plane that cuts a 10 mm fillet cylinder 0.03 µm below its tangent
    /// line: the plane face and the cylinder's strip meshed into the same triangles.
    pub(super) fn merge_coincident(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &mut Vec<FaceRecord>,
        tolerance: f64,
    ) -> Result<(), Error> {
        // The pass has a planning pass's pcurve budget for its inversions.
        self.work.remaining = self.pass_work;
        while let Some(merge) = self.find_coincident(c, faces, tolerance)? {
            self.apply_merge(c, faces, merge)?;
        }
        Ok(())
    }

    fn find_coincident(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &[FaceRecord],
        tolerance: f64,
    ) -> Result<Option<Merge>, Error> {
        // Every use of each edge: (face, loop, position).
        let mut uses: BTreeMap<usize, Vec<(usize, usize, usize)>> = BTreeMap::new();
        for (f, face) in faces.iter().enumerate() {
            for (l, lp) in face.loops.iter().enumerate() {
                for (i, u) in lp.uses.iter().enumerate() {
                    c.charge(1)?;
                    uses.entry(u.edge).or_default().push((f, l, i));
                }
            }
        }
        for (p, face) in faces.iter().enumerate() {
            let [lp] = face.loops.as_slice() else {
                continue;
            };
            let n = lp.uses.len();
            if lp.vertex.is_some() || n < 2 {
                continue;
            }
            c.charge(n)?;
            // The use on the other side of each of the loop's uses, in another face.
            let mut partners = Vec::with_capacity(n);
            for u in &lp.uses {
                let all = &uses[&u.edge];
                partners.push(match all.as_slice() {
                    [a, b] if a.0 != p && b.0 == p => *a,
                    [a, b] if b.0 != p && a.0 == p => *b,
                    _ => break,
                });
            }
            if partners.len() < n {
                continue;
            }
            let mut candidates: Vec<usize> = partners.iter().map(|x| x.0).collect();
            candidates.sort_unstable();
            candidates.dedup();
            for q in candidates {
                c.charge(n)?;
                let shared: Vec<usize> = (0..n).filter(|&i| partners[i].0 == q).collect();
                let k = shared.len();
                if k == n {
                    continue;
                }
                // The shared uses are one cyclic run of the loop, S; the rest is T.
                let Some(start) = shared
                    .iter()
                    .copied()
                    .find(|&s| !shared.contains(&((s + n - 1) % n)))
                else {
                    continue;
                };
                if !(0..k).all(|j| shared.contains(&((start + j) % n))) {
                    continue;
                }
                let s_uses: Vec<Use> = (0..k).map(|j| lp.uses[(start + j) % n]).collect();
                let t_uses: Vec<Use> = (k..n).map(|j| lp.uses[(start + j) % n]).collect();
                // In the neighbour's loop the run is S reversed, flipped and contiguous.
                let (_, ql, run_start) = partners[(start + k - 1) % n];
                let qloop = &faces[q].loops[ql];
                let m = qloop.uses.len();
                if k >= m {
                    continue;
                }
                let run = (0..k).all(|j| {
                    let (_, l, i) = partners[(start + k - 1 - j) % n];
                    let s = s_uses[k - 1 - j];
                    let u = qloop.uses[i];
                    l == ql
                        && i == (run_start + j) % m
                        && u.edge == s.edge
                        && u.forward != s.forward
                });
                if !run {
                    continue;
                }
                // The face is the smaller region: two faces on one plane satisfy the
                // surface test both ways, and the neighbour merged into the face would
                // wind the wrong way around it.
                match (self.loop_area(&lp.uses), self.loop_area(&qloop.uses)) {
                    (Some(a), Some(b)) if a < b => {}
                    _ => continue,
                }
                if !self.lies_on(c, face, &faces[q], &s_uses, &t_uses, tolerance)? {
                    continue;
                }
                return Ok(Some(Merge {
                    face: p,
                    neighbour: q,
                    loop_index: ql,
                    run_start,
                    run_len: k,
                    chain: t_uses,
                    removed: s_uses.iter().map(|u| u.edge).collect(),
                }));
            }
        }
        Ok(None)
    }

    /// The area of a loop's sampled 3D polygon (its vector area, the exact area of
    /// a planar loop), or `None` when an edge cannot be evaluated.
    fn loop_area(&self, uses: &[Use]) -> Option<f64> {
        let mut points = Vec::with_capacity(uses.len() * SAMPLES);
        for u in uses {
            let e = &self.edges[u.edge];
            for i in 0..SAMPLES {
                let f = i as f64 / SAMPLES as f64;
                let f = if u.forward { f } else { 1. - f };
                let t = e.range[0] + (e.range[1] - e.range[0]) * f;
                points.push(e.curve.evaluate(t).ok()?.position.coordinates());
            }
        }
        let mut twice = [0.; 3];
        for (i, &p) in points.iter().enumerate() {
            twice = add(twice, cross(p, points[(i + 1) % points.len()]));
        }
        Some(0.5 * norm(twice))
    }

    /// Whether `face`, sampled along the chains `s` and `t` of its loop and over its
    /// own surface between them, lies within `tolerance` of `neighbour`'s surface.
    fn lies_on(
        &mut self,
        c: &mut Context<'_, '_>,
        face: &FaceRecord,
        neighbour: &FaceRecord,
        s: &[Use],
        t: &[Use],
        tolerance: f64,
    ) -> Result<bool, Error> {
        let points = |edges: &Vec<EdgeRecord>, uses: &[Use]| -> Option<Vec<V3>> {
            let mut out = Vec::with_capacity(uses.len() * (SAMPLES + 1));
            for u in uses {
                let e = &edges[u.edge];
                for i in 0..=SAMPLES {
                    let t = e.range[0] + (e.range[1] - e.range[0]) * i as f64 / SAMPLES as f64;
                    out.push(e.curve.evaluate(t).ok()?.position.coordinates());
                }
            }
            Some(out)
        };
        let (Some(on_s), Some(on_t)) = (points(&self.edges, s), points(&self.edges, t)) else {
            return Ok(false);
        };
        // A face whose other chain runs along the shared one is a sliver, not a lens:
        // it is thinner than the tolerance on its own and is left as written.
        let sliver = on_t.iter().all(|&x| {
            on_s.windows(2).any(|w| {
                let d = sub(w[1], w[0]);
                let len2 = dot(d, d);
                let u = if len2 > 0. {
                    (dot(sub(x, w[0]), d) / len2).clamp(0., 1.)
                } else {
                    0.
                };
                norm(sub(x, add(w[0], scale(d, u)))) <= tolerance
            })
        });
        if sliver {
            return Ok(false);
        }
        // The nearest point of a face's surface to `x`, its distance and the face's
        // outward normal there (the surface normal, reversed for a reversed face).
        let nearest = |f: &FaceRecord,
                       x: V3,
                       work: &mut pcurve::Work|
         -> Result<(V3, f64, V3), PcurveError> {
            let inverter = Inverter {
                chart: &f.chart,
                surface: &f.surface,
                tolerance,
            };
            let uv = inverter.invert(x, None, work)?;
            let jet = surface_jet(&f.surface, uv, work)?;
            let normal = scale(cross(jet.du, jet.dv), if f.same { 1. } else { -1. });
            Ok((jet.p, norm(sub(jet.p, x)), normal))
        };
        let check = |work: &mut pcurve::Work| -> Result<bool, PcurveError> {
            // The boundary first: `T` leaves the surface at once for most faces.
            for &x in on_t.iter().chain(&on_s) {
                if nearest(neighbour, x, work)?.1 > tolerance {
                    return Ok(false);
                }
            }
            // The interior, on the face's own surface between the chains. The two
            // faces must face away from each other there: a face that continues
            // its neighbour on the same surface, facing the same way, is not a lens.
            for &a in &on_s {
                for &b in &on_t {
                    let (y, _, outward) = nearest(face, scale(add(a, b), 0.5), work)?;
                    let (_, distance, other) = nearest(neighbour, y, work)?;
                    if distance > tolerance || dot(outward, other) >= 0. {
                        return Ok(false);
                    }
                }
            }
            Ok(true)
        };
        match check(&mut self.work) {
            Ok(lies) => Ok(lies),
            Err(PcurveError::Limit) => Err(self.pcurve_error(c, t[0].edge, PcurveError::Limit)),
            Err(_) => Ok(false),
        }
    }

    fn apply_merge(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &mut Vec<FaceRecord>,
        merge: Merge,
    ) -> Result<(), Error> {
        c.current = faces[merge.face].entity;
        c.record()?;
        let lp = &mut faces[merge.neighbour].loops[merge.loop_index];
        let n = lp.uses.len();
        let mut rebuilt = Vec::with_capacity(n - merge.run_len + merge.chain.len());
        for j in 0..n - merge.run_len {
            rebuilt.push(lp.uses[(merge.run_start + merge.run_len + j) % n]);
        }
        // `T` runs from the end of `S` back to its start in the face's loop, the
        // direction the neighbour traverses `S` reversed, so it keeps its uses.
        rebuilt.extend_from_slice(&merge.chain);
        lp.uses = rebuilt;
        faces.remove(merge.face);
        self.adaptations.collapsed_faces += 1;
        // Compact the edge table, then the vertices only `S` used.
        let mut remap: Vec<Option<usize>> = Vec::with_capacity(self.edges.len());
        let mut compact = Vec::new();
        for (index, edge) in self.edges.drain(..).enumerate() {
            if merge.removed.contains(&index) {
                remap.push(None);
            } else {
                remap.push(Some(compact.len()));
                compact.push(edge);
            }
        }
        self.edges = compact;
        let target = |edge: usize| remap[edge].expect("only the shared chain's edges are unused");
        for face in faces.iter_mut() {
            for l in &mut face.loops {
                for u in &mut l.uses {
                    u.edge = target(u.edge);
                }
            }
        }
        self.edge_index
            .retain(|_, (edge, _)| remap[*edge].is_some());
        for (edge, _) in self.edge_index.values_mut() {
            *edge = target(*edge);
        }
        let mut used = vec![false; c.raw.vertices.len()];
        for e in &self.edges {
            for v in e.vertices {
                used[v.0] = true;
            }
        }
        for v in faces.iter().flat_map(|f| &f.loops).filter_map(|l| l.vertex) {
            used[v.0] = true;
        }
        if used.iter().all(|&u| u) {
            return Ok(());
        }
        let mut index = Vec::with_capacity(used.len());
        let mut next = 0;
        for &u in &used {
            index.push(u.then(|| {
                next += 1;
                VertexId(next - 1)
            }));
        }
        let vertex = |v: VertexId| index[v.0].expect("used vertices are kept");
        let vertices = std::mem::take(&mut c.raw.vertices);
        c.raw.vertices = vertices
            .into_iter()
            .zip(&used)
            .filter(|(_, u)| **u)
            .map(|(v, _)| v)
            .collect();
        let entities = std::mem::take(&mut c.point_entities);
        c.point_entities = entities
            .into_iter()
            .zip(&used)
            .filter(|(_, u)| **u)
            .map(|(e, _)| e)
            .collect();
        for e in &mut self.edges {
            e.vertices = e.vertices.map(vertex);
        }
        for l in faces.iter_mut().flat_map(|f| &mut f.loops) {
            if let Some(v) = &mut l.vertex {
                *v = vertex(*v);
            }
        }
        c.points.retain(|_, v| used[v.0]);
        for v in c.points.values_mut() {
            *v = vertex(*v);
        }
        Ok(())
    }
}
