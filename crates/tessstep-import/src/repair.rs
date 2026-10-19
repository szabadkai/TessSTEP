//! Opt-in topology repair for the solid importers. Sewing and face orientation run on
//! the raw B-rep before validation (the curved importer sews before it computes
//! pcurves, see `brep/sewing.rs`); after validation, faces whose loops run against
//! their surface are reversed and closed solid shells are oriented outward by the sign
//! of their algebraic volume. Strict rejection is the default: without
//! [`RepairOptions`] enabled the importers validate exactly what the file states.
use super::*;
use tessstep_math::{Angle, Length};
use tessstep_surfaces::SurfaceKind;

/// Opt-in, tolerance-bounded topology repair. Off by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RepairOptions {
    /// Merge coincident boundary vertices (of free edges and wire breaks) and free
    /// edges of different faces whose curves lie within the sewing tolerance.
    pub sew: bool,
    /// Flip faces so each shell is consistently oriented, reverse the loops of faces
    /// whose loops run against their surface, and reverse closed solid shells whose
    /// algebraic volume has the wrong sign (the outer shell must enclose a positive
    /// volume, a cavity shell a negative one).
    pub orient: bool,
    /// The sewing tolerance as a multiple of the model distance tolerance.
    pub sewing_factor: f64,
    /// Distance in metres the sewing tolerance never falls below, whatever
    /// `sewing_factor` makes of the model tolerance: a consumer that sews at one
    /// distance for every file passes it here (PreForm asks HOOPS Exchange to sew at
    /// 0.1 mm). With `sew`, the curved importer also accepts edge and vertex
    /// deviations up to this distance, counted as every accepted deviation is
    /// ([`ImportOptions::deviation_factor`](crate::ImportOptions::deviation_factor)).
    /// Finite and at least 0; the default 0 leaves the factor and the deviation policy
    /// alone.
    pub sewing_distance: f64,
}
impl Default for RepairOptions {
    fn default() -> Self {
        Self {
            sew: false,
            orient: false,
            sewing_factor: Self::DEFAULT_SEWING_FACTOR,
            sewing_distance: 0.,
        }
    }
}
impl RepairOptions {
    /// Default sewing tolerance: ten times the model distance tolerance.
    pub const DEFAULT_SEWING_FACTOR: f64 = 10.;
    /// Sewing and orientation repair at the default sewing tolerance.
    pub fn enabled() -> Self {
        Self {
            sew: true,
            orient: true,
            ..Self::default()
        }
    }
    pub fn is_enabled(self) -> bool {
        self.sew || self.orient
    }
    /// The sewing tolerance in metres for a model tolerance: `sewing_factor` times it,
    /// raised to `sewing_distance`.
    pub fn sewing_tolerance(self, model: ModelTolerance) -> f64 {
        (model.distance().as_metres() * self.sewing_factor).max(self.sewing_distance)
    }
    /// The sewing tolerance, or an options error unless the sewing distance is valid
    /// and the tolerance is finite and positive.
    pub(crate) fn checked_sewing_tolerance(self, model: ModelTolerance) -> Result<f64, Error> {
        self.checked_sewing_distance().map_err(invalid)?;
        let tolerance = self.sewing_tolerance(model);
        if tolerance.is_finite() && tolerance > 0. {
            Ok(tolerance)
        } else {
            Err(invalid(format!(
                "repair sewing_factor {} must give a finite positive sewing tolerance",
                self.sewing_factor
            )))
        }
    }
    /// `sewing_distance`, or the options message unless it is finite and at least 0.
    pub(crate) fn checked_sewing_distance(self) -> Result<f64, String> {
        if self.sewing_distance.is_finite() && self.sewing_distance >= 0. {
            Ok(self.sewing_distance)
        } else {
            Err(format!(
                "repair sewing_distance must be a finite distance of at least 0 m (got {})",
                self.sewing_distance
            ))
        }
    }
}

/// Repairs applied by the opt-in repair stage; all zero when it is off or found
/// nothing to repair. Every repair is counted; geometry is never changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RepairReport {
    /// Vertices merged into a coincident vertex (which keeps its position).
    pub merged_vertices: usize,
    /// Free edges merged into a coincident free edge of another face (whose curve
    /// both faces then use).
    pub merged_edges: usize,
    /// The largest distance between merged vertices or merged edge curves, metres.
    /// A sewn B-rep is validated, trimmed and tessellated at the model tolerance plus
    /// this distance (`NormalizedBrep::model_tolerance`).
    pub max_merge_distance: f64,
    /// Closed shells whose faces shared no edge or vertex before sewing.
    pub disconnected_shells: usize,
    /// Faces whose orientation was flipped to agree with their shell.
    pub flipped_faces: usize,
    /// Faces whose loops ran against their surface (the trimmer found the face to the
    /// right of its outer loop in the surface chart): every loop and the face
    /// orientation were reversed together, which keeps the shell's incidence. Kept
    /// only when the trimmer then accepts the face.
    pub reversed_loops: usize,
    /// Closed solid shells reversed because their algebraic volume had the wrong sign.
    pub reoriented_shells: usize,
    /// Closed solid shells whose volume sign could not be decided: the coarse
    /// tessellation failed or its volume was within its error bound. They keep their
    /// orientation; tessellation reports an inward shell as before.
    pub undecided_shells: usize,
    /// The sewing tolerance in force, metres ([`RepairOptions::sewing_tolerance`]);
    /// zero when sewing is off.
    pub sewing_tolerance: f64,
}
impl RepairReport {
    /// Whether any repair changed the B-rep.
    pub fn is_empty(&self) -> bool {
        self.merged_vertices == 0
            && self.merged_edges == 0
            && self.flipped_faces == 0
            && self.reversed_loops == 0
            && self.reoriented_shells == 0
    }
}

/// Where topology failures are reported: the root, and the STEP entity of each
/// model-local edge where one exists. `vertices` names each model-local vertex where
/// the caller tracks them through [`sew_raw`] (empty otherwise).
pub(crate) struct Locator<'a> {
    pub document: &'a Document,
    pub root: EntityId,
    pub edges: Vec<Option<EntityId>>,
    pub vertices: Vec<EntityId>,
}
impl Locator<'_> {
    fn error(&self, kind: ErrorKind, entity: Option<EntityId>, message: String) -> Error {
        let entity = entity.or(Some(self.root));
        Error {
            kind,
            stage: Stage::Topology,
            entity,
            source: entity
                .and_then(|id| self.document.entities().get(id))
                .map(|e| e.source),
            message,
        }
    }
    fn report(&self, e: Report) -> Error {
        self.error(
            if e.defect == Defect::ResourceLimit {
                ErrorKind::ResourceLimit
            } else {
                ErrorKind::InvalidGeometry
            },
            None,
            e.to_string(),
        )
    }
    fn edge(&self, edge: EdgeId) -> String {
        match self.edges.get(edge.0).copied().flatten() {
            Some(id) => format!("edge #{}", id.get()),
            None => format!("model-local edge {}", edge.0),
        }
    }
    pub(crate) fn repair(&self, e: RepairError, max_work: usize) -> Error {
        if e.report.defect == Defect::ResourceLimit {
            return self.error(
                ErrorKind::ResourceLimit,
                None,
                format!("topology repair work budget exceeded (max_work {max_work})"),
            );
        }
        let mut message = format!("{}: {}", e.report, e.reason);
        match (e.edge, e.partner, e.needed) {
            (Some(edge), Some(partner), Some(needed)) => message.push_str(&format!(
                " ({} to {} needs {needed:.3e} m; sewing tolerance {:.3e} m)",
                self.edge(edge),
                self.edge(partner),
                e.tolerance
            )),
            (Some(edge), _, _) => message.push_str(&format!(" ({})", self.edge(edge))),
            _ => {}
        }
        self.error(
            ErrorKind::InvalidGeometry,
            e.edge
                .and_then(|edge| self.edges.get(edge.0).copied().flatten()),
            message,
        )
    }
}

/// Sew a raw B-rep whose pcurves are straight-edge projections (the planar and
/// faceted profiles). Returns the sewn B-rep, its report and the working tolerance:
/// the model tolerance plus the largest merge distance. The locator's STEP entities per
/// model-local edge and vertex are compacted with the edge and vertex tables.
pub(crate) fn sew_raw(
    raw: RawBrep,
    tolerance: ModelTolerance,
    options: ImportOptions,
    locator: &mut Locator<'_>,
) -> Result<(RawBrep, RepairReport, ModelTolerance), Error> {
    let mut report = RepairReport::default();
    if !options.repair.sew {
        return Ok((raw, report, tolerance));
    }
    let sewing = options.repair.checked_sewing_tolerance(tolerance)?;
    report.sewing_tolerance = sewing;
    let plan = plan_sewing(
        &raw,
        tolerance.distance().as_metres(),
        sewing,
        ValidationLimits {
            max_records: options.max_records,
            max_work: options.max_work,
        },
    )
    .map_err(|e| locator.repair(e, options.max_work))?;
    let working = record(&mut report, &plan, tolerance)?;
    let (vertices, edges) = plan.compaction();
    let mut compact = vec![None; plan.edges.len() - plan.merged_edges];
    for (old, new) in edges.into_iter().enumerate() {
        if let (Some(new), Some(&entity)) = (new, locator.edges.get(old)) {
            compact[new] = entity;
        }
    }
    if !plan.is_empty() {
        locator.edges = compact;
        if locator.vertices.len() == vertices.len() {
            locator.vertices = std::mem::take(&mut locator.vertices)
                .into_iter()
                .zip(vertices)
                .filter_map(|(entity, kept)| kept.map(|_| entity))
                .collect();
        } else {
            locator.vertices.clear();
        }
    }
    Ok((plan.apply(raw), report, working))
}

/// Count a sewing plan and return the working tolerance it needs.
pub(crate) fn record(
    report: &mut RepairReport,
    plan: &SewPlan,
    tolerance: ModelTolerance,
) -> Result<ModelTolerance, Error> {
    report.merged_vertices += plan.merged_vertices;
    report.merged_edges += plan.merged_edges;
    report.max_merge_distance = report.max_merge_distance.max(plan.max_distance);
    report.disconnected_shells += plan.disconnected_shells;
    if plan.max_distance > 0. {
        let distance = tolerance.distance().as_metres() + plan.max_distance;
        ModelTolerance::new(
            Length::metres(distance).map_err(|e| invalid(e.to_string()))?,
            tolerance.angle(),
        )
        .map_err(|e| invalid(e.to_string()))
    } else {
        Ok(tolerance)
    }
}
fn invalid(message: String) -> Error {
    Error {
        kind: ErrorKind::InvalidOptions,
        stage: Stage::Topology,
        entity: None,
        source: None,
        message,
    }
}

/// Validate a raw B-rep: after the opt-in face orientation repair, and followed by the
/// loop direction repair and the outward orientation of closed solid shells. Without
/// repair this is plain validation with the importers' error mapping. A sewn B-rep that
/// still has an open, unsewn or broken boundary fails with the distance its sewing would
/// need; a non-orientable shell is reported only when validation finds no earlier defect.
pub(crate) fn finish(
    mut raw: RawBrep,
    tolerance: ModelTolerance,
    options: ImportOptions,
    mut report: RepairReport,
    locator: &Locator<'_>,
) -> Result<(NormalizedBrep, RepairReport), Error> {
    let limits = ValidationLimits {
        max_records: options.max_records,
        max_work: options.max_work,
    };
    if !options.repair.is_enabled() {
        let brep = raw
            .validate(tolerance, limits)
            .map_err(|e| locator.report(e))?;
        return Ok((brep.normalize(), report));
    }
    let mut unorientable = None;
    let input: Vec<bool> = raw
        .faces
        .iter()
        .map(|f| f.orientation.is_reversed())
        .collect();
    if options.repair.orient {
        match orient_faces(&mut raw, limits) {
            Ok(_) => {}
            Err(e) if e.report.defect == Defect::ResourceLimit => {
                return Err(locator.repair(e, options.max_work));
            }
            Err(e) => unorientable = Some(e),
        }
    }
    let kept = options.repair.sew.then(|| raw.clone());
    let brep = match raw.validate(tolerance, limits) {
        Ok(brep) => brep.normalize(),
        Err(e) => {
            if let Some(o) = unorientable.filter(|_| e.defect == Defect::InconsistentOrientation) {
                return Err(locator.repair(o, options.max_work));
            }
            if let Some(raw) = kept {
                if let Some(gap) = diagnose_gap(
                    &raw,
                    &e,
                    tolerance.distance().as_metres(),
                    report.sewing_tolerance,
                    limits,
                )
                .map_err(|e| locator.repair(e, options.max_work))?
                {
                    return Err(locator.repair(gap, options.max_work));
                }
            }
            return Err(locator.report(e));
        }
    };
    if !options.repair.orient {
        return Ok((brep, report));
    }
    let (brep, reversed) = reverse_loops(brep, tolerance, limits).map_err(|e| locator.report(e))?;
    let (brep, shells) =
        outward(brep, tolerance, limits, &mut report).map_err(|e| locator.report(e))?;
    // Net edits per face: a face whose loops were reversed counts once, whatever its
    // orientation did on the way; a face in a reversed shell counts with the shell.
    let data = brep.data();
    let mut in_shell = vec![false; data.faces.len()];
    for s in &shells {
        for f in &data.shells[s.0].faces {
            in_shell[f.0] = true;
        }
    }
    for (i, face) in data.faces.iter().enumerate() {
        if reversed.contains(&FaceId(i)) {
            report.reversed_loops += 1;
        } else if (face.orientation.is_reversed() != input[i]) != in_shell[i] {
            report.flipped_faces += 1;
        }
    }
    Ok((brep, report))
}

/// Reverse every loop of a face together with its orientation.
fn reverse_face(raw: &mut RawBrep, face: FaceId) {
    let f = &mut raw.faces[face.0];
    f.orientation = if f.orientation.is_reversed() {
        Orientation::Forward
    } else {
        Orientation::Reversed
    };
    for w in std::iter::once(f.outer).chain(f.holes.clone()) {
        raw.wires[w.0].coedges.reverse();
        for &c in &raw.wires[w.0].coedges {
            let coedge = &mut raw.coedges[c.0];
            coedge.orientation = if coedge.orientation.is_reversed() {
                Orientation::Forward
            } else {
                Orientation::Reversed
            };
        }
    }
}
/// Reverse the loops (and orientation) of the faces that the trimmer rejects as wrongly
/// oriented, keeping each reversal only when the trimmer then accepts the face. The
/// trimmer's default options are those tessellation uses.
fn reverse_loops(
    brep: NormalizedBrep,
    tolerance: ModelTolerance,
    limits: ValidationLimits,
) -> Result<(NormalizedBrep, Vec<FaceId>), Report> {
    let options = tessstep_trim::Options::default();
    let wrong = |brep: &NormalizedBrep, face: FaceId| {
        matches!(tessstep_trim::reconstruct(brep, face, options),
            Err(e) if e.kind == tessstep_trim::ErrorKind::WrongOrientation)
    };
    let faces: Vec<FaceId> = (0..brep.data().faces.len())
        .map(FaceId)
        .filter(|&f| wrong(&brep, f))
        .collect();
    if faces.is_empty() {
        return Ok((brep, faces));
    }
    let working = ModelTolerance::new(
        Length::metres(brep.model_tolerance()).map_err(|_| shell_report())?,
        tolerance.angle(),
    )
    .map_err(|_| shell_report())?;
    let reversed = |faces: &[FaceId]| -> Result<NormalizedBrep, Report> {
        let mut raw = brep.data().clone();
        for &f in faces {
            reverse_face(&mut raw, f);
        }
        Ok(raw.validate(working, limits)?.normalize())
    };
    let candidate = reversed(&faces)?;
    let kept: Vec<FaceId> = faces
        .iter()
        .copied()
        .filter(|&f| tessstep_trim::reconstruct(&candidate, f, options).is_ok())
        .collect();
    if kept.len() == faces.len() {
        Ok((candidate, kept))
    } else if kept.is_empty() {
        Ok((brep, kept))
    } else {
        Ok((reversed(&kept)?, kept))
    }
}

/// Reverse closed solid shells whose algebraic volume has the wrong sign: positive
/// for an outer shell, negative for a cavity. The volume is that of a coarse
/// tessellation of the shell (chord 1e-2 of the shell's extent and 0.5 rad, then 1e-3
/// and 0.25 rad), trusted only beyond twice its estimated deviation (see
/// [`volume_sign`]).
fn outward(
    brep: NormalizedBrep,
    tolerance: ModelTolerance,
    limits: ValidationLimits,
    report: &mut RepairReport,
) -> Result<(NormalizedBrep, Vec<ShellId>), Report> {
    let mut reversed = Vec::new();
    for solid in &brep.data().solids {
        for (k, &shell) in std::iter::once(&solid.shell)
            .chain(&solid.voids)
            .enumerate()
        {
            if !brep.data().shells[shell.0].closed {
                continue;
            }
            match volume_sign(&brep, shell) {
                Some(sign) if (k == 0) != (sign > 0.) => reversed.push(shell),
                Some(_) => {}
                None => report.undecided_shells += 1,
            }
        }
    }
    if reversed.is_empty() {
        return Ok((brep, reversed));
    }
    let mut raw = brep.data().clone();
    for shell in &reversed {
        for f in raw.shells[shell.0].faces.clone() {
            let face = &mut raw.faces[f.0];
            face.orientation = if face.orientation.is_reversed() {
                Orientation::Forward
            } else {
                Orientation::Reversed
            };
        }
    }
    report.reoriented_shells += reversed.len();
    let working = ModelTolerance::new(
        Length::metres(brep.model_tolerance()).map_err(|_| shell_report())?,
        tolerance.angle(),
    )
    .map_err(|_| shell_report())?;
    Ok((raw.validate(working, limits)?.normalize(), reversed))
}
fn shell_report() -> Report {
    Report {
        defect: Defect::GeometryFailure,
        kind: "model",
        index: 0,
    }
}
/// The sign of a closed shell's algebraic volume, when a coarse tessellation decides
/// it: the mesh must be one closed component, and its volume must exceed twice the
/// volume between the mesh and the surfaces, estimated per triangle of a curved face as
/// its area times its centroid's distance from the surface point at its mean UV.
fn volume_sign(brep: &NormalizedBrep, shell: ShellId) -> Option<f64> {
    let data = brep.data();
    let faces = &data.shells[shell.0].faces;
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for &f in faces {
        let face = &data.faces[f.0];
        for w in std::iter::once(&face.outer).chain(&face.holes) {
            for &c in &data.wires[w.0].coedges {
                let e = &data.edges[data.coedges[c.0].edge.0];
                for i in 0..=8 {
                    let t = e.range[0] + (e.range[1] - e.range[0]) * i as f64 / 8.;
                    let p = e.curve.evaluate(t).ok()?.position.coordinates();
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
        }
    }
    let extent = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f64>().sqrt();
    if !(extent.is_finite() && extent > 0.) {
        return None;
    }
    for (fraction, angle) in [(1e-2, 0.5), (1e-3, 0.25)] {
        let chord = (extent * fraction).max(4. * brep.model_tolerance());
        let tolerance =
            TessellationTolerance::new(Length::metres(chord).ok()?, Angle::radians(angle).ok()?)
                .ok()?;
        let Ok(mesh) = tessstep_tessellate::tessellate_faces(
            brep,
            faces,
            tolerance,
            TessellationOptions::default(),
        ) else {
            continue;
        };
        if !mesh.is_watertight() || mesh.statistics().components != 1 {
            continue;
        }
        let d = mesh.data();
        let mut error = 0.;
        for ((tri, uv), &face) in d.triangles.iter().zip(&d.uvs).zip(&d.face_ids) {
            let surface = &data.faces[face as usize].surface;
            if matches!(surface, SurfaceGeometry::Analytic(s) if s.kind() == SurfaceKind::Plane) {
                continue;
            }
            let [a, b, c] = tri.map(|i| d.positions[i as usize]);
            let mean = |k: usize| (a[k] + b[k] + c[k]) / 3.;
            let at = [0, 1].map(|k| (uv[0][k] + uv[1][k] + uv[2][k]) / 3.);
            let Ok(p) = surface.evaluate(at) else {
                error = f64::INFINITY;
                break;
            };
            let p = p.position.coordinates();
            let deviation = (0..3).map(|k| (p[k] - mean(k)).powi(2)).sum::<f64>().sqrt();
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            error += deviation * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.;
        }
        let volume = mesh.statistics().signed_volume;
        if volume.is_finite() && volume.abs() > 2. * error {
            return Some(volume.signum());
        }
    }
    None
}
