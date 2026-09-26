use crate::{
    TS_INVALID_ARGUMENT, TS_INVALID_GEOMETRY, TS_NOT_FOUND, TS_OK, TS_RESOURCE_LIMIT,
    TS_UNSUPPORTED, TsDocument, TsMesh, boundary, clear, initialize,
};
use std::{ptr, sync::Arc};
use tessstep_import::{ErrorKind, ImportLimits, Stage, TessellationOptions};
use tessstep_math::{Angle, Length, LengthUnit, ModelTolerance, TessellationTolerance};
use tessstep_part21::EntityId;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsFacetedOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub model_distance: f64,
    pub model_angle: f64,
    pub chord: f64,
    pub normal_angle: f64,
    pub uv_tolerance: f64,
    pub max_work: u64,
    pub max_records: u64,
    pub max_vertices: u64,
    pub max_triangles: u64,
}
impl Default for TsFacetedOptions {
    fn default() -> Self {
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            model_distance: 1e-8,
            model_angle: 1e-8,
            chord: 1e-5,
            normal_angle: 0.1,
            uv_tolerance: 1e-8,
            max_work: 5_000_000,
            max_records: 100_000,
            max_vertices: 1_000_000,
            max_triangles: 2_000_000,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsImportError {
    pub stage: u32,
    pub reserved: u32,
    pub entity_id: u64,
    pub start_offset: u64,
    pub end_offset: u64,
}
/// # Safety
/// Non-NULL out must be aligned writable storage for a complete C options record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_faceted_options_init(out: *mut TsFacetedOptions) -> u32 {
    boundary(|| {
        // SAFETY: caller supplies valid writable options storage.
        if unsafe { clear(out) } {
            TS_OK
        } else {
            TS_INVALID_ARGUMENT
        }
    })
}
/// # Safety
/// Follow tessstep.h live-document, options and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_tessellate_faceted(
    document: *const TsDocument,
    entity_id: u64,
    metres_per_unit: f64,
    options: *const TsFacetedOptions,
    out: *mut *const TsMesh,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: this entry point forwards the same documented pointer contract.
    unsafe {
        tessellate_import(
            document,
            entity_id,
            metres_per_unit,
            options,
            out,
            error,
            false,
        )
    }
}

/// Same frozen scalar option layout as the faceted profile; independent entry point.
pub type TsPlanarOptions = TsFacetedOptions;
/// # Safety
/// Non-NULL out must be complete writable options storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_planar_options_init(out: *mut TsPlanarOptions) -> u32 {
    // SAFETY: the alias has the identical options initialization contract.
    unsafe { ts_faceted_options_init(out) }
}
/// # Safety
/// Follow tessstep.h live-document, options and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_tessellate_planar(
    document: *const TsDocument,
    entity_id: u64,
    metres_per_unit: f64,
    options: *const TsPlanarOptions,
    out: *mut *const TsMesh,
    error: *mut TsImportError,
) -> u32 {
    // SAFETY: this entry point forwards the same documented pointer contract.
    unsafe {
        tessellate_import(
            document,
            entity_id,
            metres_per_unit,
            options,
            out,
            error,
            true,
        )
    }
}
unsafe fn tessellate_import(
    document: *const TsDocument,
    entity_id: u64,
    metres_per_unit: f64,
    options: *const TsFacetedOptions,
    out: *mut *const TsMesh,
    error: *mut TsImportError,
    planar: bool,
) -> u32 {
    // SAFETY: non-NULL output slots are disjoint writable storage supplied by caller.
    let output_ok = unsafe { initialize(out, ptr::null()) };
    if !error.is_null() {
        unsafe { clear(error) };
    }
    if !output_ok {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if document.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller supplies a live document and, when non-NULL, complete options.
        let document = unsafe { &(*document).document };
        let o = if options.is_null() {
            TsFacetedOptions::default()
        } else {
            unsafe { *options }
        };
        let Some(id) = EntityId::new(entity_id) else {
            return TS_INVALID_ARGUMENT;
        };
        if o.struct_size != size_of::<TsFacetedOptions>() as u32
            || o.abi_version != 1
            || !o.uv_tolerance.is_finite()
            || o.uv_tolerance <= 0.
        {
            return TS_INVALID_ARGUMENT;
        }
        let (Ok(work), Ok(records), Ok(vertices), Ok(triangles)) = (
            usize::try_from(o.max_work),
            usize::try_from(o.max_records),
            usize::try_from(o.max_vertices),
            usize::try_from(o.max_triangles),
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        let Ok(unit) = LengthUnit::metres_per_unit(metres_per_unit) else {
            return TS_INVALID_ARGUMENT;
        };
        let model = Length::metres(o.model_distance)
            .and_then(|d| Angle::radians(o.model_angle).and_then(|a| ModelTolerance::new(d, a)));
        let tess = Length::metres(o.chord).and_then(|d| {
            Angle::radians(o.normal_angle).and_then(|a| TessellationTolerance::new(d, a))
        });
        let (Ok(model), Ok(tess)) = (model, tess) else {
            return TS_INVALID_ARGUMENT;
        };
        let mut opts = TessellationOptions {
            max_work: work,
            max_evaluations: work,
            ..TessellationOptions::default()
        };
        // Independent stage budgets. Zero means zero throughout; limits also cap
        // the inherited defaults, so raising this field cannot lift hard ceilings.
        opts.sampling.max_evaluations = work;
        opts.sampling.max_samples = opts.sampling.max_samples.min(work);
        opts.planar.max_work = work;
        opts.planar.max_vertices = opts.planar.max_vertices.min(vertices);
        opts.planar.max_triangles = opts.planar.max_triangles.min(triangles);
        opts.planar.trim.max_work = work;
        opts.planar.trim.max_samples = opts.planar.trim.max_samples.min(work);
        opts.planar.trim.uv_tolerance = o.uv_tolerance;
        opts.mesh.max_work = work;
        opts.mesh.max_vertices = vertices;
        opts.mesh.max_triangles = triangles;
        let importer = if planar {
            tessstep_import::import_planar_solid
        } else {
            tessstep_import::import_faceted_solid
        };
        let result = importer(
            document,
            id,
            unit,
            model,
            ImportLimits {
                max_work: work,
                max_records: records,
            },
        )
        .and_then(|s| s.tessellate(tess, opts));
        match result {
            Ok(mesh) => {
                // SAFETY: required output slot was cleared and ownership transfers once.
                unsafe { out.write(Arc::into_raw(Arc::new(TsMesh::from(mesh)))) };
                TS_OK
            }
            Err(e) => {
                if !error.is_null() {
                    let span = e.source.or_else(|| {
                        e.entity
                            .and_then(|id| document.entities().get(id).map(|e| e.source))
                    });
                    let record = TsImportError {
                        stage: match e.stage {
                            Stage::Profile => 1,
                            Stage::Geometry => 2,
                            Stage::Topology => 3,
                            Stage::Tessellation => 4,
                        },
                        reserved: 0,
                        entity_id: e.entity.map_or(0, EntityId::get),
                        start_offset: span.map_or(0, |s| s.start.offset),
                        end_offset: span.map_or(0, |s| s.end.offset),
                    };
                    // SAFETY: optional error record is valid writable storage when non-NULL.
                    unsafe { error.write(record) };
                }
                match e.kind {
                    ErrorKind::InvalidOptions => TS_INVALID_ARGUMENT,
                    ErrorKind::MissingEntity => TS_NOT_FOUND,
                    ErrorKind::Unsupported => TS_UNSUPPORTED,
                    ErrorKind::ResourceLimit => TS_RESOURCE_LIMIT,
                    ErrorKind::InvalidGeometry => TS_INVALID_GEOMETRY,
                }
            }
        }
    })
}
