//! Document import sessions (Milestone 22.8). A session handle owns a document
//! acquisition and the Rust session that borrows it; only owned results leave it.
use crate::{
    TS_INTERNAL_ERROR, TS_INVALID_ARGUMENT, TS_OK, TS_ROOT_FACETED_BREP, TsAssembly, TsDiscovery,
    TsDocument, TsImportError, TsImportOptions, TsMesh, TsSolidInfo, TsTessellatedInfo, boundary,
    clear, discovery, initialize,
    options::{Options, read_options},
    solid::{self, has, units},
    tessellated,
};
use std::{
    ptr,
    sync::{Arc, Mutex, MutexGuard},
};
use tessstep_import::{ImportProfile, ImportSession};
use tessstep_math::LengthUnit;
use tessstep_model::Document;
use tessstep_part21::EntityId;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsSessionStatistics {
    pub discoveries: u64,
    pub shared_decodes: u64,
    pub shared_fallbacks: u64,
    pub shared_roots: u64,
    pub root_decodes: u64,
    pub decoded_entities: u64,
    pub adapted_entities: u64,
    pub imports: u64,
    pub triangles: u64,
}

/// A session and the document it borrows. The session caches through interior
/// mutability (`Send`, not `Sync`), so calls are serialized by the mutex: concurrent
/// C calls on one handle are safe and run one at a time. A panic poisons the mutex,
/// after which every call but release fails with `TS_INTERNAL_ERROR`.
#[derive(Debug)]
pub struct TsImportSession {
    // Declared before `document`, so it is dropped first. Its `'static` borrow of
    // the document is created in `ts_import_session_create` and never leaves this
    // handle: the field is private and every result returned to C is owned.
    session: Mutex<ImportSession<'static>>,
    options: Options,
    document: Arc<TsDocument>,
}
impl TsImportSession {
    fn document(&self) -> &Document {
        &self.document.document
    }
    fn lock(&self) -> Option<MutexGuard<'_, ImportSession<'static>>> {
        self.session.lock().ok()
    }
}

/// # Safety
/// Follow tessstep.h live-document, options and output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_session_create(
    document: *const TsDocument,
    options: *const TsImportOptions,
    out: *mut *mut TsImportSession,
) -> u32 {
    // SAFETY: non-NULL out is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null_mut()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if document.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: non-NULL options is a complete readable record of its revision.
        let Some(o) = (unsafe { read_options(options) }).and_then(TsImportOptions::options) else {
            return TS_INVALID_ARGUMENT;
        };
        // SAFETY: a live acquisition from Arc::into_raw; the session takes its own.
        let document: Arc<TsDocument> = unsafe {
            Arc::increment_strong_count(document);
            Arc::from_raw(document)
        };
        // SAFETY: the Arc keeps the document alive at a fixed heap address until the
        // handle's `document` field drops, after `session`; see TsImportSession.
        let borrowed: &'static Document = unsafe { &*ptr::addr_of!(document.document) };
        let session = ImportSession::with_budgets(borrowed, o.import, o.session);
        let handle = Box::new(TsImportSession {
            session: Mutex::new(session),
            options: o,
            document,
        });
        // SAFETY: cleared output slot; ownership transfers once after all work.
        unsafe { out.write(Box::into_raw(handle)) };
        TS_OK
    })
}

/// # Safety
/// Releases the session created by ts_import_session_create, once; NULL is permitted.
/// No other call on it may be running.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_session_release(session: *mut TsImportSession) {
    if !session.is_null() {
        boundary(|| {
            // SAFETY: the uniquely owned handle from Box::into_raw.
            drop(unsafe { Box::from_raw(session) });
            TS_OK
        });
    }
}

/// # Safety
/// Follow tessstep.h live session and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_session_discover(
    session: *mut TsImportSession,
    preference: u32,
    out: *mut *const TsDiscovery,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: non-NULL output slots are disjoint writable storage supplied by caller.
    let output_ok = unsafe { initialize(out, ptr::null()) };
    unsafe { clear(error) };
    if !output_ok || session.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: the caller keeps the session live for the call.
        let handle = unsafe { &*session };
        let Some(preference) = discovery::preference(preference) else {
            return TS_INVALID_ARGUMENT;
        };
        let Some(s) = handle.lock() else {
            return TS_INTERNAL_ERROR;
        };
        let document = handle.document();
        let result = discovery::discover(document, Some(&*s), preference, &handle.options);
        // SAFETY: the outputs cleared above.
        unsafe { discovery::publish(document, result, out, error) }
    })
}

/// The profile a root's import decodes with, from its physical record.
fn profile(document: &Document, id: EntityId) -> Option<ImportProfile> {
    let e = document.entities().get(id)?;
    const TESSELLATED: [&str; 6] = [
        "TESSELLATED_SOLID",
        "TESSELLATED_SHELL",
        "TRIANGULATED_SURFACE_SET",
        "COMPLEX_TRIANGULATED_SURFACE_SET",
        "TESSELLATED_ANNOTATION_OCCURRENCE",
        "TESSELLATED_GEOMETRIC_SET",
    ];
    if has(e, "MANIFOLD_SOLID_BREP") || has(e, "BREP_WITH_VOIDS") || has(e, "FACETED_BREP") {
        Some(if solid::solid_kind(document, id) == TS_ROOT_FACETED_BREP {
            ImportProfile::Faceted
        } else {
            ImportProfile::Brep
        })
    } else if TESSELLATED.iter().any(|name| has(e, name)) {
        Some(ImportProfile::Tessellated)
    } else {
        None
    }
}

/// # Safety
/// Follow tessstep.h live session, readable array and output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_session_prepare(
    session: *mut TsImportSession,
    roots: *const u64,
    root_count: usize,
    shared: *mut usize,
) -> u32 {
    // SAFETY: non-NULL out is valid writable storage.
    if !unsafe { clear(shared) } || session.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if (roots.is_null() && root_count != 0)
            || root_count > isize::MAX as usize / size_of::<u64>()
        {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: the caller supplies root_count readable IDs for the call.
        let roots = if root_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(roots, root_count) }
        };
        let Some(ids) = roots
            .iter()
            .map(|&r| EntityId::new(r))
            .collect::<Option<Vec<_>>>()
        else {
            return TS_INVALID_ARGUMENT;
        };
        // SAFETY: the caller keeps the session live for the call.
        let handle = unsafe { &*session };
        let Some(s) = handle.lock() else {
            return TS_INTERNAL_ERROR;
        };
        let mut groups: std::collections::BTreeMap<ImportProfile, Vec<EntityId>> =
            std::collections::BTreeMap::new();
        for id in ids {
            if let Some(profile) = profile(handle.document(), id) {
                groups.entry(profile).or_default().push(id);
            }
        }
        let count: usize = groups
            .iter()
            .map(|(&profile, ids)| s.prepare(profile, ids))
            .sum();
        // SAFETY: valid writable output, cleared above.
        unsafe { shared.write(count) };
        TS_OK
    })
}

/// # Safety
/// Follow tessstep.h live session and disjoint output-slot contracts; a non-NULL
/// info addresses at least the revision its struct_size names.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ts_import_session_import_solid(
    session: *mut TsImportSession,
    entity_id: u64,
    metres_per_unit: f64,
    radians_per_unit: f64,
    out: *mut *const TsMesh,
    info: *mut TsSolidInfo,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: forwarded caller output contract.
    let size = match unsafe { solid::begin(out, info, error) } {
        Ok(size) => size,
        Err(status) => return status,
    };
    if session.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: the caller keeps the session live for the call.
        let handle = unsafe { &*session };
        let Some((id, length, angle)) = units(entity_id, metres_per_unit, radians_per_unit) else {
            return TS_INVALID_ARGUMENT;
        };
        let Some(s) = handle.lock() else {
            return TS_INTERNAL_ERROR;
        };
        let document = handle.document();
        let result = solid::import(document, Some(&*s), id, length, angle, &handle.options);
        // SAFETY: the outputs begin accepted.
        unsafe { solid::finish(document, id, result, out, info, size, error) }
    })
}

/// # Safety
/// Follow tessstep.h live session and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_session_import_tessellated(
    session: *mut TsImportSession,
    entity_id: u64,
    metres_per_unit: f64,
    out: *mut *const TsMesh,
    info: *mut TsTessellatedInfo,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: non-NULL output slots are disjoint writable storage supplied by caller.
    let output_ok = unsafe { initialize(out, ptr::null()) };
    unsafe {
        clear(info);
        clear(error);
    }
    if !output_ok || session.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: the caller keeps the session live for the call.
        let handle = unsafe { &*session };
        let (Some(id), Ok(unit)) = (
            EntityId::new(entity_id),
            LengthUnit::metres_per_unit(metres_per_unit),
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        let Some(s) = handle.lock() else {
            return TS_INTERNAL_ERROR;
        };
        let result = s.import_tessellated(id, unit, handle.options.tessellation.mesh);
        // SAFETY: the outputs cleared above.
        unsafe { tessellated::publish(handle.document(), result, out, info, error) }
    })
}

/// # Safety
/// Follow tessstep.h live session and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_session_import_assembly(
    session: *mut TsImportSession,
    preference: u32,
    out: *mut *const TsAssembly,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: non-NULL output slots are disjoint writable storage supplied by caller.
    let output_ok = unsafe { initialize(out, ptr::null()) };
    unsafe { clear(error) };
    if !output_ok || session.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: the caller keeps the session live for the call.
        let handle = unsafe { &*session };
        let Some(preference) = discovery::preference(preference) else {
            return TS_INVALID_ARGUMENT;
        };
        let Some(s) = handle.lock() else {
            return TS_INTERNAL_ERROR;
        };
        let document = handle.document();
        let result = crate::assembly::import(document, &s, handle.options.assembly(preference));
        // SAFETY: the outputs cleared above.
        unsafe { crate::assembly::publish(document, result, out, error) }
    })
}

/// # Safety
/// Follow tessstep.h live session and writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_session_get_statistics(
    session: *const TsImportSession,
    out: *mut TsSessionStatistics,
) -> u32 {
    // SAFETY: non-NULL out is a valid writable C record.
    if !unsafe { clear(out) } || session.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: the caller keeps the session live for the call.
        let Some(s) = (unsafe { &*session }).lock() else {
            return TS_INTERNAL_ERROR;
        };
        let t = s.statistics();
        let record = TsSessionStatistics {
            discoveries: t.discoveries as u64,
            shared_decodes: t.shared_decodes as u64,
            shared_fallbacks: t.shared_fallbacks as u64,
            shared_roots: t.shared_roots as u64,
            root_decodes: t.root_decodes as u64,
            decoded_entities: t.decoded_entities as u64,
            adapted_entities: t.adapted_entities as u64,
            imports: t.imports as u64,
            triangles: t.triangles as u64,
        };
        // SAFETY: initialized writable output.
        unsafe { out.write(record) };
        TS_OK
    })
}
