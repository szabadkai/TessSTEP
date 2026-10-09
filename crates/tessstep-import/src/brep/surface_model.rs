//! Shell-based surface models: the OPEN_SHELLs and CLOSED_SHELLs of a
//! `SHELL_BASED_SURFACE_MODEL`, imported with the curved B-rep profile's face and
//! edge adapters and meshed one shell at a time. A surface model is never a solid:
//! it has no solid record, closed shells are checked for closure but not for volume
//! or outward orientation, and open shells keep their boundary edges.
use super::*;

/// One shell of an imported surface model: an owned, validated B-rep holding just
/// this shell (its own copies of any vertex or edge it shares with another shell of
/// the model), so its mesh depends on no other shell.
#[derive(Clone, Debug)]
pub struct SurfaceShell {
    entity: EntityId,
    closed: bool,
    brep: NormalizedBrep,
    faces: Vec<EntityId>,
    boundary_edges: usize,
}
impl SurfaceShell {
    /// The entity the model references: an OPEN_SHELL, a CLOSED_SHELL, or an
    /// ORIENTED_OPEN_SHELL / ORIENTED_CLOSED_SHELL whose orientation was applied.
    pub fn entity(&self) -> EntityId {
        self.entity
    }
    /// Declared closed (CLOSED_SHELL): topology required two uses of every edge and
    /// the mesh must be watertight. An open shell may have boundary edges.
    pub fn closed(&self) -> bool {
        self.closed
    }
    /// One shell (`ShellId(0)`) and no solid.
    pub fn brep(&self) -> &NormalizedBrep {
        &self.brep
    }
    /// The STEP face entity of each model-local face (after any reported sliver-face
    /// collapse).
    pub fn face_entities(&self) -> &[EntityId] {
        &self.faces
    }
    /// B-rep edges with one use in the shell, excluding collapsed pole and apex
    /// edges: the open boundary. Always zero for a closed shell.
    pub fn boundary_edges(&self) -> usize {
        self.boundary_edges
    }
    /// Returns an owned mesh of the shell in metres, through
    /// `tessstep_tessellate::tessellate_shell`: one connected component, watertight
    /// when the shell is declared closed, never checked as a solid (no volume or
    /// outward orientation). Triangle face_ids are physical STEP IDs.
    pub fn tessellate(
        &self,
        tolerance: TessellationTolerance,
        options: TessellationOptions,
    ) -> Result<tessstep_mesh::Mesh, Error> {
        let mesh =
            tessstep_tessellate::tessellate_shell(&self.brep, ShellId(0), tolerance, options)
                .map_err(|e| Error {
                    kind: if e.kind == tessstep_tessellate::TessellationErrorKind::InvalidOptions {
                        ErrorKind::InvalidOptions
                    } else if tessellation_limit(&e.kind) {
                        ErrorKind::ResourceLimit
                    } else {
                        ErrorKind::InvalidGeometry
                    },
                    stage: Stage::Tessellation,
                    entity: e
                        .face
                        .and_then(|f| self.faces.get(f.0).copied())
                        .or(Some(self.entity)),
                    source: None,
                    message: e.to_string(),
                })?;
        let mut data = mesh.into_data();
        for id in &mut data.face_ids {
            *id = self.faces[*id as usize].get();
        }
        tessstep_mesh::Mesh::new(data, options.mesh).map_err(|e| Error {
            kind: if e == tessstep_mesh::Error::ResourceLimit {
                ErrorKind::ResourceLimit
            } else {
                ErrorKind::InvalidGeometry
            },
            stage: Stage::Tessellation,
            entity: Some(self.entity),
            source: None,
            message: e.to_string(),
        })
    }
}

/// The validated shells of one surface model, in `SBSM_BOUNDARY` order.
#[derive(Clone, Debug)]
pub struct ImportedSurfaceModel {
    root: EntityId,
    shells: Vec<SurfaceShell>,
    adaptations: Adaptations,
}
impl ImportedSurfaceModel {
    /// The SHELL_BASED_SURFACE_MODEL entity.
    pub fn root(&self) -> EntityId {
        self.root
    }
    /// Representation adaptations applied by the curved importer, over all shells.
    pub fn adaptations(&self) -> Adaptations {
        self.adaptations
    }
    pub fn shells(&self) -> &[SurfaceShell] {
        &self.shells
    }
    /// One mesh per shell, in shell order; the first failing shell fails the model.
    /// Each shell has the full tessellation budgets.
    pub fn tessellate(
        &self,
        tolerance: TessellationTolerance,
        options: TessellationOptions,
    ) -> Result<Vec<tessstep_mesh::Mesh>, Error> {
        self.shells
            .iter()
            .map(|shell| shell.tessellate(tolerance, options))
            .collect()
    }
}

/// Import one selected SHELL_BASED_SURFACE_MODEL whose `SBSM_BOUNDARY` holds
/// OPEN_SHELLs and CLOSED_SHELLs (or their ORIENTED_ forms) of curved-profile faces
/// (see [`import_brep_solid`] for the faces, edges, units and adaptations). The model
/// is validated as one B-rep with a kernel shell per declared shell, open or closed as
/// declared: connected through its edges, at most two uses per edge with opposed
/// orientations, and two uses of every edge when closed. No solid is built, so no
/// volume or outward orientation is required. Each shell is then kept as its own
/// validated B-rep. VERTEX_SHELL and WIRE_SHELL boundaries are outside the profile.
/// Shells keep their EDGE_CURVE and VERTEX_POINT identities; nothing is sewn: the
/// opt-in repair stage (`options.repair`) does not apply, since sewing could join
/// declared shells. The tolerance policy (`options.deviation_factor`) does, and each
/// shell is validated at the working tolerance it implies.
pub fn import_surface_model(
    document: &Document,
    root: EntityId,
    length: LengthUnit,
    angle: AngleUnit,
    tolerance: ModelTolerance,
    options: ImportOptions,
) -> Result<ImportedSurfaceModel, Error> {
    let options = ImportOptions {
        repair: RepairOptions::default(),
        ..options
    };
    let mut declared = Vec::new();
    let imported = import_shells(
        document,
        root,
        length,
        angle,
        tolerance,
        options,
        Some(&mut declared),
    )?;
    // Accepted deviations widen the validation of every shell, as for solids.
    let working = imported.brep().model_tolerance();
    let tolerance = if working > tolerance.distance().as_metres() {
        Length::metres(working)
            .ok()
            .and_then(|distance| ModelTolerance::new(distance, tolerance.angle()).ok())
            .ok_or_else(|| Error {
                kind: ErrorKind::InvalidGeometry,
                stage: Stage::Topology,
                entity: Some(root),
                source: None,
                message: format!("working tolerance {working:e} m is not a valid model tolerance"),
            })?
    } else {
        tolerance
    };
    let mut shells = Vec::new();
    for (index, (entity, closed)) in declared.into_iter().enumerate() {
        let (raw, faces) = extract(&imported, index);
        let edges = raw.edges.len();
        let brep = raw
            .validate(
                tolerance,
                ValidationLimits {
                    max_records: options.max_records,
                    max_work: options.max_work,
                },
            )
            .map_err(|e| Error {
                kind: if e.defect == Defect::ResourceLimit {
                    ErrorKind::ResourceLimit
                } else {
                    ErrorKind::InvalidGeometry
                },
                stage: Stage::Topology,
                entity: Some(entity),
                source: document.entities().get(entity).map(|e| e.source),
                message: e.to_string(),
            })?
            .normalize();
        let boundary_edges = (0..edges)
            .filter(|&e| {
                let edge = EdgeId(e);
                brep.edge_uses(edge).is_some_and(|uses| uses.len() == 1) && !brep.is_collapsed(edge)
            })
            .count();
        shells.push(SurfaceShell {
            entity,
            closed,
            brep,
            faces,
            boundary_edges,
        });
    }
    Ok(ImportedSurfaceModel {
        root,
        shells,
        adaptations: imported.adaptations,
    })
}

/// The records of one shell of a validated model, renumbered in first-use order,
/// and the STEP entity of each of its faces.
fn extract(model: &ImportedSolid, shell: usize) -> (RawBrep, Vec<EntityId>) {
    let data = model.brep.data();
    let mut raw = RawBrep::default();
    let mut vertices = BTreeMap::new();
    let mut edges = BTreeMap::new();
    let mut faces = Vec::new();
    for &face in &data.shells[shell].faces {
        let source = &data.faces[face.0];
        let mut wires = Vec::new();
        for wire in std::iter::once(&source.outer).chain(&source.holes) {
            let mut coedges = Vec::new();
            for &coedge in &data.wires[wire.0].coedges {
                let use_ = &data.coedges[coedge.0];
                let edge = *edges.entry(use_.edge).or_insert_with(|| {
                    let e = &data.edges[use_.edge.0];
                    let ends = e.vertices.map(|v| {
                        *vertices.entry(v).or_insert_with(|| {
                            raw.vertices.push(data.vertices[v.0].clone());
                            VertexId(raw.vertices.len() - 1)
                        })
                    });
                    raw.edges.push(Edge {
                        vertices: ends,
                        ..e.clone()
                    });
                    EdgeId(raw.edges.len() - 1)
                });
                raw.coedges.push(Coedge {
                    edge,
                    ..use_.clone()
                });
                coedges.push(CoedgeId(raw.coedges.len() - 1));
            }
            raw.wires.push(Wire { coedges });
            wires.push(WireId(raw.wires.len() - 1));
        }
        raw.faces.push(Face {
            outer: wires[0],
            holes: wires[1..].to_vec(),
            ..source.clone()
        });
        faces.push(model.faces[face.0]);
    }
    raw.shells.push(Shell {
        faces: (0..raw.faces.len()).map(FaceId).collect(),
        closed: data.shells[shell].closed,
    });
    (raw, faces)
}

/// The declared shells of a surface model root, recorded in `declared` with their
/// closure; the caller applies shell orientation and collects their faces.
pub(super) fn shells<'d, 'a>(
    c: &mut Context<'d, 'a>,
    model: &'d EntityView<'a>,
    declared: &mut Vec<(EntityId, bool)>,
) -> Result<Vec<&'d EntityView<'a>>, Error> {
    if !c.is(model, "SHELL_BASED_SURFACE_MODEL") {
        return Err(c.error(
            ErrorKind::Unsupported,
            "selected root is not a SHELL_BASED_SURFACE_MODEL",
        ));
    }
    let mut shells = Vec::new();
    for value in c.aggregate(model, "SBSM_BOUNDARY")? {
        c.charge(1)?;
        let shell = c.target(value)?;
        declared.push((shell.id, c.is(shell, "CLOSED_SHELL")));
        shells.push(shell);
    }
    Ok(shells)
}

/// A reference or SELECT type rejection that names the attribute and the referenced
/// record, for example an OPEN_SHELL as the OUTER shell of a MANIFOLD_SOLID_BREP or a
/// face in a surface model's SBSM_BOUNDARY. Only the failing entity's own parameters
/// are searched, for a reference at the decoder's reported source span; any other
/// value keeps the decoder's message.
pub(super) fn reference_type(document: &Document, e: &decode::Error) -> String {
    let entities = document.entities();
    let target = e.entity.and_then(|id| entities.get(id)).and_then(|entity| {
        let mut stack: Vec<&StepValue> = entity
            .kind
            .records()
            .iter()
            .flat_map(|r| &r.parameters)
            .collect();
        while let Some(v) = stack.pop() {
            match &v.kind {
                ValueKind::Reference(id) if Some(v.source) == e.source => return Some(*id),
                ValueKind::Aggregate(items) => stack.extend(items),
                ValueKind::Typed { value, .. } => stack.push(value),
                _ => {}
            }
        }
        None
    });
    match target.and_then(|id| entities.get(id).map(|t| (id, t))) {
        Some((id, t)) => {
            let names: Vec<&str> = t.kind.records().iter().map(|r| r.name.as_ref()).collect();
            format!(
                "{e}: {} references {} #{}",
                e.attribute.unwrap_or("an attribute"),
                names.join("+"),
                id.get()
            )
        }
        None => e.to_string(),
    }
}
