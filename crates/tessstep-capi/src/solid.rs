//! Single-root solid import through the curved or faceted profile (Milestone 22).
use crate::{
    TS_INVALID_ARGUMENT, TS_OK, TS_ROOT_BREP_WITH_VOIDS, TS_ROOT_FACETED_BREP,
    TS_ROOT_MANIFOLD_SOLID_BREP, TsDocument, TsImportError, TsImportOptions, TsMesh, boundary,
    clear,
    import::report,
    initialize,
    options::{Extensible, Options, finish_output, read_options, reset_output},
};
use std::{ptr, sync::Arc};
use tessstep_import::{Adaptations, ImportSession, ImportedSolid};
use tessstep_math::{AngleUnit, LengthUnit};
use tessstep_model::Document;
use tessstep_part21::EntityId;

/// Extensible like `TsImportOptions`: the caller sets `struct_size`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsSolidInfo {
    pub struct_size: u32,
    pub abi_version: u32,
    pub kind: u32,
    pub reserved: u32,
    pub root_id: u64,
    pub face_count: u64,
    pub shell_count: u64,
    pub inferred_outer_bounds: u64,
    pub inserted_seams: u64,
    pub split_edges: u64,
    pub recharted_spheres: u64,
    pub collapsed_edges: u64,
    pub collapsed_faces: u64,
    pub supplied_pcurves: u64,
    pub rejected_pcurves: u64,
    // Revision 2: the tolerance policy (22.3) and the opt-in repair stage (22.5).
    pub off_surface_edges: u64,
    pub off_curve_vertices: u64,
    pub uncollapsed_sides: u64,
    pub largest_deviation_m: f64,
    pub working_tolerance_m: f64,
    pub merged_vertices: u64,
    pub merged_edges: u64,
    pub max_merge_distance_m: f64,
    pub disconnected_shells: u64,
    pub flipped_faces: u64,
    pub reversed_loops: u64,
    pub reoriented_shells: u64,
    pub undecided_shells: u64,
    pub sewing_tolerance_m: f64,
}
pub const TS_SOLID_INFO_SIZE_1: u32 = 104;
pub const TS_SOLID_INFO_SIZE_2: u32 = 216;
const _: () = assert!(std::mem::offset_of!(TsSolidInfo, off_surface_edges) == 104);
const _: () = assert!(size_of::<TsSolidInfo>() == TS_SOLID_INFO_SIZE_2 as usize);
impl Extensible for TsSolidInfo {
    const REVISIONS: &'static [u32] = &[TS_SOLID_INFO_SIZE_1, TS_SOLID_INFO_SIZE_2];
    fn header(&mut self, struct_size: u32) {
        self.struct_size = struct_size;
        self.abi_version = 1;
    }
}

pub(crate) fn has(e: &tessstep_part21::EntityInstance, name: &str) -> bool {
    e.kind
        .records()
        .iter()
        .any(|r| r.name.as_ref().eq_ignore_ascii_case(name))
}

/// The root kind its physical record names; the profile reports a missing root.
pub(crate) fn solid_kind(document: &Document, id: EntityId) -> u32 {
    match document.entities().get(id) {
        Some(e) if has(e, "FACETED_BREP") => TS_ROOT_FACETED_BREP,
        Some(e) if has(e, "BREP_WITH_VOIDS") => TS_ROOT_BREP_WITH_VOIDS,
        _ => TS_ROOT_MANIFOLD_SOLID_BREP,
    }
}

/// Import and tessellate one solid root, through a session when one is given (whose
/// options must be `o`), else through the free functions.
pub(crate) fn import(
    document: &Document,
    session: Option<&ImportSession<'_>>,
    id: EntityId,
    length: LengthUnit,
    angle: AngleUnit,
    o: &Options,
) -> Result<(ImportedSolid, tessstep_mesh::Mesh, u32), tessstep_import::Error> {
    let kind = solid_kind(document, id);
    let faceted = kind == TS_ROOT_FACETED_BREP;
    let solid = match session {
        None if faceted => {
            tessstep_import::import_faceted_solid(document, id, length, o.model, o.import)
        }
        None => tessstep_import::import_brep_solid(document, id, length, angle, o.model, o.import),
        Some(s) if faceted => s.import_faceted_solid(id, length, o.model),
        Some(s) => s.import_brep_solid(id, length, angle, o.model),
    }?;
    let mesh = match session {
        None => solid.tessellate(o.tolerance, o.tessellation),
        Some(s) => s.tessellate(&solid, o.tolerance, o.tessellation),
    }?;
    Ok((solid, mesh, kind))
}

/// The C record of an imported solid.
pub(crate) fn solid_info(solid: &ImportedSolid, id: EntityId, kind: u32) -> TsSolidInfo {
    let a: Adaptations = solid.adaptations();
    let r = solid.repairs();
    TsSolidInfo {
        struct_size: 0,
        abi_version: 1,
        kind,
        reserved: 0,
        root_id: id.get(),
        face_count: solid.face_entities().len() as u64,
        shell_count: solid.brep().data().shells.len() as u64,
        inferred_outer_bounds: a.inferred_outer_bounds as u64,
        inserted_seams: a.inserted_seams as u64,
        split_edges: a.split_edges as u64,
        recharted_spheres: a.recharted_spheres as u64,
        collapsed_edges: a.collapsed_edges as u64,
        collapsed_faces: a.collapsed_faces as u64,
        supplied_pcurves: a.supplied_pcurves as u64,
        rejected_pcurves: a.rejected_pcurves as u64,
        off_surface_edges: a.off_surface_edges as u64,
        off_curve_vertices: a.off_curve_vertices as u64,
        uncollapsed_sides: a.uncollapsed_sides as u64,
        largest_deviation_m: solid.largest_deviation(),
        working_tolerance_m: solid.brep().model_tolerance(),
        merged_vertices: r.merged_vertices as u64,
        merged_edges: r.merged_edges as u64,
        max_merge_distance_m: r.max_merge_distance,
        disconnected_shells: r.disconnected_shells as u64,
        flipped_faces: r.flipped_faces as u64,
        reversed_loops: r.reversed_loops as u64,
        reoriented_shells: r.reoriented_shells as u64,
        undecided_shells: r.undecided_shells as u64,
        sewing_tolerance_m: r.sewing_tolerance,
    }
}

/// Clear a solid import's outputs before validation: the mesh slot, the optional
/// error and the optional info of a published revision. `Err` is the status of an
/// unusable output (NULL `out`, or an info revision that is left unwritten).
///
/// # Safety
/// tessstep.h output contract; non-NULL `info` spans the revision it names.
pub(crate) unsafe fn begin(
    out: *mut *const TsMesh,
    info: *mut TsSolidInfo,
    error: *mut TsImportError,
) -> Result<Option<usize>, u32> {
    // SAFETY: non-NULL output slots are disjoint writable storage supplied by caller.
    let output_ok = unsafe { initialize(out, ptr::null()) };
    unsafe { clear(error) };
    // SAFETY: the info record spans the revision it names.
    let size = unsafe { reset_output(info) }.map_err(|()| TS_INVALID_ARGUMENT)?;
    if output_ok {
        Ok(size)
    } else {
        Err(TS_INVALID_ARGUMENT)
    }
}

/// Publish a solid import's result or its located error.
///
/// # Safety
/// The outputs `begin` accepted, with its info size.
pub(crate) unsafe fn finish(
    document: &Document,
    id: EntityId,
    result: Result<(ImportedSolid, tessstep_mesh::Mesh, u32), tessstep_import::Error>,
    out: *mut *const TsMesh,
    info: *mut TsSolidInfo,
    size: Option<usize>,
    error: *mut TsImportError,
) -> u32 {
    match result {
        Ok((solid, mesh, kind)) => {
            let record = solid_info(&solid, id, kind);
            let mesh = Arc::into_raw(Arc::new(TsMesh::from(mesh)));
            // SAFETY: cleared output slots; ownership transfers once, after all
            // fallible work, and info spans the revision it named.
            unsafe {
                out.write(mesh);
                finish_output(info, size, record);
            }
            TS_OK
        }
        // SAFETY: the optional error record follows the header contract.
        Err(e) => unsafe { report(document, &e, error) },
    }
}

/// Validated explicit units, or None.
pub(crate) fn units(
    entity_id: u64,
    metres_per_unit: f64,
    radians_per_unit: f64,
) -> Option<(EntityId, LengthUnit, AngleUnit)> {
    Some((
        EntityId::new(entity_id)?,
        LengthUnit::metres_per_unit(metres_per_unit).ok()?,
        AngleUnit::radians_per_unit(radians_per_unit).ok()?,
    ))
}

/// # Safety
/// Follow tessstep.h live-document, options and disjoint output-slot contracts; a
/// non-NULL info addresses at least the revision its struct_size names.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ts_document_import_solid(
    document: *const TsDocument,
    entity_id: u64,
    metres_per_unit: f64,
    radians_per_unit: f64,
    options: *const TsImportOptions,
    out: *mut *const TsMesh,
    info: *mut TsSolidInfo,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: forwarded caller output contract.
    let size = match unsafe { begin(out, info, error) } {
        Ok(size) => size,
        Err(status) => return status,
    };
    boundary(|| {
        if document.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller supplies a live document and, when non-NULL, complete options.
        let document = unsafe { &(*document).document };
        let Some(o) = (unsafe { read_options(options) }).and_then(TsImportOptions::options) else {
            return TS_INVALID_ARGUMENT;
        };
        let Some((id, length, angle)) = units(entity_id, metres_per_unit, radians_per_unit) else {
            return TS_INVALID_ARGUMENT;
        };
        let result = import(document, None, id, length, angle, &o);
        // SAFETY: the outputs begin accepted.
        unsafe { finish(document, id, result, out, info, size, error) }
    })
}
