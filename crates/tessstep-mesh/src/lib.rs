//! Owned, immutable indexed meshes with checked oriented manifold incidence.
//! Closure is combinatorial: self-intersection and material containment are not certified.
#![forbid(unsafe_code)]

pub mod appearance;
pub mod scene;
use std::collections::{BTreeMap, BTreeSet};
use tessstep_math::{ModelSpace, Point3, Vector3};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_vertices: usize,
    pub max_triangles: usize,
    pub max_work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_vertices: 1_000_000,
            max_triangles: 2_000_000,
            max_work: 50_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    ResourceLimit,
    InvalidAttributes,
    InvalidIndex,
    NonFinite,
    DegenerateTriangle,
    DuplicateTriangle,
    NonManifoldEdge,
    InconsistentWinding,
    NonManifoldVertex,
    UnusedVertex,
    Empty,
    Open,
    Disconnected,
    NonPositiveVolume,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mesh: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// Per-corner attributes preserve seams and sharp face normals after position welding.
    pub normals: Vec<[[f64; 3]; 3]>,
    pub uvs: Vec<[[f64; 2]; 3]>,
    /// Application/model-local face identity for each triangle; no STEP identity implied.
    pub face_ids: Vec<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Statistics {
    pub boundary_edges: usize,
    pub components: usize,
    pub signed_volume: f64,
}
#[derive(Clone, Debug)]
pub struct Mesh {
    data: MeshData,
    statistics: Statistics,
}
impl Mesh {
    /// Transfer the owned buffers for application-level attribute processing.
    /// Reconstruct with `Mesh::new` to validate any edits.
    pub fn into_data(self) -> MeshData {
        self.data
    }

    pub fn data(&self) -> &MeshData {
        &self.data
    }
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
    pub fn is_watertight(&self) -> bool {
        self.statistics.boundary_edges == 0
    }
    /// Require a single closed oriented component and positive algebraic volume.
    /// This still does not prove nonintersection or establish cavity semantics.
    pub fn require_solid(&self) -> Result<(), Error> {
        if !self.is_watertight() {
            return Err(Error::Open);
        }
        if self.statistics.components != 1 {
            return Err(Error::Disconnected);
        }
        if self.statistics.signed_volume <= 0. {
            return Err(Error::NonPositiveVolume);
        }
        Ok(())
    }
    /// Copy-free ownership transfer; validation finishes before a mesh is published.
    pub fn new(data: MeshData, limits: Limits) -> Result<Self, Error> {
        let nv = data.positions.len();
        let nt = data.triangles.len();
        if nv > limits.max_vertices.min(u32::MAX as usize) || nt > limits.max_triangles {
            return Err(Error::ResourceLimit);
        }
        if nv == 0 || nt == 0 {
            return Err(Error::Empty);
        }
        if data.normals.len() != nt || data.uvs.len() != nt || data.face_ids.len() != nt {
            return Err(Error::InvalidAttributes);
        }
        let mut budget = Budget(limits.max_work);
        budget.charge(nv)?;
        let positions: Vec<_> = data
            .positions
            .iter()
            .map(|&p| Point3::<ModelSpace>::new(p).map_err(|_| Error::NonFinite))
            .collect::<Result<_, _>>()?;
        let mut edges = BTreeMap::<(u32, u32), Vec<(usize, bool)>>::new();
        let mut links = vec![Vec::new(); nv];
        let mut unique = BTreeSet::new();
        let mut volume = 0.;
        let mut compensation = 0.;
        let origin = positions[0];
        for (ti, &tri) in data.triangles.iter().enumerate() {
            budget.charge(10)?;
            if tri.iter().any(|&i| i as usize >= nv) {
                return Err(Error::InvalidIndex);
            }
            let [a, b, c] = tri.map(|i| positions[i as usize]);
            let normal = b
                .difference(a)
                .and_then(|ab| c.difference(a).and_then(|ac| ab.cross(ac)))
                .and_then(|n| n.normalized())
                .map_err(|_| Error::DegenerateTriangle)?;
            let mut key = tri;
            key.sort();
            if !unique.insert(key) {
                return Err(Error::DuplicateTriangle);
            }
            for k in 0..3 {
                let n = Vector3::<ModelSpace>::new(data.normals[ti][k])
                    .map_err(|_| Error::NonFinite)?;
                let length = n.norm().map_err(|_| Error::NonFinite)?;
                if (length - 1.).abs() > 1e-8
                    || n.dot(normal.vector()).map_err(|_| Error::NonFinite)? <= 0.
                    || data.uvs[ti][k].iter().any(|x| !x.is_finite())
                {
                    return Err(Error::InvalidAttributes);
                }
                let u = tri[k];
                let v = tri[(k + 1) % 3];
                let w = tri[(k + 2) % 3];
                edges
                    .entry((u.min(v), u.max(v)))
                    .or_default()
                    .push((ti, u < v));
                links[u as usize].push((v, w));
            }
            // Translation-relative, compensated sum avoids avoidable cancellation.
            let term = a
                .difference(origin)
                .and_then(|ar| {
                    b.difference(origin).and_then(|br| {
                        c.difference(origin)
                            .and_then(|cr| br.cross(cr).and_then(|cross| ar.dot(cross)))
                    })
                })
                .map_err(|_| Error::NonFinite)?
                / 6.;
            let corrected = term - compensation;
            let next = volume + corrected;
            compensation = (next - volume) - corrected;
            volume = next;
            if !volume.is_finite() {
                return Err(Error::NonFinite);
            }
        }
        let mut adjacency = vec![Vec::new(); nt];
        let mut boundary_edges = 0;
        for uses in edges.values() {
            budget.charge(1)?;
            match uses.as_slice() {
                [_] => boundary_edges += 1,
                [(a, da), (b, db)] => {
                    if da == db {
                        return Err(Error::InconsistentWinding);
                    }
                    adjacency[*a].push(*b);
                    adjacency[*b].push(*a);
                }
                _ => return Err(Error::NonManifoldEdge),
            }
        }
        for link in links {
            if link.is_empty() {
                return Err(Error::UnusedVertex);
            }
            let mut graph = BTreeMap::<u32, Vec<u32>>::new();
            for (a, b) in link {
                budget.charge(1)?;
                graph.entry(a).or_default().push(b);
                graph.entry(b).or_default().push(a);
            }
            let ends = graph.values().filter(|v| v.len() == 1).count();
            if (ends != 0 && ends != 2) || graph.values().any(|v| v.len() > 2) {
                return Err(Error::NonManifoldVertex);
            }
            let mut seen = BTreeSet::new();
            let mut stack = vec![*graph.keys().next().unwrap()];
            while let Some(v) = stack.pop() {
                budget.charge(1)?;
                if seen.insert(v) {
                    stack.extend(&graph[&v]);
                }
            }
            if seen.len() != graph.len() {
                return Err(Error::NonManifoldVertex);
            }
        }
        let mut seen = vec![false; nt];
        let mut components = 0;
        for i in 0..nt {
            if seen[i] {
                continue;
            }
            components += 1;
            let mut stack = vec![i];
            while let Some(t) = stack.pop() {
                budget.charge(1)?;
                if !seen[t] {
                    seen[t] = true;
                    stack.extend(&adjacency[t]);
                }
            }
        }
        Ok(Self {
            data,
            statistics: Statistics {
                boundary_edges,
                components,
                signed_volume: volume,
            },
        })
    }
    /// Import indexed triangles with generated faceted normals, zero UVs and face id 0.
    pub fn from_triangles(
        positions: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        limits: Limits,
    ) -> Result<Self, Error> {
        if positions.len() > limits.max_vertices || triangles.len() > limits.max_triangles {
            return Err(Error::ResourceLimit);
        }
        let mut limits = limits;
        limits.max_work = limits
            .max_work
            .checked_sub(triangles.len())
            .ok_or(Error::ResourceLimit)?;
        let mut normals = Vec::with_capacity(triangles.len());
        for tri in &triangles {
            let [a, b, c] = tri.map(|i| {
                positions
                    .get(i as usize)
                    .copied()
                    .ok_or(Error::InvalidIndex)
            });
            let a = Point3::<ModelSpace>::new(a?).map_err(|_| Error::NonFinite)?;
            let b = Point3::new(b?).map_err(|_| Error::NonFinite)?;
            let c = Point3::new(c?).map_err(|_| Error::NonFinite)?;
            let normal = b
                .difference(a)
                .and_then(|ab| c.difference(a).and_then(|ac| ab.cross(ac)))
                .and_then(|n| n.normalized())
                .map_err(|_| Error::DegenerateTriangle)?
                .vector()
                .components();
            normals.push([normal; 3]);
        }
        let n = triangles.len();
        Self::new(
            MeshData {
                positions,
                triangles,
                normals,
                uvs: vec![[[0.; 2]; 3]; n],
                face_ids: vec![0; n],
            },
            limits,
        )
    }
}
struct Budget(usize);
impl Budget {
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        self.0 = self.0.checked_sub(n).ok_or(Error::ResourceLimit)?;
        Ok(())
    }
}
