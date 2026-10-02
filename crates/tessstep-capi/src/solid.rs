//! Single-root solid import through the curved or faceted profile (Milestone 22).
use crate::{
    TS_INVALID_ARGUMENT, TS_OK, TS_ROOT_BREP_WITH_VOIDS, TS_ROOT_FACETED_BREP,
    TS_ROOT_MANIFOLD_SOLID_BREP, TsDocument, TsImportError, TsImportOptions, TsMesh, boundary,
    clear,
    import::report,
    initialize,
    options::{read_options, write_prefix},
};
use std::{ptr, sync::Arc};
use tessstep_math::{AngleUnit, LengthUnit};
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
}
pub const TS_SOLID_INFO_SIZE_1: u32 = 104;
const _: () = assert!(size_of::<TsSolidInfo>() == TS_SOLID_INFO_SIZE_1 as usize);
const SOLID_INFO_REVISIONS: [u32; 1] = [TS_SOLID_INFO_SIZE_1];

fn has(e: &tessstep_part21::EntityInstance, name: &str) -> bool {
    e.kind
        .records()
        .iter()
        .any(|r| r.name.as_ref().eq_ignore_ascii_case(name))
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
    // SAFETY: non-NULL output slots are disjoint writable storage supplied by caller.
    let output_ok = unsafe { initialize(out, ptr::null()) };
    unsafe { clear(error) };
    // The info revision is the caller's; nothing is written for an unknown one.
    let info_size = if info.is_null() {
        None
    } else {
        // SAFETY: struct_size is the first field of every revision.
        let size = unsafe { info.cast::<u32>().read() };
        if !SOLID_INFO_REVISIONS.contains(&size) {
            return TS_INVALID_ARGUMENT;
        }
        let cleared = TsSolidInfo {
            struct_size: size,
            abi_version: 1,
            ..TsSolidInfo::default()
        };
        // SAFETY: the caller's record spans the revision it names.
        unsafe { write_prefix(&cleared, size as usize, info) };
        Some(size as usize)
    };
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
        let (Some(id), Ok(length), Ok(angle)) = (
            EntityId::new(entity_id),
            LengthUnit::metres_per_unit(metres_per_unit),
            AngleUnit::radians_per_unit(radians_per_unit),
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        // The physical record chooses the profile; a missing root is reported by it.
        let record = document.entities().get(id);
        let kind = match record {
            Some(e) if has(e, "FACETED_BREP") => TS_ROOT_FACETED_BREP,
            Some(e) if has(e, "BREP_WITH_VOIDS") => TS_ROOT_BREP_WITH_VOIDS,
            _ => TS_ROOT_MANIFOLD_SOLID_BREP,
        };
        let imported = if kind == TS_ROOT_FACETED_BREP {
            tessstep_import::import_faceted_solid(document, id, length, o.model, o.import)
        } else {
            tessstep_import::import_brep_solid(document, id, length, angle, o.model, o.import)
        };
        let result = imported.and_then(|s| {
            let mesh = s.tessellate(o.tolerance, o.tessellation)?;
            Ok((s, mesh))
        });
        match result {
            Ok((solid, mesh)) => {
                let a = solid.adaptations();
                let record = TsSolidInfo {
                    struct_size: info_size.unwrap_or(0) as u32,
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
                };
                let mesh = Arc::into_raw(Arc::new(TsMesh::from(mesh)));
                // SAFETY: cleared output slots; ownership transfers once, after all
                // fallible work, and info spans the revision it named.
                unsafe {
                    out.write(mesh);
                    if let Some(size) = info_size {
                        write_prefix(&record, size, info);
                    }
                }
                TS_OK
            }
            // SAFETY: the optional error record follows the header contract.
            Err(e) => unsafe { report(document, &e, error) },
        }
    })
}
