//! Existing triangulated tessellations to owned meshes, without retessellation.
//! Vertices are identified by (COORDINATES_LIST, point index), never by proximity;
//! connecting edges join exactly the identities they declare equal.
use crate::{Error, ErrorKind, ImportOptions, Stage};
use std::collections::{BTreeMap, BTreeSet};
use tessstep_math::{LengthUnit, ModelSpace, Point, Vector};
use tessstep_mesh::{Mesh, MeshData};
use tessstep_model::{
    Document,
    decode::{self, DecodedDocument, EntityView},
};
use tessstep_part21::{EntityId, StepValue, ValueKind};

mod presentation;
pub use presentation::{
    ImportedPresentation, PresentationItem, PresentationItemKind, PresentationLimits,
    import_presentation,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TessellatedKind {
    Solid,
    Shell,
    SurfaceSet,
}
impl TessellatedKind {
    pub fn step_name(self) -> &'static str {
        match self {
            Self::Solid => "TESSELLATED_SOLID",
            Self::Shell => "TESSELLATED_SHELL",
            Self::SurfaceSet => "TESSELLATED_SURFACE_SET",
        }
    }
}
/// One source face (or the surface set itself) and its retained link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TessellatedFace {
    pub entity: EntityId,
    /// GEOMETRIC_LINK target (a face or surface); retained, never decoded or checked.
    pub geometric_link: Option<EntityId>,
    /// Corner normals come from the file rather than from triangle winding.
    pub supplied_normals: bool,
}
/// The faces a TESSELLATED_CONNECTING_EDGE joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeConnection {
    pub faces: [EntityId; 2],
    /// SMOOTH: whether the faces meet tangentially; None when the file says UNKNOWN.
    pub smooth: Option<bool>,
}
/// A TESSELLATED_EDGE or TESSELLATED_CONNECTING_EDGE item: a polyline that adds no
/// triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct TessellatedEdge {
    pub entity: EntityId,
    /// GEOMETRIC_LINK target (an edge or curve); retained, never decoded or checked.
    pub geometric_link: Option<EntityId>,
    /// Line-strip points in metres.
    pub points: Vec<[f64; 3]>,
    /// Mesh vertex of each point whose coordinate identity is a triangle corner.
    pub vertices: Vec<Option<u32>>,
    pub connection: Option<EdgeConnection>,
}
/// A TESSELLATED_VERTEX item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TessellatedVertex {
    pub entity: EntityId,
    /// TOPOLOGICAL_LINK target (a vertex point); retained, never decoded or checked.
    pub topological_link: Option<EntityId>,
    /// Metres.
    pub position: [f64; 3],
    /// Mesh vertex when the coordinate identity is a triangle corner.
    pub vertex: Option<u32>,
}
/// An owned mesh in metres. Triangle face_ids are the physical STEP IDs of faces.
#[derive(Clone, Debug)]
pub struct ImportedTessellation {
    mesh: Mesh,
    kind: TessellatedKind,
    root: EntityId,
    link: Option<EntityId>,
    faces: Vec<TessellatedFace>,
    edges: Vec<TessellatedEdge>,
    vertices: Vec<TessellatedVertex>,
    skipped: usize,
    joined: usize,
    deviations: usize,
}
impl ImportedTessellation {
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }
    pub fn into_mesh(self) -> Mesh {
        self.mesh
    }
    pub fn kind(&self) -> TessellatedKind {
        self.kind
    }
    pub fn root(&self) -> EntityId {
        self.root
    }
    /// TESSELLATED_SOLID geometric_link or TESSELLATED_SHELL topological_link target,
    /// retained without decoding.
    pub fn link(&self) -> Option<EntityId> {
        self.link
    }
    pub fn faces(&self) -> &[TessellatedFace] {
        &self.faces
    }
    /// Edge items in item order.
    pub fn edges(&self) -> &[TessellatedEdge] {
        &self.edges
    }
    /// Vertex items in item order.
    pub fn vertices(&self) -> &[TessellatedVertex] {
        &self.vertices
    }
    /// Strip/fan triangles with a repeated index (the usual strip-stitching idiom),
    /// omitted rather than published as degenerate triangles.
    pub fn skipped_degenerate(&self) -> usize {
        self.skipped
    }
    /// Coordinate identities that connecting edges joined to a different identity.
    pub fn joined_points(&self) -> usize {
        self.joined
    }
    /// Faces whose PNMAX understated NPOINTS without a PNINDEX, accepted by the
    /// tolerant default (see `import_tessellated`).
    pub fn pnmax_deviations(&self) -> usize {
        self.deviations
    }
}

/// Import a selected TESSELLATED_SOLID, TESSELLATED_SHELL or triangulated surface set
/// whose faces are TRIANGULATED_FACE or COMPLEX_TRIANGULATED_FACE. Shell and solid
/// items may also be TESSELLATED_EDGE, TESSELLATED_CONNECTING_EDGE and
/// TESSELLATED_VERTEX; they add no triangles. A connecting edge declares that the
/// i-th points of its line strip and of each face's local strip are one point: those
/// identities must have equal coordinates and become one mesh vertex, and each strip
/// segment must be a triangle edge of both faces. Only the root's reference closure
/// is checked against the bundled original reduced profile; link attributes are
/// retained without following them. Solids must form one closed, positively oriented
/// manifold component; shells and surface sets may be open but must still be
/// edge/vertex manifold with consistent winding. Supplied normals must agree with
/// triangle winding and are otherwise rejected, never flipped. No retessellation,
/// repair, proximity welding, unit inference or placement is applied. The tolerant
/// default accepts one exporter deviation that leaves geometry unchanged: without a
/// PNINDEX, a PNMAX below NPOINTS (with at most one normal) indexes the whole list and
/// is counted by `pnmax_deviations`. `options.strict` rejects it.
pub fn import_tessellated(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    options: ImportOptions,
    mesh_limits: tessstep_mesh::Limits,
) -> Result<ImportedTessellation, Error> {
    let decoded = decode_profile(document, root, options)?;
    let mut c = Context::new(&decoded, root, options, unit, mesh_limits);
    let view = c.view(root)?;
    let (kind, link, sources) = if c.is(view, "TESSELLATED_SOLID") {
        let link = c.link(view, "GEOMETRIC_LINK")?;
        (TessellatedKind::Solid, link, c.items(view)?)
    } else if c.is(view, "TESSELLATED_SHELL") {
        let link = c.link(view, "TOPOLOGICAL_LINK")?;
        (TessellatedKind::Shell, link, c.items(view)?)
    } else if c.is(view, "TESSELLATED_SURFACE_SET") {
        (TessellatedKind::SurfaceSet, None, vec![view])
    } else {
        return Err(c.error(
            ErrorKind::Unsupported,
            "selected root is not a tessellated solid, shell or surface set",
        ));
    };
    let mut plans = Vec::new();
    let mut others = Vec::new();
    for source in sources {
        if c.is(source, "TESSELLATED_FACE") || c.is(source, "TESSELLATED_SURFACE_SET") {
            plans.push(c.plan(source, true)?);
        } else if c.is(source, "TESSELLATED_EDGE") || c.is(source, "TESSELLATED_VERTEX") {
            others.push(source);
        } else {
            c.current = source.id;
            return Err(c.error(
                ErrorKind::Unsupported,
                "tessellation item is not a face, edge or vertex",
            ));
        }
    }
    let index: BTreeMap<EntityId, usize> = plans
        .iter()
        .enumerate()
        .map(|(i, p)| (p.entity, i))
        .collect();
    // Connecting edges establish identity before any mesh vertex is created.
    let mut edge_cache = BTreeMap::new();
    let mut strips = Vec::new();
    for &item in &others {
        strips.push(c.strip(item, &plans, &index, &mut edge_cache)?);
    }
    let mut faces = Vec::new();
    for plan in &plans {
        faces.push(c.emit(plan)?);
    }
    let mut edges = Vec::new();
    let mut vertices = Vec::new();
    for (item, (identities, connection)) in others.into_iter().zip(strips) {
        c.current = item.id;
        let mut points = Vec::new();
        let mut corners = Vec::new();
        for &identity in &identities {
            c.charge(1)?;
            points.push(c.position(identity)?);
            let canonical = c.find(identity)?;
            corners.push(c.vertices.get(&canonical).copied());
        }
        if c.is(item, "TESSELLATED_VERTEX") {
            vertices.push(TessellatedVertex {
                entity: item.id,
                topological_link: c.link(item, "TOPOLOGICAL_LINK")?,
                position: points[0],
                vertex: corners[0],
            });
        } else {
            edges.push(TessellatedEdge {
                entity: item.id,
                geometric_link: c.link(item, "GEOMETRIC_LINK")?,
                points,
                vertices: corners,
                connection,
            });
        }
    }
    let joined = c.parent.len();
    let (skipped, deviations) = (c.skipped, c.deviations);
    let mesh = Mesh::new(c.data, mesh_limits).map_err(|e| mesh_error(e, document, root))?;
    if kind == TessellatedKind::Solid {
        mesh.require_solid().map_err(|e| Error {
            message: if e == tessstep_mesh::Error::Open {
                "tessellated solid is not closed under coordinate-list index identity; \
                 positions are never welded by proximity"
                    .into()
            } else {
                format!("tessellated solid: {e}")
            },
            ..mesh_error(e, document, root)
        })?;
    }
    Ok(ImportedTessellation {
        mesh,
        kind,
        root,
        link,
        faces,
        edges,
        vertices,
        skipped,
        joined,
        deviations,
    })
}

/// Decode the selected closure with every provenance link retained but not followed.
pub(crate) fn decode_profile<'a>(
    document: &'a Document,
    root: EntityId,
    options: ImportOptions,
) -> Result<DecodedDocument<'a>, Error> {
    use crate::tessellated_profile::schema_tessstep_tessellated as schema;
    use tessstep_schema::EntityBinding;
    let slot = |entity, attribute| decode::LinkSlot { entity, attribute };
    let links = [
        slot(
            schema::Entity_TESSELLATED_FACE::DECLARATION,
            "GEOMETRIC_LINK",
        ),
        slot(
            schema::Entity_TESSELLATED_SOLID::DECLARATION,
            "GEOMETRIC_LINK",
        ),
        slot(
            schema::Entity_TESSELLATED_SHELL::DECLARATION,
            "TOPOLOGICAL_LINK",
        ),
        slot(
            schema::Entity_TESSELLATED_EDGE::DECLARATION,
            "GEOMETRIC_LINK",
        ),
        slot(
            schema::Entity_TESSELLATED_VERTEX::DECLARATION,
            "TOPOLOGICAL_LINK",
        ),
        slot(schema::Entity_STYLED_ITEM::DECLARATION, "STYLES"),
    ];
    decode::decode_reachable_profile_with_links(
        document,
        &crate::tessellated_profile::SCHEMA_SET,
        "tessstep_tessellated",
        &[root],
        &[],
        &links,
        decode::Limits {
            max_work: options.max_work,
            ..decode::Limits::default()
        },
    )
    .map_err(|e| Error {
        kind: match e.kind {
            decode::ErrorKind::ResourceLimit => ErrorKind::ResourceLimit,
            decode::ErrorKind::UnknownEntity | decode::ErrorKind::Unsupported => {
                ErrorKind::Unsupported
            }
            decode::ErrorKind::MissingReference => ErrorKind::MissingEntity,
            _ => ErrorKind::InvalidGeometry,
        },
        stage: Stage::Profile,
        entity: e.entity,
        source: e.source,
        message: if e.kind == decode::ErrorKind::UnknownEntity {
            crate::outside_profile(
                document,
                &crate::tessellated_profile::SCHEMA_SET,
                "tessstep_tessellated",
                e.entity,
            )
        } else {
            e.to_string()
        },
    })
}
fn winding_normal(corners: [[f64; 3]; 3]) -> Result<Vector<ModelSpace, 3>, tessstep_math::Error> {
    let [a, b, c] = corners.map(Point::<ModelSpace, 3>::new);
    let a = a?;
    Ok(b?
        .difference(a)?
        .cross(c?.difference(a)?)?
        .normalized()?
        .vector())
}
fn mesh_error(e: tessstep_mesh::Error, document: &Document, root: EntityId) -> Error {
    use tessstep_mesh::Error as E;
    Error {
        kind: if e == E::ResourceLimit {
            ErrorKind::ResourceLimit
        } else {
            ErrorKind::InvalidGeometry
        },
        stage: match e {
            E::NonFinite | E::DegenerateTriangle | E::InvalidAttributes | E::InvalidIndex => {
                Stage::Geometry
            }
            _ => Stage::Topology,
        },
        entity: Some(root),
        source: document.entities().get(root).map(|e| e.source),
        message: e.to_string(),
    }
}
/// A coordinate identity: (COORDINATES_LIST entity, zero-based point index).
type Identity = (EntityId, usize);
/// A face or surface set whose indices were checked but whose vertices are not yet
/// created, so connecting edges can join identities first.
struct FacePlan {
    entity: EntityId,
    list: EntityId,
    /// Face-local (zero-based) to coordinate-list index; empty means identity.
    map: Vec<usize>,
    pnmax: usize,
    normals: Vec<Vector<ModelSpace, 3>>,
    /// Face-local, zero-based.
    triangles: Vec<[usize; 3]>,
    link: Option<EntityId>,
    supplied: bool,
}
impl FacePlan {
    fn identity(&self, local: usize) -> Identity {
        (self.list, self.map.get(local).copied().unwrap_or(local))
    }
}
struct Context<'d, 'a> {
    decoded: &'d DecodedDocument<'a>,
    current: EntityId,
    remaining: usize,
    records: usize,
    unit: LengthUnit,
    mesh_limits: tessstep_mesh::Limits,
    /// Coordinate lists whose point count has been checked.
    lists: BTreeMap<EntityId, &'a [StepValue]>,
    /// Identities joined by connecting edges, pointing towards the smallest member.
    parent: BTreeMap<Identity, Identity>,
    vertices: BTreeMap<Identity, u32>,
    data: MeshData,
    skipped: usize,
    strict: bool,
    /// Faces or surface sets whose pnmax understated npoints (tolerated).
    deviations: usize,
}
impl<'d, 'a> Context<'d, 'a> {
    fn new(
        decoded: &'d DecodedDocument<'a>,
        root: EntityId,
        options: ImportOptions,
        unit: LengthUnit,
        mesh_limits: tessstep_mesh::Limits,
    ) -> Self {
        Self {
            decoded,
            current: root,
            remaining: options.max_work,
            records: options.max_records.min(1_000_000),
            unit,
            mesh_limits,
            lists: BTreeMap::new(),
            parent: BTreeMap::new(),
            vertices: BTreeMap::new(),
            data: MeshData::default(),
            skipped: 0,
            strict: options.strict,
            deviations: 0,
        }
    }
    fn error(&self, kind: ErrorKind, message: impl ToString) -> Error {
        Error {
            kind,
            stage: Stage::Geometry,
            entity: Some(self.current),
            source: self
                .decoded
                .document()
                .entities()
                .get(self.current)
                .map(|e| e.source),
            message: message.to_string(),
        }
    }
    fn invalid(&self, message: &str) -> Error {
        self.error(ErrorKind::InvalidGeometry, message)
    }
    fn checked<T, E: std::fmt::Display>(&self, value: Result<T, E>) -> Result<T, Error> {
        value.map_err(|e| self.error(ErrorKind::InvalidGeometry, e))
    }
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        self.remaining = self
            .remaining
            .checked_sub(n)
            .ok_or_else(|| self.error(ErrorKind::ResourceLimit, "adapter work budget"))?;
        Ok(())
    }
    /// One face, edge, vertex or coordinate list against the record budget.
    fn record(&mut self) -> Result<(), Error> {
        self.charge(1)?;
        self.records = self
            .records
            .checked_sub(1)
            .ok_or_else(|| self.error(ErrorKind::ResourceLimit, "tessellation record budget"))?;
        Ok(())
    }
    fn is(&self, v: &EntityView<'_>, name: &str) -> bool {
        v.types.iter().any(|id| {
            self.decoded
                .schemas()
                .declaration(*id)
                .is_some_and(|d| d.name == name)
        })
    }
    fn view(&self, id: EntityId) -> Result<&'d EntityView<'a>, Error> {
        self.decoded
            .get(id)
            .ok_or_else(|| self.error(ErrorKind::MissingEntity, "entity outside selected closure"))
    }
    fn attr(&self, v: &'d EntityView<'a>, name: &str) -> Result<&'a StepValue, Error> {
        v.attributes
            .iter()
            .find(|a| a.declaration.name == name)
            .map(|a| a.value)
            .ok_or_else(|| self.error(ErrorKind::Unsupported, "profile attribute absent"))
    }
    fn target(&self, v: &StepValue) -> Result<&'d EntityView<'a>, Error> {
        if let ValueKind::Reference(id) = v.kind {
            self.view(id)
        } else {
            Err(self.invalid("expected entity reference"))
        }
    }
    fn values(&self, v: &'a StepValue) -> Result<&'a [StepValue], Error> {
        if let ValueKind::Aggregate(values) = &v.kind {
            Ok(values)
        } else {
            Err(self.invalid("expected aggregate"))
        }
    }
    fn aggregate(&self, v: &'d EntityView<'a>, name: &str) -> Result<&'a [StepValue], Error> {
        self.values(self.attr(v, name)?)
    }
    fn link(&self, v: &'d EntityView<'a>, name: &str) -> Result<Option<EntityId>, Error> {
        match self.attr(v, name)?.kind {
            ValueKind::Reference(id) => Ok(Some(id)),
            _ => Ok(None),
        }
    }
    fn items(&mut self, v: &'d EntityView<'a>) -> Result<Vec<&'d EntityView<'a>>, Error> {
        let mut items = Vec::new();
        for item in self.aggregate(v, "ITEMS")? {
            self.charge(1)?;
            items.push(self.target(item)?);
        }
        Ok(items)
    }
    fn count(&self, v: &StepValue) -> Result<usize, Error> {
        match v.kind {
            ValueKind::Integer(n) => usize::try_from(n).map_err(|_| self.invalid("negative count")),
            _ => Err(self.invalid("expected integer")),
        }
    }
    fn numbers(&self, v: &'a StepValue) -> Result<[f64; 3], Error> {
        let values = self.values(v)?;
        if values.len() != 3 {
            return Err(self.invalid("expected three components"));
        }
        let mut xyz = [0.; 3];
        for (out, value) in xyz.iter_mut().zip(values) {
            *out = match value.kind {
                ValueKind::Real(v) => v,
                ValueKind::Integer(v) => v as f64,
                _ => return Err(self.invalid("expected numeric component")),
            };
        }
        Ok(xyz)
    }
    /// 1-based indices checked against `max`, returned 0-based.
    fn indices(&mut self, v: &'a StepValue, max: usize, what: &str) -> Result<Vec<usize>, Error> {
        let mut out = Vec::new();
        for value in self.values(v)? {
            self.charge(1)?;
            match self.count(value)? {
                k @ 1.. if k <= max => out.push(k - 1),
                _ => return Err(self.invalid(what)),
            }
        }
        Ok(out)
    }
    fn coordinates(&mut self, list: &'d EntityView<'a>) -> Result<&'a [StepValue], Error> {
        if let Some(points) = self.lists.get(&list.id) {
            return Ok(points);
        }
        let item = self.current;
        self.current = list.id;
        self.record()?;
        let npoints = self.count(self.attr(list, "NPOINTS")?)?;
        let points = self.aggregate(list, "POSITION_COORDS")?;
        if npoints != points.len() {
            return Err(self.invalid("npoints differs from the coordinate count"));
        }
        self.lists.insert(list.id, points);
        self.current = item;
        Ok(points)
    }
    /// Source coordinates of an identity, located at its coordinate list.
    fn source(&mut self, (list, index): Identity) -> Result<[f64; 3], Error> {
        let item = self.current;
        self.current = list;
        let xyz = self.numbers(&self.lists[&list][index])?;
        self.current = item;
        Ok(xyz)
    }
    fn position(&mut self, identity: Identity) -> Result<[f64; 3], Error> {
        let mut xyz = self.source(identity)?;
        let item = self.current;
        self.current = identity.0;
        for x in &mut xyz {
            *x = self.checked(self.unit.to_metres(*x))?;
        }
        self.checked(Point::<ModelSpace, 3>::new(xyz))?;
        self.current = item;
        Ok(xyz)
    }
    fn find(&mut self, mut identity: Identity) -> Result<Identity, Error> {
        while let Some(&parent) = self.parent.get(&identity) {
            self.charge(1)?;
            identity = parent;
        }
        Ok(identity)
    }
    /// Join two identities that a connecting edge declares to be one point.
    fn join(&mut self, a: Identity, b: Identity) -> Result<(), Error> {
        let (a, b) = (self.find(a)?, self.find(b)?);
        if a == b {
            return Ok(());
        }
        if self.source(a)? != self.source(b)? {
            return Err(self.invalid("connecting edge joins points with different coordinates"));
        }
        // Chains point towards the smallest identity, so every joined class keeps
        // one deterministic representative.
        let (low, high) = (a.min(b), a.max(b));
        self.parent.insert(high, low);
        Ok(())
    }
    fn vertex(&mut self, identity: Identity) -> Result<u32, Error> {
        let identity = self.find(identity)?;
        if let Some(&id) = self.vertices.get(&identity) {
            return Ok(id);
        }
        let id = self.data.positions.len();
        if id >= self.mesh_limits.max_vertices {
            return Err(self.error(ErrorKind::ResourceLimit, "mesh vertex budget"));
        }
        let id = u32::try_from(id)
            .map_err(|_| self.error(ErrorKind::ResourceLimit, "mesh vertex budget"))?;
        let xyz = self.position(identity)?;
        self.data.positions.push(xyz);
        self.vertices.insert(identity, id);
        Ok(id)
    }
    /// Check a face or surface set's counts and indices without creating vertices.
    /// `check_normals` false reads supplied normals as numeric triples only; they are
    /// then neither normalized nor compared with winding (presentation graphics).
    fn plan(&mut self, face: &'d EntityView<'a>, check_normals: bool) -> Result<FacePlan, Error> {
        self.current = face.id;
        self.record()?;
        let list = self.target(self.attr(face, "COORDINATES")?)?;
        let points = self.coordinates(list)?;
        let mut pnmax = self.count(self.attr(face, "PNMAX")?)?;
        let pnindex = self.aggregate(face, "PNINDEX")?;
        let supplied = self.aggregate(face, "NORMALS")?;
        let mut map = Vec::new();
        if pnindex.is_empty() {
            // Without pnindex, pnmax only restates npoints. The tolerant default
            // accepts a smaller value, which some exporters write, and indexes the
            // whole list; per-point normals would be ambiguous and stay rejected.
            let tolerated = !self.strict && pnmax < points.len() && supplied.len() <= 1;
            if pnmax != points.len() && !tolerated {
                return Err(self.invalid("pnmax must equal npoints when pnindex is empty"));
            }
            if pnmax != points.len() {
                self.deviations += 1;
                pnmax = points.len();
            }
        } else {
            if pnindex.len() != pnmax {
                return Err(self.invalid("pnindex length differs from pnmax"));
            }
            for value in pnindex {
                self.charge(1)?;
                match self.count(value)? {
                    p @ 1.. if p <= points.len() => map.push(p - 1),
                    _ => return Err(self.invalid("pnindex entry outside the coordinate list")),
                }
            }
        }
        if supplied.len() > 1 && supplied.len() != pnmax {
            return Err(self.invalid("normal count must be 0, 1 or pnmax"));
        }
        let mut normals = Vec::new();
        for value in supplied {
            self.charge(1)?;
            let n = self.checked(Vector::<ModelSpace, 3>::new(self.numbers(value)?))?;
            if check_normals {
                normals.push(self.checked(n.normalized())?.vector());
            }
        }
        let outside = "vertex index outside 1..=pnmax";
        let mut triangles = Vec::new();
        if self.is(face, "TRIANGULATED_FACE") || self.is(face, "TRIANGULATED_SURFACE_SET") {
            for value in self.aggregate(face, "TRIANGLES")? {
                let t = self.indices(value, pnmax, outside)?;
                if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                    return Err(self.invalid("triangle repeats a vertex index"));
                }
                triangles.push([t[0], t[1], t[2]]);
            }
        } else {
            let strips = self.aggregate(face, "TRIANGLE_STRIPS")?;
            let fans = self.aggregate(face, "TRIANGLE_FANS")?;
            if strips.is_empty() && fans.is_empty() {
                return Err(self.invalid("complex triangulation has no strips or fans"));
            }
            // Strips alternate orientation so every triangle keeps the strip's winding.
            for value in strips {
                let s = self.indices(value, pnmax, outside)?;
                for j in 0..s.len() - 2 {
                    triangles.push(if j % 2 == 0 {
                        [s[j], s[j + 1], s[j + 2]]
                    } else {
                        [s[j + 1], s[j], s[j + 2]]
                    });
                }
            }
            for value in fans {
                let f = self.indices(value, pnmax, outside)?;
                for j in 1..f.len() - 1 {
                    triangles.push([f[0], f[j], f[j + 1]]);
                }
            }
            let before = triangles.len();
            triangles.retain(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
            self.skipped += before - triangles.len();
        }
        Ok(FacePlan {
            entity: face.id,
            list: list.id,
            map,
            pnmax,
            normals,
            triangles,
            link: if self.is(face, "TESSELLATED_FACE") {
                self.link(face, "GEOMETRIC_LINK")?
            } else {
                None
            },
            supplied: !supplied.is_empty(),
        })
    }
    /// Identities of an edge's line strip or a vertex's point. Connecting edges also
    /// join the corresponding face points and check their segments against both faces.
    #[allow(clippy::type_complexity)]
    fn strip(
        &mut self,
        item: &'d EntityView<'a>,
        plans: &[FacePlan],
        index: &BTreeMap<EntityId, usize>,
        edge_cache: &mut BTreeMap<EntityId, BTreeSet<(usize, usize)>>,
    ) -> Result<(Vec<Identity>, Option<EdgeConnection>), Error> {
        self.current = item.id;
        self.record()?;
        let list = self.target(self.attr(item, "COORDINATES")?)?;
        let points = self.coordinates(list)?;
        let outside = "point index outside the coordinate list";
        if self.is(item, "TESSELLATED_VERTEX") {
            let k = self.count(self.attr(item, "POINT_INDEX")?)?;
            if !(1..=points.len()).contains(&k) {
                return Err(self.invalid(outside));
            }
            return Ok((vec![(list.id, k - 1)], None));
        }
        let strip = self.indices(self.attr(item, "LINE_STRIP")?, points.len(), outside)?;
        if strip.windows(2).any(|w| w[0] == w[1]) {
            return Err(self.invalid("line strip repeats a consecutive point"));
        }
        let identities: Vec<Identity> = strip.iter().map(|&k| (list.id, k)).collect();
        if !self.is(item, "TESSELLATED_CONNECTING_EDGE") {
            return Ok((identities, None));
        }
        let smooth = match &self.attr(item, "SMOOTH")?.kind {
            ValueKind::Enumeration(s) if s.as_ref() == "T" => Some(true),
            ValueKind::Enumeration(s) if s.as_ref() == "F" => Some(false),
            _ => None,
        };
        let mut faces = [item.id; 2];
        for (side, (face, local)) in [("FACE1", "LINE_STRIP_FACE1"), ("FACE2", "LINE_STRIP_FACE2")]
            .into_iter()
            .enumerate()
        {
            let face = self.target(self.attr(item, face)?)?;
            let Some(&i) = index.get(&face.id) else {
                return Err(
                    self.invalid("connecting edge face is not an item of this tessellation")
                );
            };
            let plan = &plans[i];
            faces[side] = plan.entity;
            let local = self.indices(
                self.attr(item, local)?,
                plan.pnmax,
                "face strip index outside 1..=pnmax",
            )?;
            if local.len() != identities.len() {
                return Err(self.invalid("face strip length differs from the line strip"));
            }
            if let std::collections::btree_map::Entry::Vacant(slot) = edge_cache.entry(plan.entity)
            {
                let mut edges = BTreeSet::new();
                for t in &plan.triangles {
                    self.charge(1)?;
                    for k in 0..3 {
                        let (a, b) = (t[k], t[(k + 1) % 3]);
                        edges.insert((a.min(b), a.max(b)));
                    }
                }
                slot.insert(edges);
            }
            let edges = &edge_cache[&plan.entity];
            if local
                .windows(2)
                .any(|w| !edges.contains(&(w[0].min(w[1]), w[0].max(w[1]))))
            {
                return Err(
                    self.invalid("connecting edge segment is not a triangle edge of its face")
                );
            }
            for (&k, &identity) in local.iter().zip(&identities) {
                self.charge(1)?;
                self.join(identity, plan.identity(k))?;
            }
        }
        Ok((identities, Some(EdgeConnection { faces, smooth })))
    }
    fn emit(&mut self, plan: &FacePlan) -> Result<TessellatedFace, Error> {
        self.current = plan.entity;
        let id = plan.entity.get();
        for t in &plan.triangles {
            self.charge(1)?;
            if self.data.triangles.len() >= self.mesh_limits.max_triangles {
                return Err(self.error(ErrorKind::ResourceLimit, "mesh triangle budget"));
            }
            let mut corners = [0; 3];
            for (corner, &k) in corners.iter_mut().zip(t) {
                *corner = self.vertex(plan.identity(k))?;
            }
            let geometric = winding_normal(corners.map(|i| self.data.positions[i as usize]))
                .map_err(|_| self.invalid("degenerate triangle"))?;
            let corner_normals = match plan.normals.len() {
                0 => [geometric; 3],
                1 => [plan.normals[0]; 3],
                _ => t.map(|k| plan.normals[k]),
            };
            for n in corner_normals {
                if self.checked(n.dot(geometric))? <= 0. {
                    return Err(self.invalid("supplied normal opposes triangle winding"));
                }
            }
            self.data.triangles.push(corners);
            self.data
                .normals
                .push(corner_normals.map(|n| n.components()));
            self.data.uvs.push([[0.; 2]; 3]);
            self.data.face_ids.push(id);
        }
        Ok(TessellatedFace {
            entity: plan.entity,
            geometric_link: plan.link,
            supplied_normals: !plan.normals.is_empty(),
        })
    }
}
