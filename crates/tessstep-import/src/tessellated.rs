//! Existing triangulated tessellations to owned meshes, without retessellation.
//! Vertices are identified by (COORDINATES_LIST, point index), never by proximity.
use crate::{Error, ErrorKind, ImportOptions, Stage};
use std::collections::BTreeMap;
use tessstep_math::{LengthUnit, ModelSpace, Point, Vector};
use tessstep_mesh::{Mesh, MeshData};
use tessstep_model::{
    Document,
    decode::{self, DecodedDocument, EntityView},
};
use tessstep_part21::{EntityId, StepValue, ValueKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TessellatedKind {
    Solid,
    Shell,
    SurfaceSet,
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
/// An owned mesh in metres. Triangle face_ids are the physical STEP IDs of faces.
#[derive(Clone, Debug)]
pub struct ImportedTessellation {
    mesh: Mesh,
    kind: TessellatedKind,
    root: EntityId,
    link: Option<EntityId>,
    faces: Vec<TessellatedFace>,
    skipped: usize,
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
    /// Strip/fan triangles with a repeated index (the usual strip-stitching idiom),
    /// omitted rather than published as degenerate triangles.
    pub fn skipped_degenerate(&self) -> usize {
        self.skipped
    }
}

/// Import a selected TESSELLATED_SOLID, TESSELLATED_SHELL or triangulated surface set
/// whose faces are TRIANGULATED_FACE or COMPLEX_TRIANGULATED_FACE. Only the root's
/// reference closure is checked against the bundled original reduced profile; link
/// attributes are retained without following them. Solids must form one closed,
/// positively oriented manifold component; shells and surface sets may be open but
/// must still be edge/vertex manifold with consistent winding. Supplied normals must
/// agree with triangle winding and are otherwise rejected, never flipped. No
/// retessellation, repair, proximity welding, unit inference or placement is applied.
/// `options.strict` has no effect: this profile tolerates no exporter deviations.
pub fn import_tessellated(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    options: ImportOptions,
    mesh_limits: tessstep_mesh::Limits,
) -> Result<ImportedTessellation, Error> {
    use crate::tessellated_profile::schema_tessstep_tessellated as schema;
    use tessstep_schema::EntityBinding;
    let links = [
        decode::LinkSlot {
            entity: schema::Entity_TESSELLATED_FACE::DECLARATION,
            attribute: "GEOMETRIC_LINK",
        },
        decode::LinkSlot {
            entity: schema::Entity_TESSELLATED_SOLID::DECLARATION,
            attribute: "GEOMETRIC_LINK",
        },
        decode::LinkSlot {
            entity: schema::Entity_TESSELLATED_SHELL::DECLARATION,
            attribute: "TOPOLOGICAL_LINK",
        },
    ];
    let decoded = decode::decode_reachable_profile_with_links(
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
    })?;
    let mut c = Context {
        decoded: &decoded,
        current: root,
        remaining: options.max_work,
        records: options.max_records.min(1_000_000),
        unit,
        mesh_limits,
        lists: BTreeMap::new(),
        vertices: BTreeMap::new(),
        data: MeshData::default(),
        skipped: 0,
    };
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
    let mut faces = Vec::new();
    for source in sources {
        faces.push(c.face(source)?);
    }
    let skipped = c.skipped;
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
        skipped,
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
struct Context<'d, 'a> {
    decoded: &'d DecodedDocument<'a>,
    current: EntityId,
    remaining: usize,
    records: usize,
    unit: LengthUnit,
    mesh_limits: tessstep_mesh::Limits,
    /// Coordinate lists whose point count has been checked.
    lists: BTreeMap<EntityId, &'a [StepValue]>,
    vertices: BTreeMap<(EntityId, usize), u32>,
    data: MeshData,
    skipped: usize,
}
impl<'d, 'a> Context<'d, 'a> {
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
    /// One face or coordinate list against the record budget.
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
    /// Local 1-based indices checked against PNMAX, returned 0-based.
    fn indices(&mut self, v: &'a StepValue, pnmax: usize) -> Result<Vec<usize>, Error> {
        let mut out = Vec::new();
        for value in self.values(v)? {
            self.charge(1)?;
            match self.count(value)? {
                k @ 1.. if k <= pnmax => out.push(k - 1),
                _ => return Err(self.invalid("vertex index outside 1..=pnmax")),
            }
        }
        Ok(out)
    }
    fn coordinates(&mut self, list: &'d EntityView<'a>) -> Result<&'a [StepValue], Error> {
        if let Some(points) = self.lists.get(&list.id) {
            return Ok(points);
        }
        let face = self.current;
        self.current = list.id;
        self.record()?;
        let npoints = self.count(self.attr(list, "NPOINTS")?)?;
        let points = self.aggregate(list, "POSITION_COORDS")?;
        if npoints != points.len() {
            return Err(self.invalid("npoints differs from the coordinate count"));
        }
        self.lists.insert(list.id, points);
        self.current = face;
        Ok(points)
    }
    fn vertex(
        &mut self,
        list: EntityId,
        points: &'a [StepValue],
        index: usize,
    ) -> Result<u32, Error> {
        if let Some(&id) = self.vertices.get(&(list, index)) {
            return Ok(id);
        }
        let id = self.data.positions.len();
        if id >= self.mesh_limits.max_vertices {
            return Err(self.error(ErrorKind::ResourceLimit, "mesh vertex budget"));
        }
        let id = u32::try_from(id)
            .map_err(|_| self.error(ErrorKind::ResourceLimit, "mesh vertex budget"))?;
        let face = self.current;
        self.current = list;
        let mut xyz = self.numbers(&points[index])?;
        for x in &mut xyz {
            *x = self.checked(self.unit.to_metres(*x))?;
        }
        self.checked(Point::<ModelSpace, 3>::new(xyz))?;
        self.current = face;
        self.data.positions.push(xyz);
        self.vertices.insert((list, index), id);
        Ok(id)
    }
    fn face(&mut self, face: &'d EntityView<'a>) -> Result<TessellatedFace, Error> {
        self.current = face.id;
        self.record()?;
        let list = self.target(self.attr(face, "COORDINATES")?)?;
        let points = self.coordinates(list)?;
        let pnmax = self.count(self.attr(face, "PNMAX")?)?;
        let pnindex = self.aggregate(face, "PNINDEX")?;
        let mut map = Vec::new();
        if pnindex.is_empty() {
            if pnmax != points.len() {
                return Err(self.invalid("pnmax must equal npoints when pnindex is empty"));
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
        let supplied = self.aggregate(face, "NORMALS")?;
        if supplied.len() > 1 && supplied.len() != pnmax {
            return Err(self.invalid("normal count must be 0, 1 or pnmax"));
        }
        let mut normals = Vec::new();
        for value in supplied {
            self.charge(1)?;
            let n = self.checked(Vector::<ModelSpace, 3>::new(self.numbers(value)?))?;
            normals.push(self.checked(n.normalized())?.vector());
        }
        let mut triangles = Vec::new();
        if self.is(face, "TRIANGULATED_FACE") || self.is(face, "TRIANGULATED_SURFACE_SET") {
            for value in self.aggregate(face, "TRIANGLES")? {
                let t = self.indices(value, pnmax)?;
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
                let s = self.indices(value, pnmax)?;
                for j in 0..s.len() - 2 {
                    triangles.push(if j % 2 == 0 {
                        [s[j], s[j + 1], s[j + 2]]
                    } else {
                        [s[j + 1], s[j], s[j + 2]]
                    });
                }
            }
            for value in fans {
                let f = self.indices(value, pnmax)?;
                for j in 1..f.len() - 1 {
                    triangles.push([f[0], f[j], f[j + 1]]);
                }
            }
            let before = triangles.len();
            triangles.retain(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
            self.skipped += before - triangles.len();
        }
        let id = face.id.get();
        for t in triangles {
            self.charge(1)?;
            if self.data.triangles.len() >= self.mesh_limits.max_triangles {
                return Err(self.error(ErrorKind::ResourceLimit, "mesh triangle budget"));
            }
            let mut corners = [0; 3];
            for (corner, &k) in corners.iter_mut().zip(&t) {
                *corner = self.vertex(list.id, points, map.get(k).copied().unwrap_or(k))?;
            }
            let geometric = winding_normal(corners.map(|i| self.data.positions[i as usize]))
                .map_err(|_| self.invalid("degenerate triangle"))?;
            let corner_normals = match normals.len() {
                0 => [geometric; 3],
                1 => [normals[0]; 3],
                _ => t.map(|k| normals[k]),
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
            entity: face.id,
            geometric_link: if self.is(face, "TESSELLATED_FACE") {
                self.link(face, "GEOMETRIC_LINK")?
            } else {
                None
            },
            supplied_normals: !normals.is_empty(),
        })
    }
}
