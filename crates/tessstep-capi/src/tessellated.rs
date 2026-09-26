//! Existing tessellation and presentation-graphics import (Milestone 20).
use crate::{
    TS_IMPORT_STRICT, TS_INVALID_ARGUMENT, TS_NOT_FOUND, TS_OK, TsDocument, TsImportError, TsMesh,
    boundary, clear, import::report, initialize,
};
use std::{ptr, sync::Arc};
use tessstep_import::{ImportOptions, PresentationItemKind, PresentationLimits, TessellatedKind};
use tessstep_math::LengthUnit;
use tessstep_part21::EntityId;

pub const TS_TESSELLATED_SOLID: u32 = 1;
pub const TS_TESSELLATED_SHELL: u32 = 2;
pub const TS_TESSELLATED_SURFACE_SET: u32 = 3;
pub const TS_PRESENTATION_CURVE_SET: u32 = 1;
pub const TS_PRESENTATION_POINT_SET: u32 = 2;
pub const TS_PRESENTATION_SURFACE_SET: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsTessellatedOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub max_work: u64,
    pub max_records: u64,
    pub max_vertices: u64,
    pub max_triangles: u64,
    pub flags: u32,
    pub reserved: u32,
}
impl Default for TsTessellatedOptions {
    fn default() -> Self {
        let (import, mesh) = (ImportOptions::default(), tessstep_mesh::Limits::default());
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            max_work: import.max_work as u64,
            max_records: import.max_records as u64,
            max_vertices: mesh.max_vertices as u64,
            max_triangles: mesh.max_triangles as u64,
            flags: 0,
            reserved: 0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsTessellatedInfo {
    pub kind: u32,
    pub reserved: u32,
    pub root_id: u64,
    pub link_id: u64,
    pub face_count: u64,
    pub linked_face_count: u64,
    pub edge_count: u64,
    pub vertex_count: u64,
    pub skipped_degenerate: u64,
    pub joined_points: u64,
    pub pnmax_deviations: u64,
}

/// Validated, private bridge limits; never exposed as a C record.
fn budgets(
    struct_size: u32,
    expected: usize,
    abi_version: u32,
    flags: u32,
    reserved: u32,
    counts: &[u64],
) -> Option<(bool, Vec<usize>)> {
    if struct_size != expected as u32
        || abi_version != 1
        || flags & !TS_IMPORT_STRICT != 0
        || reserved != 0
    {
        return None;
    }
    let counts = counts
        .iter()
        .map(|&c| usize::try_from(c).ok())
        .collect::<Option<Vec<_>>>()?;
    Some((flags & TS_IMPORT_STRICT != 0, counts))
}

/// # Safety
/// Non-NULL out must be aligned writable storage for a complete C options record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_tessellated_options_init(out: *mut TsTessellatedOptions) -> u32 {
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
pub unsafe extern "C" fn ts_document_import_tessellated(
    document: *const TsDocument,
    entity_id: u64,
    metres_per_unit: f64,
    options: *const TsTessellatedOptions,
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
            TsTessellatedOptions::default()
        } else {
            unsafe { *options }
        };
        let (Some(id), Ok(unit)) = (
            EntityId::new(entity_id),
            LengthUnit::metres_per_unit(metres_per_unit),
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        let Some((strict, counts)) = budgets(
            o.struct_size,
            size_of::<TsTessellatedOptions>(),
            o.abi_version,
            o.flags,
            o.reserved,
            &[o.max_work, o.max_records, o.max_vertices, o.max_triangles],
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        let [work, records, vertices, triangles] = counts[..] else {
            unreachable!("four budgets")
        };
        let result = tessstep_import::import_tessellated(
            document,
            id,
            unit,
            ImportOptions {
                max_work: work,
                max_records: records,
                strict,
            },
            tessstep_mesh::Limits {
                max_vertices: vertices,
                max_triangles: triangles,
                max_work: work,
            },
        );
        match result {
            Ok(imported) => {
                let record = TsTessellatedInfo {
                    kind: match imported.kind() {
                        TessellatedKind::Solid => TS_TESSELLATED_SOLID,
                        TessellatedKind::Shell => TS_TESSELLATED_SHELL,
                        TessellatedKind::SurfaceSet => TS_TESSELLATED_SURFACE_SET,
                    },
                    reserved: 0,
                    root_id: imported.root().get(),
                    link_id: imported.link().map_or(0, EntityId::get),
                    face_count: imported.faces().len() as u64,
                    linked_face_count: imported
                        .faces()
                        .iter()
                        .filter(|f| f.geometric_link.is_some())
                        .count() as u64,
                    edge_count: imported.edges().len() as u64,
                    vertex_count: imported.vertices().len() as u64,
                    skipped_degenerate: imported.skipped_degenerate() as u64,
                    joined_points: imported.joined_points() as u64,
                    pnmax_deviations: imported.pnmax_deviations() as u64,
                };
                let mesh = Arc::into_raw(Arc::new(TsMesh::from(imported.into_mesh())));
                // SAFETY: cleared output slots; ownership transfers once, after all
                // fallible work, and the optional info record is writable when non-NULL.
                unsafe {
                    out.write(mesh);
                    if !info.is_null() {
                        info.write(record);
                    }
                }
                TS_OK
            }
            // SAFETY: the optional error record follows the header contract.
            Err(e) => unsafe { report(document, &e, error) },
        }
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsPresentationOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub max_work: u64,
    pub max_records: u64,
    pub max_vertices: u64,
    pub max_triangles: u64,
    pub max_polyline_points: u64,
    pub max_depth: u64,
    pub flags: u32,
    pub reserved: u32,
}
impl Default for TsPresentationOptions {
    fn default() -> Self {
        let (import, limits) = (ImportOptions::default(), PresentationLimits::default());
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            max_work: import.max_work as u64,
            max_records: import.max_records as u64,
            max_vertices: limits.max_vertices as u64,
            max_triangles: limits.max_triangles as u64,
            max_polyline_points: limits.max_polyline_points as u64,
            max_depth: limits.max_depth as u64,
            flags: 0,
            reserved: 0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsPresentationInfo {
    pub root_id: u64,
    pub occurrence_id: u64,
    pub geometric_set_id: u64,
    pub style_count: u64,
    pub item_count: u64,
    pub skipped_degenerate: u64,
    pub zero_area_triangles: u64,
    pub pnmax_deviations: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsPresentationItem {
    pub entity_id: u64,
    pub kind: u32,
    pub supplied_normals: u32,
    pub placements: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsPresentationView {
    pub positions: *const f64,
    pub vertex_count: usize,
    pub polyline_points: *const u32,
    pub polyline_point_count: usize,
    pub polyline_offsets: *const u64,
    pub polyline_items: *const u64,
    pub polyline_count: usize,
    pub triangles: *const u32,
    pub triangle_items: *const u64,
    pub triangle_count: usize,
    pub points: *const u32,
    pub point_items: *const u64,
    pub point_count: usize,
}
impl Default for TsPresentationView {
    fn default() -> Self {
        Self {
            positions: ptr::null(),
            vertex_count: 0,
            polyline_points: ptr::null(),
            polyline_point_count: 0,
            polyline_offsets: ptr::null(),
            polyline_items: ptr::null(),
            polyline_count: 0,
            triangles: ptr::null(),
            triangle_items: ptr::null(),
            triangle_count: 0,
            points: ptr::null(),
            point_items: ptr::null(),
            point_count: 0,
        }
    }
}
/// Private scalar storage implementing the C buffer protocol; no kernel layouts exposed.
#[derive(Debug)]
pub struct TsPresentation {
    positions: Vec<f64>,
    polyline_points: Vec<u32>,
    polyline_offsets: Vec<u64>,
    polyline_items: Vec<u64>,
    triangles: Vec<u32>,
    triangle_items: Vec<u64>,
    points: Vec<u32>,
    point_items: Vec<u64>,
    styles: Vec<u64>,
    items: Vec<TsPresentationItem>,
    info: TsPresentationInfo,
}
impl From<tessstep_import::ImportedPresentation> for TsPresentation {
    fn from(p: tessstep_import::ImportedPresentation) -> Self {
        Self {
            positions: p.positions().iter().flatten().copied().collect(),
            polyline_points: p.polyline_points().to_vec(),
            polyline_offsets: p.polyline_offsets().to_vec(),
            polyline_items: p.polyline_items().to_vec(),
            triangles: p.triangles().iter().flatten().copied().collect(),
            triangle_items: p.triangle_items().to_vec(),
            points: p.points().to_vec(),
            point_items: p.point_items().to_vec(),
            styles: p.styles().iter().map(|s| s.get()).collect(),
            items: p
                .items()
                .iter()
                .map(|i| TsPresentationItem {
                    entity_id: i.entity.get(),
                    kind: match i.kind {
                        PresentationItemKind::CurveSet => TS_PRESENTATION_CURVE_SET,
                        PresentationItemKind::PointSet => TS_PRESENTATION_POINT_SET,
                        PresentationItemKind::SurfaceSet => TS_PRESENTATION_SURFACE_SET,
                    },
                    supplied_normals: u32::from(i.supplied_normals),
                    placements: i.placements as u64,
                })
                .collect(),
            info: TsPresentationInfo {
                root_id: p.root().get(),
                occurrence_id: p.occurrence().map_or(0, EntityId::get),
                geometric_set_id: p.geometric_set().get(),
                style_count: p.styles().len() as u64,
                item_count: p.items().len() as u64,
                skipped_degenerate: p.skipped_degenerate() as u64,
                zero_area_triangles: p.zero_area_triangles() as u64,
                pnmax_deviations: p.pnmax_deviations() as u64,
            },
        }
    }
}
/// NULL for empty arrays, so C callers never receive dangling non-NULL pointers.
fn array<T>(values: &[T]) -> *const T {
    if values.is_empty() {
        ptr::null()
    } else {
        values.as_ptr()
    }
}

/// # Safety
/// Non-NULL out must be aligned writable storage for a complete C options record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_presentation_options_init(out: *mut TsPresentationOptions) -> u32 {
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
pub unsafe extern "C" fn ts_document_import_presentation(
    document: *const TsDocument,
    entity_id: u64,
    metres_per_unit: f64,
    options: *const TsPresentationOptions,
    out: *mut *const TsPresentation,
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
        let o = if options.is_null() {
            TsPresentationOptions::default()
        } else {
            unsafe { *options }
        };
        let (Some(id), Ok(unit)) = (
            EntityId::new(entity_id),
            LengthUnit::metres_per_unit(metres_per_unit),
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        let Some((strict, counts)) = budgets(
            o.struct_size,
            size_of::<TsPresentationOptions>(),
            o.abi_version,
            o.flags,
            o.reserved,
            &[
                o.max_work,
                o.max_records,
                o.max_vertices,
                o.max_triangles,
                o.max_polyline_points,
                o.max_depth,
            ],
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        let [work, records, vertices, triangles, polyline_points, depth] = counts[..] else {
            unreachable!("six budgets")
        };
        let result = tessstep_import::import_presentation(
            document,
            id,
            unit,
            ImportOptions {
                max_work: work,
                max_records: records,
                strict,
            },
            PresentationLimits {
                max_vertices: vertices,
                max_triangles: triangles,
                max_polyline_points: polyline_points,
                max_depth: depth,
            },
        );
        match result {
            Ok(p) => {
                let handle = Arc::into_raw(Arc::new(TsPresentation::from(p)));
                // SAFETY: cleared output slot; ownership transfers once after all work.
                unsafe { out.write(handle) };
                TS_OK
            }
            // SAFETY: the optional error record follows the header contract.
            Err(e) => unsafe { report(document, &e, error) },
        }
    })
}

/// # Safety
/// The non-NULL presentation must be live for the entire call; each retain needs release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_presentation_retain(presentation: *const TsPresentation) -> u32 {
    boundary(|| {
        if presentation.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: returned by this library with a positive strong count.
        unsafe { Arc::increment_strong_count(presentation) };
        TS_OK
    })
}
/// # Safety
/// Releases exactly one acquisition; NULL is permitted. Follow tessstep.h lifetimes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_presentation_release(presentation: *const TsPresentation) {
    if !presentation.is_null() {
        // SAFETY: caller transfers one live strong reference from import/retain.
        drop(unsafe { Arc::from_raw(presentation) });
    }
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_presentation_get_info(
    presentation: *const TsPresentation,
    out: *mut TsPresentationInfo,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a valid writable C record.
        if !unsafe { clear(out) } || presentation.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps a live acquisition throughout this query.
        let p = unsafe { &*presentation };
        unsafe { out.write(p.info) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_presentation_get_view(
    presentation: *const TsPresentation,
    out: *mut TsPresentationView,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a valid writable C view record.
        if !unsafe { clear(out) } || presentation.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps a live acquisition throughout this query and view use.
        let p = unsafe { &*presentation };
        let view = TsPresentationView {
            positions: array(&p.positions),
            vertex_count: p.positions.len() / 3,
            polyline_points: array(&p.polyline_points),
            polyline_point_count: p.polyline_points.len(),
            polyline_offsets: array(&p.polyline_offsets),
            polyline_items: array(&p.polyline_items),
            polyline_count: p.polyline_items.len(),
            triangles: array(&p.triangles),
            triangle_items: array(&p.triangle_items),
            triangle_count: p.triangle_items.len(),
            points: array(&p.points),
            point_items: array(&p.point_items),
            point_count: p.points.len(),
        };
        // SAFETY: initialized writable output; scalar/pointer C fields only.
        unsafe { out.write(view) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_presentation_style_at(
    presentation: *const TsPresentation,
    index: usize,
    out: *mut u64,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is valid writable storage.
        if !unsafe { clear(out) } || presentation.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps a live acquisition throughout this query.
        let p = unsafe { &*presentation };
        match p.styles.get(index) {
            Some(&style) => {
                unsafe { out.write(style) };
                TS_OK
            }
            None => TS_NOT_FOUND,
        }
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_presentation_item_at(
    presentation: *const TsPresentation,
    index: usize,
    out: *mut TsPresentationItem,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a valid writable C record.
        if !unsafe { clear(out) } || presentation.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps a live acquisition throughout this query.
        let p = unsafe { &*presentation };
        match p.items.get(index) {
            Some(&item) => {
                unsafe { out.write(item) };
                TS_OK
            }
            None => TS_NOT_FOUND,
        }
    })
}
