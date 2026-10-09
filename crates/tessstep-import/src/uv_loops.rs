//! Located, classified UV loop failures at the tessellation stage.
//!
//! Trimming and boundary checks report the coedges (and loop) of a failed polygon
//! check; this module names their STEP entities and the cause class, so that no face
//! fails with an unlocated polygon error. Each class follows from the topology alone:
//! which coedges conflict, whether they meet, whether the importer inserted them.
use crate::{ErrorKind, ImportedSolid};
use tessstep_part21::EntityId;
use tessstep_tessellate::{BoundaryError, PlanarError, TessellationErrorKind};
use tessstep_topology::{CoedgeId, FaceId, NormalizedBrep, WireId};
use tessstep_trim::{Budget, Error as TrimError, ErrorKind as Trim};

/// STEP identity of a solid's topology, by model-local index, for diagnostics.
#[derive(Clone, Debug, Default)]
pub(crate) struct TopologyEntities {
    /// The EDGE_CURVE of each edge (both halves of a split edge name it); `None` for
    /// edges the importer inserted: seams and collapsed pole edges.
    pub edges: Vec<Option<EntityId>>,
    /// The VERTEX_POINT of each vertex; an inserted vertex names the entity it was
    /// inserted for (the split EDGE_CURVE, or the face of an inserted pole).
    pub vertices: Vec<EntityId>,
}

/// The kind, entity and message of a tessellation failure in a face's UV loops, or
/// `None` when the failure is not a loop failure. The entity is the EDGE_CURVE of the
/// failing coedge when it has one, otherwise the face.
pub(crate) fn describe(
    solid: &ImportedSolid,
    face: FaceId,
    kind: &TessellationErrorKind,
) -> Option<(ErrorKind, Option<EntityId>, String)> {
    let (e, stage) = match kind {
        TessellationErrorKind::Boundary(BoundaryError::Trim(e))
        | TessellationErrorKind::Polygon(PlanarError::Boundary(BoundaryError::Trim(e))) => {
            (e, "UV trimming")
        }
        TessellationErrorKind::Boundary(BoundaryError::Resolution(e))
        | TessellationErrorKind::Polygon(PlanarError::InvalidPolygon(e)) => {
            (e, "the mesh boundary")
        }
        _ => return None,
    };
    let face_entity = solid.face_entities().get(face.0).copied();
    let names = Names {
        brep: solid.brep(),
        topology: solid.topology(),
    };
    let (class, entity) = classify(&names, face, e);
    let at = face_entity.map_or_else(String::new, |f| format!("face #{}: ", f.get()));
    let kind = if e.kind == Trim::Limit {
        ErrorKind::ResourceLimit
    } else {
        ErrorKind::InvalidGeometry
    };
    Some((
        kind,
        entity.or(face_entity),
        format!("tessellation of {at}{stage}: {class}"),
    ))
}

struct Names<'a> {
    brep: &'a NormalizedBrep,
    topology: &'a crate::uv_loops::TopologyEntities,
}
impl Names<'_> {
    fn edge_entity(&self, c: CoedgeId) -> Option<EntityId> {
        let edge = self.brep.data().coedges.get(c.0)?.edge;
        self.topology.edges.get(edge.0).copied().flatten()
    }
    /// "EDGE_CURVE #N", or what the importer inserted.
    fn edge(&self, c: CoedgeId) -> String {
        let Some(coedge) = self.brep.data().coedges.get(c.0) else {
            return "an unknown edge".into();
        };
        match self.edge_entity(c) {
            Some(id) => format!("EDGE_CURVE #{}", id.get()),
            None if self.brep.is_collapsed(coedge.edge) => "an inserted pole edge".into(),
            None => "an inserted seam".into(),
        }
    }
    fn vertex(&self, v: tessstep_topology::VertexId) -> String {
        match self.topology.vertices.get(v.0) {
            Some(id) => format!("vertex #{}", id.get()),
            None => "a vertex".into(),
        }
    }
    /// "the outer bound" or "hole N" (1-based, in face order) of the face.
    fn wire(&self, face: FaceId, w: Option<WireId>) -> String {
        let Some(f) = self.brep.data().faces.get(face.0) else {
            return "a loop".into();
        };
        match w {
            Some(w) if w == f.outer => "the outer bound".into(),
            Some(w) => match f.holes.iter().position(|&h| h == w) {
                Some(i) => format!("hole {}", i + 1),
                None => "a loop".into(),
            },
            None => "a loop".into(),
        }
    }
}

/// The cause class and located entity of one trimming or boundary failure.
fn classify(n: &Names<'_>, face: FaceId, e: &TrimError) -> (String, Option<EntityId>) {
    let raw = n.brep.data();
    let entity = e.coedge.and_then(|c| n.edge_entity(c));
    let loop_name = n.wire(face, e.wire);
    let class = match (e.kind, e.coedge, e.other) {
        (Trim::Limit, ..) => match e.budget {
            Some(Budget::MaxSamples(max)) => {
                format!("the trim max_samples budget ({max}) is exhausted (budget)")
            }
            Some(Budget::MaxWork(max)) => {
                format!("the trim max_work budget ({max}) is exhausted (budget)")
            }
            None => "a trim budget is exhausted (budget)".into(),
        },
        (Trim::SelfIntersection, Some(a), Some(b)) if a == b => format!(
            "the chart image of {} doubles back on itself or crosses itself beyond the model tolerance (self-intersecting edge)",
            n.edge(a)
        ),
        (Trim::SelfIntersection | Trim::IntersectingLoops, Some(a), Some(b)) => {
            let inserted = [a, b].into_iter().find(|&c| n.edge_entity(c).is_none());
            let shared = match (raw.use_vertices(a), raw.use_vertices(b)) {
                (Some([sa, ea]), Some([sb, eb])) => {
                    [sa, ea].into_iter().find(|v| *v == sb || *v == eb)
                }
                _ => None,
            };
            let visits = |v| {
                let wire = e.wire.and_then(|w| raw.wires.get(w.0));
                wire.map_or(0, |w| {
                    w.coedges
                        .iter()
                        .filter(|&&c| raw.use_vertices(c).is_some_and(|[s, _]| s == v))
                        .count()
                })
            };
            let pair = format!("{} and {}", n.edge(a), n.edge(b));
            match (inserted, shared) {
                (Some(c), _) if n.brep.is_collapsed(raw.coedges[c.0].edge) => format!(
                    "{pair} cross or touch in the surface chart at a pole or apex line (collapsed edge)"
                ),
                (Some(_), _) => format!(
                    "{pair} cross or touch in the surface chart: the inserted seam meets a loop edge (seam choice)"
                ),
                (None, Some(v)) if e.kind == Trim::SelfIntersection && visits(v) > 1 => format!(
                    "{pair} touch at {}, which {loop_name} passes through twice (self-touching loop)",
                    n.vertex(v)
                ),
                (None, Some(v)) => format!(
                    "{pair}, which meet at {}, cross or touch again in the surface chart beyond the model tolerance (tangent edges at a vertex)",
                    n.vertex(v)
                ),
                (None, None) if e.kind == Trim::IntersectingLoops => format!(
                    "{pair} of different loops cross or touch in the surface chart (intersecting loops)"
                ),
                (None, None) => format!(
                    "{pair} of {loop_name} cross or touch in the surface chart (crossing trims)"
                ),
            }
        }
        (Trim::IntersectingLoops, ..) => {
            format!("{loop_name} overlaps or nests in another hole (intersecting loops)")
        }
        (Trim::HoleOutside, ..) => {
            format!("{loop_name} does not lie inside the outer bound (hole outside)")
        }
        (Trim::WrongOrientation, ..) => format!(
            "{loop_name} winds the wrong way in the surface chart: outer bounds run counterclockwise, holes clockwise (wrong orientation)"
        ),
        (Trim::DegenerateLoop, Some(c), _) => format!(
            "{} has a boundary segment shorter than the tolerance in {loop_name} (degenerate loop)",
            n.edge(c)
        ),
        (Trim::DegenerateLoop, None, _) => format!(
            "{loop_name} encloses no area at the tolerance or has fewer than three points (degenerate loop)"
        ),
        (Trim::NonContractibleLoop, ..) => format!(
            "{loop_name} wraps around a periodic surface direction and does not close in the chart (non-contractible loop)"
        ),
        (Trim::OpenLoop, Some(c), _) => format!(
            "{loop_name} does not close: {} does not start where the previous edge ends (open loop)",
            n.edge(c)
        ),
        (kind, Some(c), _) => format!("{kind:?} at {} in {loop_name}", n.edge(c)),
        (kind, None, _) => format!("{kind:?} in {loop_name}"),
    };
    (class, entity)
}

#[cfg(test)]
#[path = "../../tessstep-topology/tests/support/mod.rs"]
mod support;
#[cfg(test)]
mod tests {
    use super::support::*;
    use super::*;
    use crate::Adaptations;
    use tessstep_topology::{RawBrep, ValidationLimits, WireId};

    fn id(n: u64) -> EntityId {
        EntityId::new(n).unwrap()
    }
    /// The first face of `raw` as an imported solid whose edge `i` is EDGE_CURVE
    /// #(100 + i), except `inserted` edges, and whose vertex `i` is #(10 + i).
    fn solid(raw: RawBrep, inserted: &[usize]) -> ImportedSolid {
        let edges = (0..raw.edges.len())
            .map(|i| (!inserted.contains(&i)).then(|| id(100 + i as u64)))
            .collect();
        let vertices = (0..raw.vertices.len()).map(|i| id(10 + i as u64)).collect();
        ImportedSolid {
            brep: raw
                .validate(tolerance(), ValidationLimits::default())
                .unwrap()
                .normalize(),
            faces: vec![id(1)],
            root: id(2),
            adaptations: Adaptations::default(),
            topology: TopologyEntities { edges, vertices },
            repairs: Default::default(),
        }
    }
    /// The located kind, entity and message of the first face's trimming failure.
    fn failure(
        solid: &ImportedSolid,
        options: tessstep_trim::Options,
    ) -> (ErrorKind, Option<EntityId>, String) {
        let e = tessstep_trim::reconstruct(solid.brep(), FaceId(0), options).unwrap_err();
        describe(
            solid,
            FaceId(0),
            &TessellationErrorKind::Boundary(BoundaryError::Trim(e)),
        )
        .unwrap()
    }
    fn trimmed(raw: RawBrep, inserted: &[usize]) -> (ErrorKind, Option<EntityId>, String) {
        failure(&solid(raw, inserted), tessstep_trim::Options::default())
    }

    #[test]
    fn loop_failures_name_their_edges_and_cause() {
        // Bow tie: two of its edges cross.
        let bow_tie = || {
            polygons(
                &[[0., 0., 0.], [1., 1., 0.], [0., 1., 0.], [1., 0., 0.]],
                &[vec![0, 1, 2, 3]],
            )
        };
        let (kind, entity, message) = trimmed(bow_tie(), &[]);
        assert_eq!(kind, ErrorKind::InvalidGeometry);
        assert!(matches!(entity.map(EntityId::get), Some(100 | 102)));
        assert!(
            message.starts_with("tessellation of face #1: UV trimming: "),
            "{message}"
        );
        assert!(
            message.ends_with(
                "of the outer bound cross or touch in the surface chart (crossing trims)"
            ),
            "{message}"
        );
        // The same crossing with an importer-inserted edge is a seam choice.
        let (_, _, message) = trimmed(bow_tie(), &[0, 2]);
        assert!(
            message.contains("an inserted seam") && message.ends_with("(seam choice)"),
            "{message}"
        );
        // Two triangles touching at vertex 2: the loop passes through it twice.
        let pinched = polygons(
            &[
                [0., 0., 0.],
                [2., 0., 0.],
                [1., 1., 0.],
                [2., 2., 0.],
                [0., 2., 0.],
            ],
            &[vec![0, 1, 2, 3, 4, 2]],
        );
        let (_, _, message) = trimmed(pinched, &[]);
        assert!(message.contains("touch at vertex #12, which the outer bound passes through twice (self-touching loop)"), "{message}");
        // An edge running back along the previous one, past their shared vertex.
        let spike = polygons(
            &[
                [0., 0., 0.],
                [2., 0., 0.],
                [2., 2., 0.],
                [2., 1., 0.],
                [0., 2., 0.],
            ],
            &[vec![0, 1, 2, 3, 4]],
        );
        let (_, entity, message) = trimmed(spike, &[]);
        assert!(matches!(entity.map(EntityId::get), Some(101 | 102)));
        assert!(
            message.contains("which meet at vertex #12, cross or touch again")
                && message.ends_with("(tangent edges at a vertex)"),
            "{message}"
        );
    }

    #[test]
    fn loop_level_failures_name_their_loop_and_budgets_their_value() {
        let mut outside = polygons(
            &[
                [0., 0., 0.],
                [1., 0., 0.],
                [1., 1., 0.],
                [0., 1., 0.],
                [2., 2., 0.],
                [2., 3., 0.],
                [3., 3., 0.],
                [3., 2., 0.],
            ],
            &[vec![0, 1, 2, 3], vec![4, 5, 6, 7]],
        );
        outside.faces.pop();
        outside.faces[0].holes.push(WireId(1));
        let (kind, entity, message) = trimmed(outside, &[]);
        assert_eq!((kind, entity), (ErrorKind::InvalidGeometry, Some(id(1))));
        assert!(
            message.ends_with("hole 1 does not lie inside the outer bound (hole outside)"),
            "{message}"
        );
        let clockwise = polygons(
            &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            &[vec![3, 2, 1, 0]],
        );
        let (_, _, message) = trimmed(clockwise, &[]);
        assert!(
            message.contains("the outer bound winds the wrong way")
                && message.ends_with("(wrong orientation)"),
            "{message}"
        );
        let options = tessstep_trim::Options {
            max_samples: 3,
            ..tessstep_trim::Options::default()
        };
        let (kind, _, message) = failure(&solid(square(), &[]), options);
        assert_eq!(kind, ErrorKind::ResourceLimit);
        assert!(
            message.ends_with("the trim max_samples budget (3) is exhausted (budget)"),
            "{message}"
        );
        // Other tessellation failures are left to the general mapping.
        assert!(
            describe(
                &solid(square(), &[]),
                FaceId(0),
                &TessellationErrorKind::UnresolvedTolerance
            )
            .is_none()
        );
    }
}
