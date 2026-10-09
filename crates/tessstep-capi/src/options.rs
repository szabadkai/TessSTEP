//! Extensible import options (Milestone 22). Extensible records name their revision
//! in `struct_size`; ABI 1.0 publishes revision 1 of each, a later 1.x revision only
//! appends fields, and the bridge reads or writes exactly the bytes of the revision a
//! caller names.
use crate::{TS_IMPORT_STRICT, TS_INVALID_ARGUMENT, TS_OK, TsDocument, boundary};
use std::ptr;
use tessstep_import::{
    AssemblyImportOptions, ImportOptions, PresentationLimits, RepairOptions,
    RepresentationPreference, SessionBudgets, TessellationOptions, TessellationTolerance,
};
use tessstep_math::{Angle, Length, ModelTolerance};

/// Opt-in sewing of coincident boundary vertices and free edges (Milestone 22.5).
pub const TS_IMPORT_REPAIR_SEW: u32 = 2;
/// Opt-in face and shell orientation repair (Milestone 22.5).
pub const TS_IMPORT_REPAIR_ORIENT: u32 = 4;
/// Discovery also reports SHELL_BASED_SURFACE_MODEL roots (Milestone 22.6).
pub const TS_DISCOVER_SURFACE_MODELS: u32 = 8;
const IMPORT_FLAGS: u32 =
    TS_IMPORT_STRICT | TS_IMPORT_REPAIR_SEW | TS_IMPORT_REPAIR_ORIENT | TS_DISCOVER_SURFACE_MODELS;

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
    pub sewing_factor: f64,
    pub deviation_factor: f64,
    pub max_assembly_instances: u64,
    pub max_assembly_depth: u64,
    pub max_session_decoded_entities: u64,
    pub max_session_adapted_entities: u64,
    pub max_session_triangles: u64,
    pub max_polyline_points: u64,
    pub max_presentation_depth: u64,
}
/// Published revision sizes, oldest first; the newest is the whole current record.
pub const TS_IMPORT_OPTIONS_SIZE_1: u32 = 168;
const _: () = assert!(size_of::<TsImportOptions>() == TS_IMPORT_OPTIONS_SIZE_1 as usize);
pub(crate) const IMPORT_OPTIONS_REVISIONS: [u32; 1] = [TS_IMPORT_OPTIONS_SIZE_1];

impl Default for TsImportOptions {
    fn default() -> Self {
        let (import, solid) = (ImportOptions::default(), AssemblyImportOptions::default());
        let tessellation = TessellationOptions::default();
        let (session, presentation) = (SessionBudgets::default(), PresentationLimits::default());
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
            sewing_factor: import.repair.sewing_factor,
            deviation_factor: import.deviation_factor,
            max_assembly_instances: solid.assembly.expansion.max_instances as u64,
            max_assembly_depth: solid.assembly.expansion.max_depth as u64,
            max_session_decoded_entities: session.max_decoded_entities as u64,
            max_session_adapted_entities: session.max_adapted_entities as u64,
            max_session_triangles: session.max_triangles as u64,
            max_polyline_points: presentation.max_polyline_points as u64,
            max_presentation_depth: presentation.max_depth as u64,
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
    pub session: SessionBudgets,
    pub presentation: PresentationLimits,
    pub surface_models: bool,
    minimum_model_tolerance: f64,
    max_instances: usize,
    max_depth: usize,
}
impl TsImportOptions {
    /// Every field is checked by every entry point, whether or not it uses it.
    pub(crate) fn options(self) -> Option<Options> {
        if !IMPORT_OPTIONS_REVISIONS.contains(&self.struct_size)
            || self.abi_version != 1
            || self.flags & !IMPORT_FLAGS != 0
            || self.reserved != 0
            || !(self.sewing_factor.is_finite() && self.sewing_factor > 0.)
            || !(self.deviation_factor.is_finite() && self.deviation_factor >= 1.)
        {
            return None;
        }
        let count = |v: u64| usize::try_from(v).ok();
        // Strict rejection of topology defects stays the default; each repair is a
        // separate opt-in. Strict import also forces a deviation factor of 1.
        let import = ImportOptions {
            max_work: count(self.max_work)?,
            max_records: count(self.max_records)?,
            strict: self.flags & TS_IMPORT_STRICT != 0,
            repair: RepairOptions {
                sew: self.flags & TS_IMPORT_REPAIR_SEW != 0,
                orient: self.flags & TS_IMPORT_REPAIR_ORIENT != 0,
                sewing_factor: self.sewing_factor,
            },
            deviation_factor: self.deviation_factor,
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
            session: SessionBudgets {
                max_decoded_entities: count(self.max_session_decoded_entities)?,
                max_adapted_entities: count(self.max_session_adapted_entities)?,
                max_triangles: count(self.max_session_triangles)?,
            },
            surface_models: self.flags & TS_DISCOVER_SURFACE_MODELS != 0,
            presentation: PresentationLimits {
                max_vertices: vertices,
                max_triangles: triangles,
                max_polyline_points: count(self.max_polyline_points)?,
                max_depth: count(self.max_presentation_depth)?,
            },
            minimum_model_tolerance: self.model_distance_m,
            max_instances: count(self.max_assembly_instances)?,
            max_depth: count(self.max_assembly_depth)?,
        })
    }
}
impl Options {
    /// Assembly import: `model_distance_m` floors each root's context uncertainty,
    /// and the instance and depth budgets bound the product-structure expansion; the
    /// scene keeps its Rust limits, as `AssemblyImportOptions::default` does.
    pub(crate) fn assembly(&self, preference: RepresentationPreference) -> AssemblyImportOptions {
        let mut o = AssemblyImportOptions {
            preference,
            tolerance: self.tolerance,
            tessellation: self.tessellation,
            minimum_model_tolerance: self.minimum_model_tolerance,
            mesh: self.tessellation.mesh,
            ..AssemblyImportOptions::default()
        };
        o.assembly.import = self.import;
        o.assembly.expansion.max_work = self.import.max_work;
        o.assembly.expansion.max_instances = self.max_instances;
        o.assembly.expansion.max_depth = self.max_depth;
        o
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

/// An extensible output record: its caller sets `struct_size` to a published
/// revision, and the bridge writes exactly that revision.
pub(crate) trait Extensible: Copy + Default {
    const REVISIONS: &'static [u32];
    /// Set `struct_size` and `abi_version`.
    fn header(&mut self, struct_size: u32);
}
/// Reset a caller's output record before validation: `struct_size` kept,
/// `abi_version` 1, every other field of its revision zero. `Ok(None)` for NULL,
/// `Err(())` for an unknown revision, which is not written.
///
/// # Safety
/// Non-NULL `out` must address at least the revision its first field names.
pub(crate) unsafe fn reset_output<T: Extensible>(out: *mut T) -> Result<Option<usize>, ()> {
    if out.is_null() {
        return Ok(None);
    }
    // SAFETY: struct_size is the first field of every revision.
    let size = unsafe { out.cast::<u32>().read() };
    if !T::REVISIONS.contains(&size) {
        return Err(());
    }
    let mut cleared = T::default();
    cleared.header(size);
    // SAFETY: the caller's record spans the revision it names.
    unsafe { write_prefix(&cleared, size as usize, out) };
    Ok(Some(size as usize))
}
/// Publish `value` as the revision `reset_output` accepted.
///
/// # Safety
/// As `reset_output`, for the same record.
pub(crate) unsafe fn finish_output<T: Extensible>(out: *mut T, size: Option<usize>, mut value: T) {
    if let Some(size) = size {
        value.header(size as u32);
        // SAFETY: the revision accepted by reset_output.
        unsafe { write_prefix(&value, size, out) };
    }
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
        let document = unsafe { &(*document).document };
        let (sized, session) = (
            ImportOptions::for_document(document),
            SessionBudgets::for_document(document),
        );
        let value = TsImportOptions {
            max_work: sized.max_work as u64,
            max_records: sized.max_records as u64,
            max_session_decoded_entities: session.max_decoded_entities as u64,
            max_session_adapted_entities: session.max_adapted_entities as u64,
            max_session_triangles: session.max_triangles as u64,
            ..TsImportOptions::default()
        };
        // SAFETY: forwarded caller storage contract.
        unsafe { publish(out, struct_size, value) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The initialized record maps to exactly the Rust defaults, so a C import with
    /// default options is the Rust default import (TessellationOptions has no
    /// PartialEq, so their Debug forms are compared).
    #[test]
    fn default_options_are_the_rust_defaults() {
        let o = TsImportOptions::default().options().unwrap();
        assert_eq!(
            format!("{:?}", o.tessellation),
            format!("{:?}", TessellationOptions::default())
        );
        let (rust, c) = (ImportOptions::default(), o.import);
        assert_eq!(
            (rust.max_work, rust.max_records, rust.strict, rust.repair),
            (c.max_work, c.max_records, c.strict, c.repair)
        );
        assert_eq!(rust.deviation_factor, c.deviation_factor);
        assert_eq!(o.session, SessionBudgets::default());
        assert_eq!(
            format!("{:?}", o.presentation),
            format!("{:?}", PresentationLimits::default())
        );
        let assembly = AssemblyImportOptions::default();
        let mapped = o.assembly(RepresentationPreference::Exact);
        assert_eq!(
            format!("{:?}", mapped.tessellation),
            format!("{:?}", assembly.tessellation)
        );
        assert_eq!(mapped.tolerance, assembly.tolerance);
        assert_eq!(format!("{:?}", mapped.mesh), format!("{:?}", assembly.mesh));
        assert_eq!(
            mapped.minimum_model_tolerance,
            assembly.minimum_model_tolerance
        );
        assert_eq!(
            format!("{:?}", mapped.assembly),
            format!("{:?}", assembly.assembly)
        );
        // Strict import is what the flag asks for, and repair is never implied.
        let strict = TsImportOptions {
            flags: TS_IMPORT_STRICT,
            ..TsImportOptions::default()
        };
        let o = strict.options().unwrap();
        assert!(o.import.strict && !o.import.repair.is_enabled());
    }
}
