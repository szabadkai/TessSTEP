use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use tessstep_math::ModelTolerance;
#[derive(Clone, Copy, Debug)]
pub struct ValidationLimits {
    pub max_records: usize,
    pub max_work: usize,
}
impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_records: 1_000_000,
            max_work: 10_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defect {
    ResourceLimit,
    Empty,
    InvalidHandle,
    InvalidRange,
    EndpointMismatch,
    GeometryFailure,
    BrokenWire,
    DuplicateOwnership,
    UnusedRecord,
    NonManifoldEdge,
    InconsistentOrientation,
    OpenShell,
    DisconnectedShell,
    NonManifoldVertex,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub defect: Defect,
    pub kind: &'static str,
    pub index: usize,
}
impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} at {} {}", self.defect, self.kind, self.index)
    }
}
impl std::error::Error for Report {}
fn fail(defect: Defect, kind: &'static str, index: usize) -> Report {
    Report {
        defect,
        kind,
        index,
    }
}
fn own(count: &mut [bool], index: usize, kind: &'static str) -> Result<(), Report> {
    let slot = count
        .get_mut(index)
        .ok_or(fail(Defect::InvalidHandle, kind, index))?;
    if *slot {
        return Err(fail(Defect::DuplicateOwnership, kind, index));
    }
    *slot = true;
    Ok(())
}
fn all_used(count: &[bool], kind: &'static str) -> Result<(), Report> {
    if let Some(i) = count.iter().position(|x| !x) {
        return Err(fail(Defect::UnusedRecord, kind, i));
    }
    Ok(())
}
fn spend(work: &mut usize, n: usize) -> Result<(), Report> {
    *work = work
        .checked_sub(n)
        .ok_or(fail(Defect::ResourceLimit, "model", 0))?;
    Ok(())
}
impl RawBrep {
    /// First deterministic defect; input geometry is never healed or rewritten.
    pub fn validate(
        self,
        tolerance: ModelTolerance,
        limits: ValidationLimits,
    ) -> Result<ValidatedBrep, Report> {
        let mut work = limits.max_work;
        let mut records = 0usize;
        for n in [
            self.vertices.len(),
            self.edges.len(),
            self.coedges.len(),
            self.wires.len(),
            self.faces.len(),
            self.shells.len(),
            self.solids.len(),
        ] {
            records = records
                .checked_add(n)
                .ok_or(fail(Defect::ResourceLimit, "model", 0))?;
        }
        if records > limits.max_records.min(1_000_000) {
            return Err(fail(Defect::ResourceLimit, "model", 0));
        }
        spend(&mut work, records)?;
        if self.faces.is_empty() {
            return Err(fail(Defect::Empty, "faces", 0));
        }
        let mut vertices = vec![false; self.vertices.len()];
        let mut edge_uses = vec![Vec::new(); self.edges.len()];
        let distance = tolerance.distance().as_metres();
        for (i, e) in self.edges.iter().enumerate() {
            let [a, b] = e.range;
            if !a.is_finite() || !b.is_finite() || a >= b || !(b - a).is_finite() {
                return Err(fail(Defect::InvalidRange, "edge", i));
            }
            for (v, u) in e.vertices.iter().zip(e.range) {
                let vertex =
                    self.vertices
                        .get(v.0)
                        .ok_or(fail(Defect::InvalidHandle, "edge", i))?;
                vertices[v.0] = true;
                let p = e
                    .curve
                    .evaluate(u)
                    .map_err(|_| fail(Defect::GeometryFailure, "edge", i))?
                    .position;
                if p.distance(vertex.position)
                    .map_err(|_| fail(Defect::GeometryFailure, "edge", i))?
                    > distance
                {
                    return Err(fail(Defect::EndpointMismatch, "edge", i));
                }
            }
        }
        all_used(&vertices, "vertex")?;
        for (i, c) in self.coedges.iter().enumerate() {
            edge_uses
                .get_mut(c.edge.0)
                .ok_or(fail(Defect::InvalidHandle, "coedge", i))?
                .push(CoedgeId(i));
            if let Some(p) = &c.pcurve {
                if p.range.iter().any(|x| !x.is_finite())
                    || p.range[0] == p.range[1]
                    || !(p.range[1] - p.range[0]).is_finite()
                {
                    return Err(fail(Defect::InvalidRange, "pcurve", i));
                }
                for u in p.range {
                    p.curve
                        .evaluate(u)
                        .map_err(|_| fail(Defect::GeometryFailure, "pcurve", i))?;
                }
            }
        }
        if let Some(i) = edge_uses.iter().position(Vec::is_empty) {
            return Err(fail(Defect::UnusedRecord, "edge", i));
        }
        let mut coedges = vec![false; self.coedges.len()];
        for (i, w) in self.wires.iter().enumerate() {
            spend(&mut work, w.coedges.len())?;
            if w.coedges.is_empty() {
                return Err(fail(Defect::Empty, "wire", i));
            }
            for (j, &c) in w.coedges.iter().enumerate() {
                own(&mut coedges, c.0, "coedge")?;
                let a = self
                    .use_vertices(c)
                    .ok_or(fail(Defect::InvalidHandle, "wire", i))?;
                let b = self
                    .use_vertices(w.coedges[(j + 1) % w.coedges.len()])
                    .ok_or(fail(Defect::InvalidHandle, "wire", i))?;
                if a[1] != b[0] {
                    return Err(fail(Defect::BrokenWire, "wire", i));
                }
            }
        }
        all_used(&coedges, "coedge")?;
        let mut wires = vec![false; self.wires.len()];
        for f in &self.faces {
            spend(&mut work, f.holes.len().saturating_add(1))?;
            for w in std::iter::once(&f.outer).chain(&f.holes) {
                own(&mut wires, w.0, "wire")?;
            }
        }
        all_used(&wires, "wire")?;
        let mut faces = vec![false; self.faces.len()];
        for (si, shell) in self.shells.iter().enumerate() {
            spend(&mut work, shell.faces.len())?;
            if shell.faces.is_empty() {
                return Err(fail(Defect::Empty, "shell", si));
            }
            let mut incidence: BTreeMap<EdgeId, Vec<(usize, bool)>> = BTreeMap::new();
            let mut links: BTreeMap<VertexId, Vec<(EdgeId, EdgeId)>> = BTreeMap::new();
            for (local, &fid) in shell.faces.iter().enumerate() {
                own(&mut faces, fid.0, "face")?;
                let f = &self.faces[fid.0];
                for w in std::iter::once(&f.outer).chain(&f.holes) {
                    let cs = &self.wires[w.0].coedges;
                    spend(&mut work, cs.len())?;
                    for (j, &cid) in cs.iter().enumerate() {
                        let c = &self.coedges[cid.0];
                        incidence.entry(c.edge).or_default().push((
                            local,
                            c.orientation.is_reversed() ^ f.orientation.is_reversed(),
                        ));
                        let prev = &self.coedges[cs[(j + cs.len() - 1) % cs.len()].0];
                        let v = self.use_vertices(cid).expect("validated handles")[0];
                        links.entry(v).or_default().push((prev.edge, c.edge));
                    }
                }
            }
            let mut adjacency = vec![Vec::new(); shell.faces.len()];
            for uses in incidence.values() {
                if uses.len() > 2 {
                    return Err(fail(Defect::NonManifoldEdge, "shell", si));
                }
                if uses.len() == 1 && shell.closed {
                    return Err(fail(Defect::OpenShell, "shell", si));
                }
                if uses.len() == 2 {
                    if uses[0].1 == uses[1].1 {
                        return Err(fail(Defect::InconsistentOrientation, "shell", si));
                    }
                    adjacency[uses[0].0].push(uses[1].0);
                    adjacency[uses[1].0].push(uses[0].0);
                }
            }
            let mut seen = vec![false; adjacency.len()];
            let mut stack = vec![0];
            seen[0] = true;
            while let Some(i) = stack.pop() {
                spend(&mut work, adjacency[i].len())?;
                for &j in &adjacency[i] {
                    if !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            if seen.contains(&false) {
                return Err(fail(Defect::DisconnectedShell, "shell", si));
            }
            // Each vertex link must be one path (boundary) or one cycle (interior).
            for corners in links.values() {
                let mut graph: BTreeMap<EdgeId, Vec<EdgeId>> = BTreeMap::new();
                for &(a, b) in corners {
                    graph.entry(a).or_default().push(b);
                    graph.entry(b).or_default().push(a);
                }
                if graph
                    .values()
                    .any(|a| a.len() > 2 || (shell.closed && a.len() != 2))
                {
                    return Err(fail(Defect::NonManifoldVertex, "shell", si));
                }
                let start = *graph.keys().next().expect("nonempty link");
                let mut visited = BTreeSet::from([start]);
                let mut todo = vec![start];
                while let Some(a) = todo.pop() {
                    spend(&mut work, graph[&a].len())?;
                    for &b in &graph[&a] {
                        if visited.insert(b) {
                            todo.push(b);
                        }
                    }
                }
                if visited.len() != graph.len() {
                    return Err(fail(Defect::NonManifoldVertex, "shell", si));
                }
            }
        }
        // Faces may be intentionally unsewn; shells may be open. Solids may not.
        let mut solids = vec![false; self.shells.len()];
        for (i, s) in self.solids.iter().enumerate() {
            own(&mut solids, s.shell.0, "shell")?;
            if !self.shells[s.shell.0].closed {
                return Err(fail(Defect::OpenShell, "solid", i));
            }
        }
        Ok(ValidatedBrep {
            raw: self,
            edge_uses,
            tolerance: distance,
        })
    }
}
