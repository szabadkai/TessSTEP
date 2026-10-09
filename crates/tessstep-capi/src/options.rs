//! Extensible import options (Milestone 22). The only ABI 1 records that grow:
//! `struct_size` names the caller's revision, later revisions only append fields,
//! and the bridge reads or writes exactly that many bytes of a published revision.
use crate::{TS_IMPORT_STRICT, TS_INVALID_ARGUMENT, TS_OK, TsDocument, boundary};
use std::ptr;
use tessstep_import::{
    AssemblyImportOptions, ImportOptions, TessellationOptions, TessellationTolerance,
};
use tessstep_math::{Angle, Length, ModelTolerance};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsImportOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub flags: u32,
    pub reserved: u32,
    pub max_work: u64,
    pub max_records: u64,
    pub model_distance_m: f64,
    pub model_angle_rad: f64,
    pub chord_m: f64,
    pub normal_angle_rad: f64,
    pub max_tessellation_work: u64,
    pub max_evaluations: u64,
    pub max_vertices: u64,
    pub max_triangles: u64,
}
/// Published revision sizes, oldest first; revision 1 is the whole current record.
pub const TS_IMPORT_OPTIONS_SIZE_1: u32 = 96;
const _: () = assert!(size_of::<TsImportOptions>() == TS_IMPORT_OPTIONS_SIZE_1 as usize);
const IMPORT_OPTIONS_REVISIONS: [u32; 1] = [TS_IMPORT_OPTIONS_SIZE_1];

impl Default for TsImportOptions {
    fn default() -> Self {
        let (import, solid) = (ImportOptions::default(), AssemblyImportOptions::default());
        let tessellation = TessellationOptions::default();
        Self {
            struct_size: TS_IMPORT_OPTIONS_SIZE_1,
            abi_version: 1,
            flags: 0,
            reserved: 0,
            max_work: import.max_work as u64,
            max_records: import.max_records as u64,
            model_distance_m: solid.minimum_model_tolerance,
            model_angle_rad: 1e-8,
            chord_m: solid.tolerance.chord().as_metres(),
            normal_angle_rad: solid.tolerance.normal_angle().as_radians(),
            max_tessellation_work: tessellation.max_work as u64,
            max_evaluations: tessellation.max_evaluations as u64,
            max_vertices: tessellation.mesh.max_vertices as u64,
            max_triangles: tessellation.mesh.max_triangles as u64,
        }
    }
}

/// Validated Rust options of one C record; never exposed across the boundary.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub import: ImportOptions,
    pub model: ModelTolerance,
    pub tolerance: TessellationTolerance,
    pub tessellation: TessellationOptions,
}
impl TsImportOptions {
    /// Every field is checked by every entry point, whether or not it uses it.
    pub(crate) fn options(self) -> Option<Options> {
        if !IMPORT_OPTIONS_REVISIONS.contains(&self.struct_size)
            || self.abi_version != 1
            || self.flags & !TS_IMPORT_STRICT != 0
            || self.reserved != 0
        {
            return None;
        }
        let count = |v: u64| usize::try_from(v).ok();
        let import = ImportOptions {
            max_work: count(self.max_work)?,
            max_records: count(self.max_records)?,
            strict: self.flags & TS_IMPORT_STRICT != 0,
            // The opt-in repair stage is not exposed in C yet: C imports stay strict.
            ..ImportOptions::default()
        };
        let model = ModelTolerance::new(
            Length::metres(self.model_distance_m).ok()?,
            Angle::radians(self.model_angle_rad).ok()?,
        )
        .ok()?;
        let tolerance = TessellationTolerance::new(
            Length::metres(self.chord_m).ok()?,
            Angle::radians(self.normal_angle_rad).ok()?,
        )
        .ok()?;
        let (work, evaluations) = (
            count(self.max_tessellation_work)?,
            count(self.max_evaluations)?,
        );
        let (vertices, triangles) = (count(self.max_vertices)?, count(self.max_triangles)?);
        // The shared solid budgets and mesh limits are explicit; per-face polygon,
        // trimming and sampling budgets keep their defaults, capped by them, so
        // lowering a field always binds and zero means zero.
        let mut t = TessellationOptions {
            max_work: work,
            max_evaluations: evaluations,
            ..TessellationOptions::default()
        };
        t.sampling.max_evaluations = t.sampling.max_evaluations.min(evaluations);
        t.sampling.max_samples = t.sampling.max_samples.min(evaluations);
        t.planar.max_work = t.planar.max_work.min(work);
        t.planar.max_vertices = t.planar.max_vertices.min(vertices);
        t.planar.max_triangles = t.planar.max_triangles.min(triangles);
        t.planar.trim.max_work = t.planar.trim.max_work.min(work);
        t.planar.trim.max_samples = t.planar.trim.max_samples.min(evaluations);
        t.mesh = tessstep_mesh::Limits {
            max_vertices: vertices,
            max_triangles: triangles,
            max_work: t.mesh.max_work.min(work),
        };
        Some(Options {
            import,
            model,
            tolerance,
            tessellation: t,
        })
    }
}

/// Read a caller's options record of any published revision; fields beyond it keep
/// their defaults. NULL selects the defaults; an unknown size is None.
///
/// # Safety
/// Non-NULL `options` must address at least `struct_size` readable bytes, aligned
/// for the record, of which the first field is `struct_size`.
pub(crate) unsafe fn read_options(options: *const TsImportOptions) -> Option<TsImportOptions> {
    let mut value = TsImportOptions::default();
    if options.is_null() {
        return Some(value);
    }
    // SAFETY: struct_size is the first field of every revision.
    let size = unsafe { options.cast::<u32>().read() };
    if !IMPORT_OPTIONS_REVISIONS.contains(&size) {
        return None;
    }
    // SAFETY: the caller's record spans size bytes; the default spans more.
    unsafe { read_prefix(options, size as usize, &mut value) };
    Some(value)
}

/// # Safety
/// `from` must address `size <= size_of::<T>()` readable bytes of a plain C record.
pub(crate) unsafe fn read_prefix<T: Copy>(from: *const T, size: usize, into: &mut T) {
    debug_assert!(size <= size_of::<T>());
    // SAFETY: both ranges span size bytes and do not overlap (into is local).
    unsafe { ptr::copy_nonoverlapping(from.cast::<u8>(), ptr::from_mut(into).cast(), size) };
}
/// # Safety
/// `to` must address `size <= size_of::<T>()` writable bytes of a plain C record.
pub(crate) unsafe fn write_prefix<T: Copy>(value: &T, size: usize, to: *mut T) {
    debug_assert!(size <= size_of::<T>());
    // SAFETY: both ranges span size bytes; the caller's storage is disjoint.
    unsafe { ptr::copy_nonoverlapping(ptr::from_ref(value).cast::<u8>(), to.cast(), size) };
}

/// The published revision a caller's `sizeof` names.
fn revision(struct_size: usize) -> Option<u32> {
    IMPORT_OPTIONS_REVISIONS
        .iter()
        .copied()
        .find(|&s| s as usize == struct_size)
}

/// Write `value` as the revision `struct_size` names, or nothing for an unknown size.
///
/// # Safety
/// Non-NULL `out` must address `struct_size` writable, aligned bytes.
unsafe fn publish(
    out: *mut TsImportOptions,
    struct_size: usize,
    mut value: TsImportOptions,
) -> u32 {
    let Some(size) = revision(struct_size) else {
        return TS_INVALID_ARGUMENT;
    };
    if out.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    value.struct_size = size;
    // SAFETY: a published revision size the caller's storage spans.
    unsafe { write_prefix(&value, struct_size, out) };
    TS_OK
}

/// # Safety
/// Non-NULL out must address `struct_size` writable, aligned bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_options_init(
    out: *mut TsImportOptions,
    struct_size: usize,
) -> u32 {
    // SAFETY: forwarded caller storage contract.
    boundary(|| unsafe { publish(out, struct_size, TsImportOptions::default()) })
}

/// # Safety
/// Non-NULL out must address `struct_size` writable, aligned bytes; non-NULL
/// document must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_import_options_for_document(
    document: *const TsDocument,
    out: *mut TsImportOptions,
    struct_size: usize,
) -> u32 {
    boundary(|| {
        if document.is_null() {
            if revision(struct_size).is_some() && !out.is_null() {
                // SAFETY: every field is an integer or float, so all-zero is valid;
                // its zero struct_size cannot be passed on as valid options.
                let zero: TsImportOptions = unsafe { std::mem::zeroed() };
                // SAFETY: a published revision size the caller's storage spans.
                unsafe { write_prefix(&zero, struct_size, out) };
            }
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps the document live for the call.
        let sized = ImportOptions::for_document(unsafe { &(*document).document });
        let value = TsImportOptions {
            max_work: sized.max_work as u64,
            max_records: sized.max_records as u64,
            ..TsImportOptions::default()
        };
        // SAFETY: forwarded caller storage contract.
        unsafe { publish(out, struct_size, value) }
    })
}
