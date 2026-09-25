//! Independent B-rep ownership and checked structural validity states.
//! Validation is not proof of nonintersection, volume containment or STEP conformance.
#![forbid(unsafe_code)]
mod geometry;
mod validate;
pub use geometry::*;
use tessstep_math::{ModelSpace, ParameterSpace, Point3};
pub use validate::*;

macro_rules! handles {
    ($($name:ident),*) => {$ (
        /// A model-local index, checked during validation. Not a physical EntityId.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub usize);
    )*};
}
handles!(VertexId, EdgeId, CoedgeId, WireId, FaceId, ShellId, SolidId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Forward,
    Reversed,
}
impl Orientation {
    pub fn is_reversed(self) -> bool {
        self == Self::Reversed
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Vertex {
    pub position: Point3<ModelSpace>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub vertices: [VertexId; 2],
    pub curve: CurveGeometry<ModelSpace, 3>,
    /// Increasing, unwrapped basis parameter interval; orientation belongs to uses.
    pub range: [f64; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Pcurve {
    pub curve: CurveGeometry<ParameterSpace, 2>,
    /// Endpoints correspond to the edge's canonical start and end, even for reversed uses.
    /// This release requires an affine correspondence to the 3D edge parameter.
    pub range: [f64; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Coedge {
    pub edge: EdgeId,
    pub orientation: Orientation,
    pub pcurve: Option<Pcurve>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Wire {
    pub coedges: Vec<CoedgeId>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub surface: SurfaceGeometry,
    pub outer: WireId,
    pub holes: Vec<WireId>,
    pub orientation: Orientation,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Shell {
    pub faces: Vec<FaceId>,
    pub closed: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    /// One closed boundary shell. Cavity/nesting semantics are not yet implemented.
    pub shell: ShellId,
}
/// Handles of different topology kinds cannot be substituted.
/// ```compile_fail
/// use tessstep_topology::{EdgeId, VertexId};
/// let edge: EdgeId = VertexId(0);
/// ```
/// A raw model cannot bypass the checked sampling boundary.
/// ```compile_fail
/// use tessstep_topology::{RawBrep, NormalizedBrep};
/// let normalized: NormalizedBrep = RawBrep::default();
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawBrep {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub coedges: Vec<Coedge>,
    pub wires: Vec<Wire>,
    pub faces: Vec<Face>,
    pub shells: Vec<Shell>,
    pub solids: Vec<Solid>,
}
#[derive(Clone, Debug)]
pub struct ValidatedBrep {
    raw: RawBrep,
    edge_uses: Vec<Vec<CoedgeId>>,
    tolerance: f64,
}
/// Immutable, structurally checked model with canonical edge-use incidence.
/// Normalization does not heal, merge vertices or rewrite geometry.
#[derive(Clone, Debug)]
pub struct NormalizedBrep {
    validated: ValidatedBrep,
}
impl ValidatedBrep {
    pub fn normalize(self) -> NormalizedBrep {
        NormalizedBrep { validated: self }
    }
    pub fn data(&self) -> &RawBrep {
        &self.raw
    }
}
impl NormalizedBrep {
    pub fn data(&self) -> &RawBrep {
        &self.validated.raw
    }
    pub fn edge_uses(&self, edge: EdgeId) -> Option<&[CoedgeId]> {
        self.validated.edge_uses.get(edge.0).map(Vec::as_slice)
    }
    pub fn model_tolerance(&self) -> f64 {
        self.validated.tolerance
    }
}
impl RawBrep {
    pub fn use_vertices(&self, coedge: CoedgeId) -> Option<[VertexId; 2]> {
        let c = self.coedges.get(coedge.0)?;
        let mut v = self.edges.get(c.edge.0)?.vertices;
        if c.orientation.is_reversed() {
            v.reverse();
        }
        Some(v)
    }
}
