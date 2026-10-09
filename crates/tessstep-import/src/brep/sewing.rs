//! Opt-in sewing of the curved importer's edge and face records, before planning, so
//! that every pcurve is computed for the surviving edge of a merge.
use super::*;
use crate::repair::{Locator, RepairReport, record};

impl Builder {
    /// Merge coincident boundary vertices and free edges (`ImportOptions::repair`):
    /// plan the merges on a pcurve-free raw B-rep of the records
    /// (`tessstep_topology::plan_sewing`), retarget every use onto the surviving
    /// edges and vertices, and compact the vertex and edge tables. The working model
    /// tolerance grows by the largest merge distance. Without sewing this does nothing.
    pub(super) fn sew(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &mut [FaceRecord],
        root: EntityId,
        options: ImportOptions,
    ) -> Result<RepairReport, Error> {
        let mut report = RepairReport::default();
        if !options.repair.sew {
            return Ok(report);
        }
        let sewing = options.repair.checked_sewing_tolerance(c.tolerance)?;
        report.sewing_tolerance = sewing;
        let mut raw = RawBrep {
            vertices: c.raw.vertices.clone(),
            ..RawBrep::default()
        };
        for e in &self.edges {
            c.charge(1)?;
            raw.edges.push(Edge {
                vertices: e.vertices,
                curve: e.curve.clone(),
                range: e.range,
            });
        }
        let mut shells: Vec<Vec<FaceId>> = Vec::new();
        for face in faces.iter() {
            let mut wires = Vec::new();
            for l in face.loops.iter().filter(|l| !l.uses.is_empty()) {
                c.charge(l.uses.len())?;
                let mut coedges = Vec::new();
                for u in &l.uses {
                    coedges.push(CoedgeId(raw.coedges.len()));
                    raw.coedges.push(Coedge {
                        edge: EdgeId(u.edge),
                        orientation: if u.forward {
                            Orientation::Forward
                        } else {
                            Orientation::Reversed
                        },
                        pcurve: None,
                    });
                }
                wires.push(WireId(raw.wires.len()));
                raw.wires.push(Wire { coedges });
            }
            let Some((&outer, holes)) = wires.split_first() else {
                continue;
            };
            raw.faces.push(Face {
                surface: face.surface.clone(),
                outer,
                holes: holes.to_vec(),
                orientation: if face.same {
                    Orientation::Forward
                } else {
                    Orientation::Reversed
                },
            });
            if shells.len() <= face.shell {
                shells.resize(face.shell + 1, Vec::new());
            }
            shells[face.shell].push(FaceId(raw.faces.len() - 1));
        }
        raw.shells = shells
            .into_iter()
            .map(|faces| Shell {
                faces,
                closed: true,
            })
            .collect();
        let locator = Locator {
            document: c.decoded.document(),
            root,
            edges: self.edges.iter().map(|e| e.entity).collect(),
            vertices: Vec::new(),
        };
        let plan = plan_sewing(
            &raw,
            c.tolerance.distance().as_metres(),
            sewing,
            ValidationLimits {
                max_records: options.max_records,
                max_work: options.max_work,
            },
        )
        .map_err(|e| locator.repair(e, options.max_work))?;
        c.tolerance = record(&mut report, &plan, c.tolerance)?;
        if plan.is_empty() {
            return Ok(report);
        }
        let (vertex_index, edge_index) = plan.compaction();
        let vertex = |v: VertexId| {
            VertexId(vertex_index[plan.vertices[v.0].0].expect("representatives are kept"))
        };
        let edge = |e: usize| {
            let (survivor, flip) = plan.edges[e];
            (edge_index[survivor.0].expect("survivors are kept"), flip)
        };
        for face in faces.iter_mut() {
            for l in &mut face.loops {
                if let Some(v) = &mut l.vertex {
                    *v = vertex(*v);
                }
                for u in &mut l.uses {
                    let (index, flip) = edge(u.edge);
                    u.edge = index;
                    u.forward ^= flip;
                }
            }
        }
        for (index, forward) in self.edge_index.values_mut() {
            let (survivor, flip) = edge(*index);
            *index = survivor;
            *forward ^= flip;
        }
        let records = std::mem::take(&mut self.edges);
        self.edges = records
            .into_iter()
            .enumerate()
            .filter(|(i, _)| edge_index[*i].is_some())
            .map(|(_, mut e)| {
                e.vertices = e.vertices.map(vertex);
                e
            })
            .collect();
        for v in c.points.values_mut() {
            *v = vertex(*v);
        }
        let keep = |i: &usize| vertex_index[*i].is_some();
        let vertices = std::mem::take(&mut c.raw.vertices);
        c.raw.vertices = vertices
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep(i))
            .map(|(_, v)| v)
            .collect();
        let entities = std::mem::take(&mut c.point_entities);
        c.point_entities = entities
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep(i))
            .map(|(_, v)| v)
            .collect();
        Ok(report)
    }
}
