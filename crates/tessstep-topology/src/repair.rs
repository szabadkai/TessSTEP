//! Opt-in, tolerance-bounded repair of raw B-reps, applied explicitly by a caller
//! before validation; validation itself never repairs. Sewing merges coincident
//! boundary vertices and free edges; orientation flips faces so each shell is
//! consistently oriented. Coordinates, curves and surfaces are never changed: a merged
//! vertex keeps its representative's position and a merged edge its survivor's curve,
//! so a sewn model is validated at the model tolerance plus the largest merge distance.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use tessstep_curves::CurveKind;

/// A repair that could not be made, located at the defect it leaves in place.
#[derive(Clone, Debug, PartialEq)]
pub struct RepairError {
    pub report: Report,
    /// The edge at the defect, and the nearest edge it could be sewn to.
    pub edge: Option<EdgeId>,
    pub partner: Option<EdgeId>,
    /// The sewing tolerance the repair would need, metres; `None` when no tolerance
    /// would do (no candidate, a non-orientable shell or an exhausted work budget).
    pub needed: Option<f64>,
    /// The sewing tolerance in force, metres.
    pub tolerance: f64,
    pub reason: &'static str,
}
impl std::fmt::Display for RepairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.report, self.reason)?;
        if let Some(needed) = self.needed {
            write!(
                f,
                " (needs {needed:.3e} m; sewing tolerance {:.3e} m)",
                self.tolerance
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for RepairError {}

/// The merges chosen by [`plan_sewing`]. Apply them with [`SewPlan::apply`], or map
/// them onto other records (an importer may sew before it computes pcurves).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SewPlan {
    /// The representative of every vertex: itself, or the lowest-indexed vertex of
    /// its merged cluster, whose position the cluster keeps.
    pub vertices: Vec<VertexId>,
    /// The surviving edge of every edge, and whether a use moved onto it reverses.
    pub edges: Vec<(EdgeId, bool)>,
    pub merged_vertices: usize,
    pub merged_edges: usize,
    /// The largest measured distance between merged vertices (to their
    /// representative) or between merged edge curves, metres.
    pub max_distance: f64,
    /// Closed shells of at least two faces whose faces shared no edge or vertex
    /// before sewing (unsewn, or disconnected, closed shells).
    pub disconnected_shells: usize,
}
impl SewPlan {
    pub fn is_empty(&self) -> bool {
        self.merged_vertices == 0 && self.merged_edges == 0
    }
    /// Retarget every use onto the surviving edges and vertices and drop the merged
    /// records. A use moved onto an edge running the other way is reversed, with its
    /// pcurve interval swapped so that its ends still match the canonical edge ends;
    /// pcurves are otherwise kept, so this suits straight edges, whose parameter
    /// correspondence stays affine. Indices of surviving records are compacted in order.
    pub fn apply(&self, mut raw: RawBrep) -> RawBrep {
        if self.is_empty()
            || self.vertices.len() != raw.vertices.len()
            || self.edges.len() != raw.edges.len()
        {
            return raw;
        }
        let (vertex_index, edge_index) = self.compaction();
        for c in &mut raw.coedges {
            let Some(&(survivor, flip)) = self.edges.get(c.edge.0) else {
                continue;
            };
            c.edge = EdgeId(edge_index[survivor.0].expect("survivors are kept"));
            if flip {
                c.orientation = if c.orientation.is_reversed() {
                    Orientation::Forward
                } else {
                    Orientation::Reversed
                };
                if let Some(p) = &mut c.pcurve {
                    p.range.swap(0, 1);
                }
            }
        }
        let mut edges = Vec::with_capacity(raw.edges.len() - self.merged_edges);
        for (i, mut e) in raw.edges.drain(..).enumerate() {
            if edge_index[i].is_some() {
                for v in &mut e.vertices {
                    let r = self.vertices[v.0];
                    *v = VertexId(vertex_index[r.0].expect("representatives are kept"));
                }
                edges.push(e);
            }
        }
        raw.edges = edges;
        let vertices: Vec<Vertex> = raw
            .vertices
            .drain(..)
            .enumerate()
            .filter(|(i, _)| vertex_index[*i].is_some())
            .map(|(_, v)| v)
            .collect();
        raw.vertices = vertices;
        raw
    }
    /// New indices of the kept vertices and edges, in order; `None` for merged ones.
    pub fn compaction(&self) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
        let mut next = 0;
        let vertices = (0..self.vertices.len())
            .map(|i| {
                (self.vertices[i].0 == i).then(|| {
                    next += 1;
                    next - 1
                })
            })
            .collect();
        next = 0;
        let edges = (0..self.edges.len())
            .map(|i| {
                (self.edges[i].0.0 == i).then(|| {
                    next += 1;
                    next - 1
                })
            })
            .collect();
        (vertices, edges)
    }
}

fn exhausted(tolerance: f64) -> RepairError {
    RepairError {
        report: Report {
            defect: Defect::ResourceLimit,
            kind: "model",
            index: 0,
        },
        edge: None,
        partner: None,
        needed: None,
        tolerance,
        reason: "repair work budget exceeded",
    }
}
struct Budget {
    remaining: usize,
    tolerance: f64,
}
impl Budget {
    fn spend(&mut self, n: usize) -> Result<(), RepairError> {
        self.remaining = self
            .remaining
            .checked_sub(n)
            .ok_or_else(|| exhausted(self.tolerance))?;
        Ok(())
    }
}

/// Whether every handle a repair follows is valid; repairs leave other models alone
/// for validation to diagnose.
fn indexable(raw: &RawBrep) -> bool {
    let vertices = raw.vertices.len();
    raw.edges
        .iter()
        .all(|e| e.vertices.iter().all(|v| v.0 < vertices))
        && raw.coedges.iter().all(|c| c.edge.0 < raw.edges.len())
        && raw
            .wires
            .iter()
            .all(|w| w.coedges.iter().all(|c| c.0 < raw.coedges.len()))
        && raw.faces.iter().all(|f| {
            std::iter::once(&f.outer)
                .chain(&f.holes)
                .all(|w| w.0 < raw.wires.len())
        })
        && raw
            .shells
            .iter()
            .all(|s| s.faces.iter().all(|f| f.0 < raw.faces.len()))
}

fn position(raw: &RawBrep, v: VertexId) -> [f64; 3] {
    raw.vertices[v.0].position.coordinates()
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn at(edge: &Edge, t: f64) -> Option<[f64; 3]> {
    Some(edge.curve.evaluate(t).ok()?.position.coordinates())
}
/// A closed edge whose curve stays within the model tolerance of its vertex; it
/// closes a face chart at a singular line and is never a free edge.
fn collapsed(raw: &RawBrep, edge: &Edge, model: f64) -> bool {
    if edge.vertices[0] != edge.vertices[1] {
        return false;
    }
    let p = position(raw, edge.vertices[0]);
    let [a, b] = edge.range;
    [0.25, 0.5, 0.75]
        .iter()
        .all(|f| at(edge, a + (b - a) * f).is_some_and(|q| distance(p, q) <= model))
}

/// The faces of each shell and the shell and face of every edge use.
struct Incidence {
    /// Per edge: (shell, face) of each use, in shell and wire order.
    uses: Vec<Vec<(usize, FaceId)>>,
    /// Per vertex: the shells whose faces use it.
    shells: Vec<BTreeSet<usize>>,
    /// Wire breaks: (shell, end vertex of a use, start vertex of the next use).
    breaks: Vec<(usize, VertexId, VertexId)>,
}
fn incidence(raw: &RawBrep, budget: &mut Budget) -> Result<Incidence, RepairError> {
    let mut uses = vec![Vec::new(); raw.edges.len()];
    let mut shells = vec![BTreeSet::new(); raw.vertices.len()];
    let mut breaks = Vec::new();
    for (si, shell) in raw.shells.iter().enumerate() {
        for &fid in &shell.faces {
            let face = &raw.faces[fid.0];
            for w in std::iter::once(&face.outer).chain(&face.holes) {
                let cs = &raw.wires[w.0].coedges;
                budget.spend(cs.len())?;
                for (j, &cid) in cs.iter().enumerate() {
                    let c = &raw.coedges[cid.0];
                    uses[c.edge.0].push((si, fid));
                    for v in raw.edges[c.edge.0].vertices {
                        shells[v.0].insert(si);
                    }
                    let end = raw.use_vertices(cid).expect("indexable")[1];
                    let start = raw.use_vertices(cs[(j + 1) % cs.len()]).expect("indexable")[0];
                    if end != start {
                        breaks.push((si, end, start));
                    }
                }
            }
        }
    }
    Ok(Incidence {
        uses,
        shells,
        breaks,
    })
}
/// Closed shells of at least two faces whose faces share no edge or vertex.
fn disconnected(raw: &RawBrep, incidence: &Incidence) -> usize {
    raw.shells
        .iter()
        .enumerate()
        .filter(|(si, shell)| {
            if !shell.closed || shell.faces.len() < 2 {
                return false;
            }
            let mut owner: BTreeMap<VertexId, FaceId> = BTreeMap::new();
            for (e, uses) in incidence.uses.iter().enumerate() {
                let mut faces = uses.iter().filter(|(s, _)| s == si).map(|(_, f)| *f);
                let Some(first) = faces.next() else {
                    continue;
                };
                if faces.any(|f| f != first) {
                    return false;
                }
                for v in raw.edges[e].vertices {
                    if *owner.entry(v).or_insert(first) != first {
                        return false;
                    }
                }
            }
            true
        })
        .count()
}

/// Edges with exactly one use (in all shells) that are not collapsed.
fn free_edges(raw: &RawBrep, incidence: &Incidence, model: f64) -> Vec<bool> {
    raw.edges
        .iter()
        .zip(&incidence.uses)
        .map(|(e, u)| u.len() == 1 && !collapsed(raw, e, model))
        .collect()
}

/// The parameter of `edge` nearest to `p` and its distance: the nearest of 32 samples,
/// refined by golden-section search between its neighbours.
fn nearest(
    edge: &Edge,
    p: [f64; 3],
    budget: &mut Budget,
) -> Result<Option<(f64, f64)>, RepairError> {
    let [a, b] = edge.range;
    let n = 32;
    budget.spend(n + 1 + 2 * 24)?;
    let mut best = None;
    for i in 0..=n {
        let t = a + (b - a) * i as f64 / n as f64;
        let Some(q) = at(edge, t) else {
            return Ok(None);
        };
        let d = distance(p, q);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    let (i, _) = best.expect("samples");
    let step = (b - a) / n as f64;
    let (mut lo, mut hi) = (
        (a + step * (i as f64 - 1.)).max(a),
        (a + step * (i as f64 + 1.)).min(b),
    );
    let ratio = (5f64.sqrt() - 1.) / 2.;
    let f = |t: f64| at(edge, t).map(|q| distance(p, q));
    for _ in 0..24 {
        let (x1, x2) = (hi - ratio * (hi - lo), lo + ratio * (hi - lo));
        let (Some(f1), Some(f2)) = (f(x1), f(x2)) else {
            return Ok(None);
        };
        if f1 <= f2 {
            hi = x2;
        } else {
            lo = x1;
        }
    }
    let t = (lo + hi) / 2.;
    Ok(f(t).map(|d| (t, d.min(best.expect("samples").1))))
}
/// Distance from a point to a segment.
fn segment_distance(p: [f64; 3], s: [f64; 3], t: [f64; 3]) -> f64 {
    let d = [t[0] - s[0], t[1] - s[1], t[2] - s[2]];
    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let u = if len2 > 0. {
        (((p[0] - s[0]) * d[0] + (p[1] - s[1]) * d[1] + (p[2] - s[2]) * d[2]) / len2).clamp(0., 1.)
    } else {
        0.
    };
    distance(p, [s[0] + d[0] * u, s[1] + d[1] * u, s[2] + d[2] * u])
}
fn is_line(edge: &Edge) -> bool {
    matches!(&edge.curve, CurveGeometry::Analytic(c) if c.kind() == CurveKind::Line)
}
/// The two-sided distance between two edge curves: exact between straight edges,
/// otherwise the largest distance from 17 samples of each to the other curve.
fn curve_distance(a: &Edge, b: &Edge, budget: &mut Budget) -> Result<Option<f64>, RepairError> {
    if is_line(a) && is_line(b) {
        let ends = |e: &Edge| Some([at(e, e.range[0])?, at(e, e.range[1])?]);
        let (Some([a0, a1]), Some([b0, b1])) = (ends(a), ends(b)) else {
            return Ok(None);
        };
        return Ok(Some(
            [
                segment_distance(a0, b0, b1),
                segment_distance(a1, b0, b1),
                segment_distance(b0, a0, a1),
                segment_distance(b1, a0, a1),
            ]
            .into_iter()
            .fold(0., f64::max),
        ));
    }
    let mut worst = 0f64;
    for (from, to) in [(a, b), (b, a)] {
        let [s, t] = from.range;
        for i in 0..=16 {
            let Some(p) = at(from, s + (t - s) * i as f64 / 16.) else {
                return Ok(None);
            };
            let Some((_, d)) = nearest(to, p, budget)? else {
                return Ok(None);
            };
            worst = worst.max(d);
        }
    }
    Ok(Some(worst))
}
/// Whether `b` runs the same way as `a`, given that their end vertices merged.
fn same_direction(
    a: &Edge,
    b: &Edge,
    rep: &[VertexId],
    budget: &mut Budget,
) -> Result<Option<bool>, RepairError> {
    if a.vertices[0] != a.vertices[1] {
        return Ok(Some(rep[a.vertices[0].0] == rep[b.vertices[0].0]));
    }
    // Closed edges start at the same vertex: a quarter of the way along `b` lies in
    // the first half of `a` when both run the same way.
    let [s, t] = b.range;
    let Some(p) = at(b, s + (t - s) / 4.) else {
        return Ok(None);
    };
    let [s, t] = a.range;
    Ok(nearest(a, p, budget)?.map(|(u, _)| u < s + (t - s) / 2.))
}

/// Grid cell of a point for neighbour search at `cell` spacing.
fn cell(p: [f64; 3], cell: f64) -> Option<[i64; 3]> {
    let mut out = [0; 3];
    for (o, x) in out.iter_mut().zip(p) {
        let k = (x / cell).floor();
        if !k.is_finite() || k.abs() > 1e15 {
            return None;
        }
        *o = k as i64;
    }
    Some(out)
}

/// Choose the merges that sew coincident boundaries of a raw B-rep: vertices of free
/// edges and of wire breaks within `tolerance` of each other in the same shell, then
/// pairs of free edges of different faces that join the same merged vertices and whose
/// curves lie within `tolerance` of each other. A vertex merges only when it stays
/// within `tolerance` of its cluster's representative and no edge would collapse;
/// three or more coincident free edges are left unsewn. `model` identifies collapsed
/// edges. A model with invalid handles is left to validation (an empty plan).
pub fn plan_sewing(
    raw: &RawBrep,
    model: f64,
    tolerance: f64,
    limits: ValidationLimits,
) -> Result<SewPlan, RepairError> {
    let mut plan = SewPlan {
        vertices: (0..raw.vertices.len()).map(VertexId).collect(),
        edges: (0..raw.edges.len()).map(|e| (EdgeId(e), false)).collect(),
        ..SewPlan::default()
    };
    if !indexable(raw) || !(tolerance.is_finite() && tolerance > 0.) {
        return Ok(plan);
    }
    let mut budget = Budget {
        remaining: limits.max_work,
        tolerance,
    };
    let incidence = incidence(raw, &mut budget)?;
    plan.disconnected_shells = disconnected(raw, &incidence);
    let free = free_edges(raw, &incidence, model);
    // Candidate vertices and the one shell each belongs to.
    let mut candidate: BTreeMap<VertexId, usize> = BTreeMap::new();
    let single = |v: VertexId| (incidence.shells[v.0].len() == 1).then_some(v);
    for (e, edge) in raw.edges.iter().enumerate() {
        if free[e] {
            let shell = incidence.uses[e][0].0;
            for v in edge.vertices.into_iter().filter_map(single) {
                candidate.insert(v, shell);
            }
        }
    }
    for &(shell, a, b) in &incidence.breaks {
        for v in [a, b].into_iter().filter_map(single) {
            candidate.insert(v, shell);
        }
    }
    // Vertices joined by an edge never merge (the edge would collapse).
    let mut neighbours: BTreeMap<VertexId, BTreeSet<VertexId>> = BTreeMap::new();
    for e in &raw.edges {
        let [a, b] = e.vertices;
        if a != b {
            neighbours.entry(a).or_default().insert(b);
            neighbours.entry(b).or_default().insert(a);
        }
    }
    let mut grid: BTreeMap<[i64; 3], Vec<VertexId>> = BTreeMap::new();
    for &v in candidate.keys() {
        budget.spend(1)?;
        if let Some(k) = cell(position(raw, v), tolerance) {
            grid.entry(k).or_default().push(v);
        }
    }
    let mut pairs = Vec::new();
    for (&v, &shell) in &candidate {
        let Some(k) = cell(position(raw, v), tolerance) else {
            continue;
        };
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(others) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else {
                        continue;
                    };
                    budget.spend(others.len())?;
                    for &w in others {
                        let d = distance(position(raw, v), position(raw, w));
                        if w > v
                            && candidate[&w] == shell
                            && d <= tolerance
                            && !neighbours.get(&v).is_some_and(|n| n.contains(&w))
                        {
                            pairs.push((d, v, w));
                        }
                    }
                }
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    // Constrained single linkage: nearest pairs first; a union is refused when a
    // member would lie beyond the tolerance from the new representative or when the
    // two clusters hold the two ends of an edge.
    let mut members: BTreeMap<VertexId, Vec<VertexId>> =
        candidate.keys().map(|&v| (v, vec![v])).collect();
    let mut rep: Vec<VertexId> = plan.vertices.clone();
    for (_, v, w) in pairs {
        let (a, b) = (rep[v.0], rep[w.0]);
        if a == b {
            continue;
        }
        let (keep, gone) = (a.min(b), a.max(b));
        let origin = position(raw, keep);
        let joined: Vec<VertexId> = members[&keep]
            .iter()
            .chain(&members[&gone])
            .copied()
            .collect();
        budget.spend(joined.len() * 2)?;
        let within = joined
            .iter()
            .all(|&m| distance(origin, position(raw, m)) <= tolerance);
        let collapses = members[&keep].iter().any(|m| {
            neighbours
                .get(m)
                .is_some_and(|n| members[&gone].iter().any(|g| n.contains(g)))
        });
        if !within || collapses {
            continue;
        }
        for &m in &members[&gone] {
            rep[m.0] = keep;
        }
        members.remove(&gone);
        members.insert(keep, joined);
    }
    for (v, r) in rep.iter().enumerate() {
        if r.0 != v {
            plan.merged_vertices += 1;
            plan.max_distance = plan
                .max_distance
                .max(distance(position(raw, VertexId(v)), position(raw, *r)));
        }
    }
    // Free edges of different faces between the same merged vertices.
    let mut groups: BTreeMap<(usize, VertexId, VertexId), Vec<usize>> = BTreeMap::new();
    for (e, edge) in raw.edges.iter().enumerate() {
        if free[e] {
            let [a, b] = edge.vertices.map(|v| rep[v.0]);
            groups
                .entry((incidence.uses[e][0].0, a.min(b), a.max(b)))
                .or_default()
                .push(e);
        }
    }
    for group in groups.values() {
        let &[e, f] = group.as_slice() else {
            continue;
        };
        if incidence.uses[e][0].1 == incidence.uses[f][0].1 {
            continue;
        }
        let (ea, eb) = (&raw.edges[e], &raw.edges[f]);
        let Some(d) = curve_distance(ea, eb, &mut budget)? else {
            continue;
        };
        if d > tolerance {
            continue;
        }
        let Some(same) = same_direction(ea, eb, &rep, &mut budget)? else {
            continue;
        };
        // An analytic curve survives a B-spline (usually its approximation).
        let (survivor, removed) = if matches!(ea.curve, CurveGeometry::Nurbs(_))
            && matches!(eb.curve, CurveGeometry::Analytic(_))
        {
            (f, e)
        } else {
            (e, f)
        };
        plan.edges[removed] = (EdgeId(survivor), !same);
        plan.merged_edges += 1;
        plan.max_distance = plan.max_distance.max(d);
    }
    plan.vertices = rep;
    Ok(plan)
}

/// The smallest sewing tolerance that would close the defect `report` leaves in `raw`:
/// for an open or unsewn closed shell, the distance from its first free edge to the
/// nearest other free edge of the shell; for a broken wire, the distance across its
/// first break. `None` for other defects or a model with invalid handles.
pub fn diagnose_gap(
    raw: &RawBrep,
    report: &Report,
    model: f64,
    tolerance: f64,
    limits: ValidationLimits,
) -> Result<Option<RepairError>, RepairError> {
    if !indexable(raw) {
        return Ok(None);
    }
    let mut budget = Budget {
        remaining: limits.max_work,
        tolerance,
    };
    let mut error = RepairError {
        report: report.clone(),
        edge: None,
        partner: None,
        needed: None,
        tolerance,
        reason: "",
    };
    match report.defect {
        Defect::BrokenWire => {
            let Some(w) = raw.wires.get(report.index) else {
                return Ok(None);
            };
            let cs = &w.coedges;
            for (j, &c) in cs.iter().enumerate() {
                let end = raw.use_vertices(c).expect("indexable")[1];
                let start = raw.use_vertices(cs[(j + 1) % cs.len()]).expect("indexable")[0];
                if end != start {
                    let d = distance(position(raw, end), position(raw, start));
                    error.edge = Some(raw.coedges[c.0].edge);
                    error.partner = Some(raw.coedges[cs[(j + 1) % cs.len()].0].edge);
                    error.needed = Some(d);
                    error.reason = if d > tolerance {
                        "consecutive edge uses end apart, beyond the sewing tolerance"
                    } else {
                        "consecutive edge uses end within the sewing tolerance but their vertices could not be merged without collapsing an edge"
                    };
                    return Ok(Some(error));
                }
            }
            Ok(None)
        }
        Defect::OpenShell | Defect::UnsewnShell if report.kind == "shell" => {
            if report.index >= raw.shells.len() {
                return Ok(None);
            }
            let incidence = incidence(raw, &mut budget)?;
            let free = free_edges(raw, &incidence, model);
            let edges: Vec<usize> = (0..raw.edges.len())
                .filter(|&e| free[e] && incidence.uses[e][0].0 == report.index)
                .collect();
            let Some(&first) = edges.first() else {
                return Ok(None);
            };
            error.edge = Some(EdgeId(first));
            let mut best: Option<(f64, usize)> = None;
            for &other in &edges[1..] {
                if let Some(d) = curve_distance(&raw.edges[first], &raw.edges[other], &mut budget)?
                {
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, other));
                    }
                }
            }
            match best {
                Some((d, other)) => {
                    error.partner = Some(EdgeId(other));
                    error.needed = Some(d);
                    error.reason = if d > tolerance {
                        "a free edge lies apart from the nearest free edge, beyond the sewing tolerance"
                    } else {
                        "coincident free edges within the sewing tolerance could not be sewn unambiguously"
                    };
                }
                None => error.reason = "a free edge has no other free edge to be sewn to",
            }
            Ok(Some(error))
        }
        _ => Ok(None),
    }
}

/// Flip faces so that, in every shell, the two uses of each edge shared by two faces
/// have opposite effective orientations. Each edge-connected group of faces keeps the
/// orientation of most of its faces (of its first face on a tie). Returns the number of
/// flipped faces; a group that no flips make consistent (non-orientable) is an error.
/// A model with invalid handles, or a face in several shells, is left to validation.
pub fn orient_faces(raw: &mut RawBrep, limits: ValidationLimits) -> Result<usize, RepairError> {
    if !indexable(raw) {
        return Ok(0);
    }
    let mut budget = Budget {
        remaining: limits.max_work,
        tolerance: 0.,
    };
    let mut seen = BTreeSet::new();
    if raw
        .shells
        .iter()
        .flat_map(|s| &s.faces)
        .any(|f| !seen.insert(*f))
    {
        return Ok(0);
    }
    let mut flips = Vec::new();
    for (si, shell) in raw.shells.iter().enumerate() {
        // Effective orientation of each edge use, per edge.
        let mut uses: BTreeMap<EdgeId, Vec<(usize, bool)>> = BTreeMap::new();
        for (local, &fid) in shell.faces.iter().enumerate() {
            let f = &raw.faces[fid.0];
            for w in std::iter::once(&f.outer).chain(&f.holes) {
                budget.spend(raw.wires[w.0].coedges.len())?;
                for &cid in &raw.wires[w.0].coedges {
                    let c = &raw.coedges[cid.0];
                    uses.entry(c.edge).or_default().push((
                        local,
                        c.orientation.is_reversed() ^ f.orientation.is_reversed(),
                    ));
                }
            }
        }
        // Constraint graph: (neighbour, whether the two faces' flips must differ).
        let mut graph = vec![Vec::new(); shell.faces.len()];
        for u in uses.values() {
            if let [(a, ea), (b, eb)] = u.as_slice() {
                if a != b {
                    graph[*a].push((*b, ea == eb));
                    graph[*b].push((*a, ea == eb));
                }
            }
        }
        let mut flip: Vec<Option<bool>> = vec![None; shell.faces.len()];
        for seed in 0..shell.faces.len() {
            if flip[seed].is_some() {
                continue;
            }
            flip[seed] = Some(false);
            let mut group = vec![seed];
            let mut stack = vec![seed];
            while let Some(i) = stack.pop() {
                budget.spend(graph[i].len() + 1)?;
                let fi = flip[i].expect("assigned");
                for &(j, differ) in &graph[i] {
                    let want = fi ^ differ;
                    match flip[j] {
                        None => {
                            flip[j] = Some(want);
                            group.push(j);
                            stack.push(j);
                        }
                        Some(have) if have != want => {
                            return Err(RepairError {
                                report: Report {
                                    defect: Defect::InconsistentOrientation,
                                    kind: "shell",
                                    index: si,
                                },
                                edge: None,
                                partner: None,
                                needed: None,
                                tolerance: 0.,
                                reason: "no face orientation makes every shared edge consistent (the shell is not orientable)",
                            });
                        }
                        Some(_) => {}
                    }
                }
            }
            let flipped = group.iter().filter(|&&i| flip[i] == Some(true)).count();
            if 2 * flipped > group.len() {
                for &i in &group {
                    flip[i] = flip[i].map(|f| !f);
                }
            }
        }
        for (local, f) in flip.into_iter().enumerate() {
            if f == Some(true) {
                flips.push(shell.faces[local]);
            }
        }
    }
    for f in &flips {
        let face = &mut raw.faces[f.0];
        face.orientation = if face.orientation.is_reversed() {
            Orientation::Forward
        } else {
            Orientation::Reversed
        };
    }
    Ok(flips.len())
}
