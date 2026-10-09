//! Shell-based surface models imported as shell meshes (Milestone 22.6).
use crate::{
    TS_INVALID_ARGUMENT, TS_NOT_FOUND, TS_OK, TsDocument, TsImportError, TsImportOptions, TsMesh,
    boundary, clear,
    import::report,
    initialize,
    options::{Extensible, finish_output, read_options, reset_output},
    solid::units,
    tessellated::array,
};
use std::{ptr, sync::Arc};
use tessstep_import::{ImportOptions, RepairOptions};

pub const TS_ROOT_SHELL_BASED_SURFACE_MODEL: u32 = 7;

/// Extensible like `TsImportOptions`: the caller sets `struct_size`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsSurfaceModelInfo {
    pub struct_size: u32,
    pub abi_version: u32,
    pub root_id: u64,
    pub shell_count: u64,
    pub closed_shell_count: u64,
    pub face_count: u64,
    pub boundary_edges: u64,
    pub inferred_outer_bounds: u64,
    pub inserted_seams: u64,
    pub split_edges: u64,
    pub recharted_spheres: u64,
    pub collapsed_edges: u64,
    pub collapsed_faces: u64,
    pub supplied_pcurves: u64,
    pub rejected_pcurves: u64,
    pub off_surface_edges: u64,
    pub off_curve_vertices: u64,
    pub uncollapsed_sides: u64,
}
pub const TS_SURFACE_MODEL_INFO_SIZE_1: u32 = 136;
const _: () = assert!(size_of::<TsSurfaceModelInfo>() == TS_SURFACE_MODEL_INFO_SIZE_1 as usize);
impl Extensible for TsSurfaceModelInfo {
    const REVISIONS: &'static [u32] = &[TS_SURFACE_MODEL_INFO_SIZE_1];
    fn header(&mut self, struct_size: u32) {
        self.struct_size = struct_size;
        self.abi_version = 1;
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsSurfaceShell {
    pub entity_id: u64,
    pub closed: u32,
    pub reserved: u32,
    pub faces: *const u64,
    pub face_count: usize,
    pub boundary_edges: u64,
    pub working_tolerance_m: f64,
}
impl Default for TsSurfaceShell {
    fn default() -> Self {
        Self {
            entity_id: 0,
            closed: 0,
            reserved: 0,
            faces: ptr::null(),
            face_count: 0,
            boundary_edges: 0,
            working_tolerance_m: 0.,
        }
    }
}

#[derive(Debug)]
struct Shell {
    entity: u64,
    closed: bool,
    faces: Vec<u64>,
    boundary_edges: u64,
    working_tolerance: f64,
    mesh: Arc<TsMesh>,
}
/// Private owned records and shell meshes; no kernel layouts exposed.
#[derive(Debug)]
pub struct TsSurfaceModel {
    info: TsSurfaceModelInfo,
    shells: Vec<Shell>,
}

/// # Safety
/// Follow tessstep.h live-document, options and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_import_surface_model(
    document: *const TsDocument,
    entity_id: u64,
    metres_per_unit: f64,
    radians_per_unit: f64,
    options: *const TsImportOptions,
    out: *mut *const TsSurfaceModel,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: non-NULL output slots are disjoint writable storage supplied by caller.
    let output_ok = unsafe { initialize(out, ptr::null()) };
    unsafe { clear(error) };
    if !output_ok {
        return TS_INVALID_ARGUMENT;
    }
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
        // Surface models are never sewn or reoriented: the repair flags are validated
        // with the record and do not apply here.
        let import = ImportOptions {
            repair: RepairOptions::default(),
            ..o.import
        };
        let result =
            tessstep_import::import_surface_model(document, id, length, angle, o.model, import)
                .and_then(|model| {
                    let meshes = model.tessellate(o.tolerance, o.tessellation)?;
                    Ok((model, meshes))
                });
        let (model, meshes) = match result {
            Ok(imported) => imported,
            // SAFETY: the optional error record follows the header contract.
            Err(e) => return unsafe { report(document, &e, error) },
        };
        let a = model.adaptations();
        let shells: Vec<Shell> = model
            .shells()
            .iter()
            .zip(meshes)
            .map(|(shell, mesh)| Shell {
                entity: shell.entity().get(),
                closed: shell.closed(),
                faces: shell.face_entities().iter().map(|f| f.get()).collect(),
                boundary_edges: shell.boundary_edges() as u64,
                working_tolerance: shell.brep().model_tolerance(),
                mesh: Arc::new(TsMesh::from(mesh)),
            })
            .collect();
        let info = TsSurfaceModelInfo {
            struct_size: 0,
            abi_version: 1,
            root_id: model.root().get(),
            shell_count: shells.len() as u64,
            closed_shell_count: shells.iter().filter(|s| s.closed).count() as u64,
            face_count: shells.iter().map(|s| s.faces.len() as u64).sum(),
            boundary_edges: shells.iter().map(|s| s.boundary_edges).sum(),
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
        };
        let handle = Arc::into_raw(Arc::new(TsSurfaceModel { info, shells }));
        // SAFETY: cleared output slot; ownership transfers once after all work.
        unsafe { out.write(handle) };
        TS_OK
    })
}

/// # Safety
/// The non-NULL model must be live for the entire call; each retain needs release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_surface_model_retain(model: *const TsSurfaceModel) -> u32 {
    boundary(|| {
        if model.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: returned by this library with a positive strong count.
        unsafe { Arc::increment_strong_count(model) };
        TS_OK
    })
}
/// # Safety
/// Releases exactly one acquisition; NULL is permitted. Follow tessstep.h lifetimes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_surface_model_release(model: *const TsSurfaceModel) {
    if !model.is_null() {
        // SAFETY: caller transfers one live strong reference from import/retain.
        drop(unsafe { Arc::from_raw(model) });
    }
}
/// # Safety
/// Follow tessstep.h live handle contract; non-NULL out spans the revision its
/// struct_size names.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_surface_model_get_info(
    model: *const TsSurfaceModel,
    out: *mut TsSurfaceModelInfo,
) -> u32 {
    boundary(|| {
        // SAFETY: the record spans the revision it names.
        let Ok(Some(size)) = (unsafe { reset_output(out) }) else {
            return TS_INVALID_ARGUMENT;
        };
        if model.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps a live acquisition throughout this query.
        let info = unsafe { (*model).info };
        // SAFETY: the revision reset_output accepted.
        unsafe { finish_output(out, Some(size), info) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts; the face
/// array is borrowed from the model.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_surface_model_shell_at(
    model: *const TsSurfaceModel,
    index: usize,
    out: *mut TsSurfaceShell,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a valid writable C record.
        if !unsafe { clear(out) } || model.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps a live acquisition throughout this query and view use.
        let Some(s) = (unsafe { &*model }).shells.get(index) else {
            return TS_NOT_FOUND;
        };
        let shell = TsSurfaceShell {
            entity_id: s.entity,
            closed: u32::from(s.closed),
            reserved: 0,
            faces: array(&s.faces),
            face_count: s.faces.len(),
            boundary_edges: s.boundary_edges,
            working_tolerance_m: s.working_tolerance,
        };
        // SAFETY: initialized writable output.
        unsafe { out.write(shell) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable pointer slot contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_surface_model_shell_mesh(
    model: *const TsSurfaceModel,
    index: usize,
    out: *mut *const TsMesh,
) -> u32 {
    // SAFETY: non-NULL out is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if model.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: the model remains live throughout this query.
        let Some(s) = (unsafe { &*model }).shells.get(index) else {
            return TS_NOT_FOUND;
        };
        // SAFETY: initialized output; transfers an independent mesh acquisition.
        unsafe { out.write(Arc::into_raw(s.mesh.clone())) };
        TS_OK
    })
}
