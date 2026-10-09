//! Schema selection, assembly import and imported appearance (Milestone 21), and
//! assembly import with the extensible import options (Milestone 22).
use crate::{
    TS_IMPORT_STRICT, TS_INVALID_ARGUMENT, TS_NOT_FOUND, TS_OK, TsAppearance, TsDocument,
    TsImportError, TsImportOptions, TsScene, TsStringView, boundary, clear, discovery,
    import::{error_record, report},
    initialize,
    options::read_options,
};
use std::{ptr, sync::Arc};
use tessstep_import::{
    AssemblyImportOptions, ImportOptions, ImportSession, Protocol, ShapeRootKind, SolidKind,
    StyleOptions, TessellatedKind, ToleratedEncodings,
};
use tessstep_math::{Angle, Length, TessellationTolerance};
use tessstep_model::Document;
use tessstep_part21::EntityId;

pub const TS_PROTOCOL_UNKNOWN: u32 = 0;
pub const TS_PROTOCOL_AP203: u32 = 1;
pub const TS_PROTOCOL_AP203E2: u32 = 2;
pub const TS_PROTOCOL_AP214: u32 = 3;
pub const TS_PROTOCOL_AP242: u32 = 4;

pub const TS_ROOT_MANIFOLD_SOLID_BREP: u32 = 1;
pub const TS_ROOT_BREP_WITH_VOIDS: u32 = 2;
pub const TS_ROOT_FACETED_BREP: u32 = 3;
pub const TS_ROOT_TESSELLATED_SOLID: u32 = 4;
pub const TS_ROOT_TESSELLATED_SHELL: u32 = 5;
pub const TS_ROOT_TESSELLATED_SURFACE_SET: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsSchemaInfo {
    pub protocol: u32,
    pub reserved: u32,
    pub schema_count: u64,
    pub declared: TsStringView,
}

/// # Safety
/// Follow tessstep.h live-document and writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_get_schema(
    document: *const TsDocument,
    out: *mut TsSchemaInfo,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a valid writable C record.
        if !unsafe { clear(out) } || document.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps the document live while it uses the borrowed name.
        let document = unsafe { &(*document).document };
        let Some(schema) = tessstep_import::declared_schema(document) else {
            return TS_NOT_FOUND;
        };
        let info = TsSchemaInfo {
            protocol: match schema.protocol {
                Some(Protocol::Ap203) => TS_PROTOCOL_AP203,
                Some(Protocol::Ap203e2) => TS_PROTOCOL_AP203E2,
                Some(Protocol::Ap214) => TS_PROTOCOL_AP214,
                Some(Protocol::Ap242) => TS_PROTOCOL_AP242,
                None => TS_PROTOCOL_UNKNOWN,
            },
            reserved: 0,
            schema_count: schema.count as u64,
            declared: TsStringView::new(schema.declared),
        };
        // SAFETY: initialized writable output; the view borrows document storage.
        unsafe { out.write(info) };
        TS_OK
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsAssemblyOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub max_work: u64,
    pub max_records: u64,
    pub max_instances: u64,
    pub max_depth: u64,
    pub chord_m: f64,
    pub angle_rad: f64,
    pub minimum_model_tolerance_m: f64,
    pub flags: u32,
    pub reserved: u32,
}
impl Default for TsAssemblyOptions {
    fn default() -> Self {
        let o = AssemblyImportOptions::default();
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            max_work: o.assembly.import.max_work as u64,
            max_records: o.assembly.import.max_records as u64,
            max_instances: o.assembly.expansion.max_instances as u64,
            max_depth: o.assembly.expansion.max_depth as u64,
            chord_m: o.tolerance.chord().as_metres(),
            angle_rad: o.tolerance.normal_angle().as_radians(),
            minimum_model_tolerance_m: o.minimum_model_tolerance,
            flags: 0,
            reserved: 0,
        }
    }
}
impl TsAssemblyOptions {
    fn options(self) -> Option<AssemblyImportOptions> {
        if self.struct_size != size_of::<Self>() as u32
            || self.abi_version != 1
            || self.flags & !TS_IMPORT_STRICT != 0
            || self.reserved != 0
        {
            return None;
        }
        let count = |v: u64| usize::try_from(v).ok();
        let mut o = AssemblyImportOptions::default();
        o.assembly.import = ImportOptions {
            max_work: count(self.max_work)?,
            max_records: count(self.max_records)?,
            strict: self.flags & TS_IMPORT_STRICT != 0,
            // The opt-in repair stays off; the curved-import tolerance policy keeps its
            // default, which strict disables.
            ..ImportOptions::default()
        };
        o.assembly.expansion.max_work = o.assembly.import.max_work;
        o.assembly.expansion.max_instances = count(self.max_instances)?;
        o.assembly.expansion.max_depth = count(self.max_depth)?;
        o.assembly.scene.max_instances = o.assembly.expansion.max_instances;
        o.assembly.scene.max_depth = o.assembly.expansion.max_depth;
        o.tolerance = TessellationTolerance::new(
            Length::metres(self.chord_m).ok()?,
            Angle::radians(self.angle_rad).ok()?,
        )
        .ok()?;
        if !self.minimum_model_tolerance_m.is_finite() || self.minimum_model_tolerance_m <= 0. {
            return None;
        }
        o.minimum_model_tolerance = self.minimum_model_tolerance_m;
        Some(o)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsAssemblyInfo {
    pub node_count: u64,
    pub product_count: u64,
    pub occurrence_count: u64,
    pub root_count: u64,
    pub imported_root_count: u64,
    pub unplaced_count: u64,
    pub unimported_count: u64,
    pub excluded_count: u64,
    pub complete: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsAssemblyNode {
    pub instance_id: u64,
    pub definition_id: u64,
    pub occurrence_id: u64,
    pub representation_id: u64,
    pub root_id: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsAssemblyProduct {
    pub definition_id: u64,
    pub product_id: u64,
    pub id: TsStringView,
    pub name: TsStringView,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsAssemblyRoot {
    pub root_id: u64,
    pub kind: u32,
    pub selected: u32,
    pub status: u32,
    pub reserved: u32,
    pub error: TsImportError,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsAssemblyExclusion {
    pub record_id: u64,
    pub structural: u32,
    pub status: u32,
    pub error: TsImportError,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsStyleInfo {
    pub styled_items: u64,
    pub asset_styles: u64,
    pub face_styles: u64,
    pub unimported: u64,
    pub unsupported_targets: u64,
    pub no_surface_colour: u64,
    pub context_styles: u64,
    pub conflicts: u64,
    pub complex_items: u64,
    pub layers: u64,
    pub invisibility: u64,
    pub excluded: u64,
}

/// Private owned records implementing the C queries; no kernel layouts exposed.
#[derive(Debug)]
pub struct TsAssembly {
    scene: Arc<TsScene>,
    appearance: Arc<TsAppearance>,
    info: TsAssemblyInfo,
    nodes: Vec<TsAssemblyNode>,
    products: Vec<(u64, u64, String, String)>,
    roots: Vec<TsAssemblyRoot>,
    exclusions: Vec<TsAssemblyExclusion>,
    unplaced: Vec<u64>,
    style: TsStyleInfo,
    adaptations: TsAssemblyAdaptations,
    /// Per root, in `roots` order: its context's tolerated encodings and its failure
    /// message (empty when imported).
    reports: Vec<(TsToleratedEncodings, String)>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsToleratedEncodings {
    pub explicit_si_dimensions: u64,
    pub unset_si_dimensions: u64,
    pub omitted_conversion_dimensions: u64,
    pub untyped_measures: u64,
    pub unordered_complex: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsAssemblyAdaptations {
    pub mapped_placements: u64,
    pub tolerated: TsToleratedEncodings,
}
/// A root's tolerated unit encodings and failure text, borrowed from its handle.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsRootReport {
    pub tolerated: TsToleratedEncodings,
    pub message: TsStringView,
}
impl TsRootReport {
    /// The C record of a stored report; an empty message is `{NULL, 0}`.
    pub(crate) fn new(tolerated: TsToleratedEncodings, message: &str) -> Self {
        Self {
            tolerated,
            message: if message.is_empty() {
                TsStringView::default()
            } else {
                TsStringView::new(message)
            },
        }
    }
}
pub(crate) fn encodings(e: ToleratedEncodings) -> TsToleratedEncodings {
    TsToleratedEncodings {
        explicit_si_dimensions: e.explicit_si_dimensions as u64,
        unset_si_dimensions: e.unset_si_dimensions as u64,
        omitted_conversion_dimensions: e.omitted_conversion_dimensions as u64,
        untyped_measures: e.untyped_measures as u64,
        unordered_complex: e.unordered_complex as u64,
    }
}

/// # Safety
/// Non-NULL out must be aligned writable storage for a complete C options record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_options_init(out: *mut TsAssemblyOptions) -> u32 {
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
pub unsafe extern "C" fn ts_document_import_assembly(
    document: *const TsDocument,
    options: *const TsAssemblyOptions,
    out: *mut *const TsAssembly,
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
            TsAssemblyOptions::default()
        } else {
            unsafe { *options }
        };
        let Some(options) = o.options() else {
            return TS_INVALID_ARGUMENT;
        };
        // A one-shot session, as `tessstep_import::import_assembly` creates.
        let session = ImportSession::new(document, options.assembly.import);
        // SAFETY: the outputs cleared above.
        unsafe { publish(document, import(document, &session, options), out, error) }
    })
}

/// # Safety
/// Follow tessstep.h live-document, options and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_import_assembly_with_options(
    document: *const TsDocument,
    preference: u32,
    options: *const TsImportOptions,
    out: *mut *const TsAssembly,
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
        let (Some(o), Some(preference)) = (
            (unsafe { read_options(options) }).and_then(TsImportOptions::options),
            discovery::preference(preference),
        ) else {
            return TS_INVALID_ARGUMENT;
        };
        // The session budgets are the caller's, not sized by the library.
        let session = ImportSession::with_budgets(document, o.import, o.session);
        // SAFETY: the outputs cleared above.
        unsafe {
            publish(
                document,
                import(document, &session, o.assembly(preference)),
                out,
                error,
            )
        }
    })
}

/// Import an assembly and its appearance through a session and copy them into C
/// records; the session's import options apply to both.
pub(crate) fn import(
    document: &Document,
    session: &ImportSession<'_>,
    options: AssemblyImportOptions,
) -> Result<TsAssembly, tessstep_import::Error> {
    let imported = session.import_assembly(options)?;
    let styles = session.import_appearance(&imported.assembly, StyleOptions::default())?;
    let assembly = &imported.assembly;
    let scene = Arc::new(TsScene::imported(assembly.scene().clone()));
    let appearance = Arc::new(TsAppearance::imported(
        (**styles.appearance()).clone(),
        scene.clone(),
    ));
    let roots: Vec<TsAssemblyRoot> = imported
        .roots
        .iter()
        .map(|r| {
            let (status, error) = match &r.result {
                Ok(()) => (TS_OK, TsImportError::default()),
                Err(e) => error_record(document, e),
            };
            TsAssemblyRoot {
                root_id: r.root.get(),
                kind: match r.kind {
                    ShapeRootKind::Solid(SolidKind::ManifoldSolidBrep) => {
                        TS_ROOT_MANIFOLD_SOLID_BREP
                    }
                    ShapeRootKind::Solid(SolidKind::BrepWithVoids) => TS_ROOT_BREP_WITH_VOIDS,
                    ShapeRootKind::Solid(SolidKind::FacetedBrep) => TS_ROOT_FACETED_BREP,
                    ShapeRootKind::Tessellated(TessellatedKind::Solid) => TS_ROOT_TESSELLATED_SOLID,
                    ShapeRootKind::Tessellated(TessellatedKind::Shell) => TS_ROOT_TESSELLATED_SHELL,
                    ShapeRootKind::Tessellated(TessellatedKind::SurfaceSet) => {
                        TS_ROOT_TESSELLATED_SURFACE_SET
                    }
                },
                selected: u32::from(r.selected),
                status,
                reserved: 0,
                error,
            }
        })
        .collect();
    let exclusions: Vec<TsAssemblyExclusion> = assembly
        .excluded()
        .iter()
        .chain(styles.excluded())
        .map(|x| {
            let (status, error) = error_record(document, &x.error);
            TsAssemblyExclusion {
                record_id: x.record.get(),
                structural: u32::from(x.structural),
                status,
                error,
            }
        })
        .collect();
    let c = styles.counts();
    let handle = TsAssembly {
        info: TsAssemblyInfo {
            node_count: assembly.nodes().len() as u64,
            product_count: assembly.products().len() as u64,
            occurrence_count: assembly.occurrence_count() as u64,
            root_count: roots.len() as u64,
            imported_root_count: roots.iter().filter(|r| r.status == TS_OK).count() as u64,
            unplaced_count: assembly.unplaced().len() as u64,
            unimported_count: assembly.unimported().len() as u64,
            excluded_count: exclusions.len() as u64,
            complete: u32::from(assembly.is_complete()),
            reserved: 0,
        },
        nodes: assembly
            .nodes()
            .iter()
            .map(|n| TsAssemblyNode {
                instance_id: n.instance.0,
                definition_id: n.definition.get(),
                occurrence_id: n.occurrence.map_or(0, EntityId::get),
                representation_id: n.representation.get(),
                root_id: n.root.map_or(0, EntityId::get),
            })
            .collect(),
        products: assembly
            .products()
            .iter()
            .map(|p| {
                (
                    p.definition.get(),
                    p.product.get(),
                    p.id.clone(),
                    p.name.clone(),
                )
            })
            .collect(),
        roots,
        exclusions,
        unplaced: assembly.unplaced().iter().map(|r| r.get()).collect(),
        style: TsStyleInfo {
            styled_items: c.styled_items as u64,
            asset_styles: c.asset_styles as u64,
            face_styles: c.face_styles as u64,
            unimported: c.unimported as u64,
            unsupported_targets: c.unsupported_targets.values().sum::<usize>() as u64,
            no_surface_colour: c.no_surface_colour as u64,
            context_styles: c.context_styles as u64,
            conflicts: c.conflicts as u64,
            complex_items: c.complex_items as u64,
            layers: c.layers as u64,
            invisibility: c.invisibility as u64,
            excluded: styles.excluded().len() as u64,
        },
        adaptations: TsAssemblyAdaptations {
            mapped_placements: assembly.mapped_placements() as u64,
            tolerated: encodings(assembly.tolerated()),
        },
        reports: imported
            .roots
            .iter()
            .map(|r| {
                let message = r.result.as_ref().err().map(|e| e.message.clone());
                (encodings(r.tolerated), message.unwrap_or_default())
            })
            .collect(),
        scene,
        appearance,
    };
    Ok(handle)
}

/// Publish an assembly or its located error.
///
/// # Safety
/// Cleared, writable output slots of the header contract.
pub(crate) unsafe fn publish(
    document: &Document,
    result: Result<TsAssembly, tessstep_import::Error>,
    out: *mut *const TsAssembly,
    error: *mut TsImportError,
) -> u32 {
    match result {
        Ok(assembly) => {
            // SAFETY: cleared output slot; ownership transfers once after all work.
            unsafe { out.write(Arc::into_raw(Arc::new(assembly))) };
            TS_OK
        }
        // SAFETY: the optional error record follows the header contract.
        Err(e) => unsafe { report(document, &e, error) },
    }
}

/// # Safety
/// The non-NULL assembly must be live for the entire call; each retain needs release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_retain(assembly: *const TsAssembly) -> u32 {
    boundary(|| {
        if assembly.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: returned by this library with a positive strong count.
        unsafe { Arc::increment_strong_count(assembly) };
        TS_OK
    })
}
/// # Safety
/// Releases exactly one acquisition; NULL is permitted. Follow tessstep.h lifetimes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_release(assembly: *const TsAssembly) {
    if !assembly.is_null() {
        // SAFETY: caller transfers one live strong reference from import/retain.
        drop(unsafe { Arc::from_raw(assembly) });
    }
}

/// Copy one record out of a live assembly.
///
/// # Safety
/// Non-NULL out is writable storage; non-NULL assembly is live for the call.
unsafe fn query<T: Default + Copy>(
    assembly: *const TsAssembly,
    out: *mut T,
    get: impl FnOnce(&TsAssembly) -> Option<T>,
) -> u32 {
    // SAFETY: non-NULL out is a valid writable C record.
    if !unsafe { clear(out) } || assembly.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    // SAFETY: caller keeps a live acquisition throughout this query.
    match get(unsafe { &*assembly }) {
        Some(value) => {
            // SAFETY: initialized writable output.
            unsafe { out.write(value) };
            TS_OK
        }
        None => TS_NOT_FOUND,
    }
}

/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_get_info(
    assembly: *const TsAssembly,
    out: *mut TsAssemblyInfo,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(assembly, out, |a| Some(a.info)) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_node_at(
    assembly: *const TsAssembly,
    index: usize,
    out: *mut TsAssemblyNode,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(assembly, out, |a| a.nodes.get(index).copied()) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts; the
/// returned text is borrowed from the assembly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_product_at(
    assembly: *const TsAssembly,
    index: usize,
    out: *mut TsAssemblyProduct,
) -> u32 {
    boundary(|| unsafe {
        // SAFETY: forwarded caller contract; views borrow the live assembly.
        query(assembly, out, |a| {
            a.products
                .get(index)
                .map(|(definition, product, id, name)| TsAssemblyProduct {
                    definition_id: *definition,
                    product_id: *product,
                    id: TsStringView::new(id),
                    name: TsStringView::new(name),
                })
        })
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_root_at(
    assembly: *const TsAssembly,
    index: usize,
    out: *mut TsAssemblyRoot,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(assembly, out, |a| a.roots.get(index).copied()) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_excluded_at(
    assembly: *const TsAssembly,
    index: usize,
    out: *mut TsAssemblyExclusion,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(assembly, out, |a| a.exclusions.get(index).copied()) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_unplaced_at(
    assembly: *const TsAssembly,
    index: usize,
    out: *mut u64,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(assembly, out, |a| a.unplaced.get(index).copied()) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_get_style_info(
    assembly: *const TsAssembly,
    out: *mut TsStyleInfo,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(assembly, out, |a| Some(a.style)) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_get_adaptations(
    assembly: *const TsAssembly,
    out: *mut TsAssemblyAdaptations,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(assembly, out, |a| Some(a.adaptations)) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts; the message
/// is borrowed from the assembly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_root_report_at(
    assembly: *const TsAssembly,
    index: usize,
    out: *mut TsRootReport,
) -> u32 {
    // SAFETY: forwarded caller contract; the view borrows the live assembly.
    boundary(|| unsafe {
        query(assembly, out, |a| {
            a.reports
                .get(index)
                .map(|(tolerated, message)| TsRootReport::new(*tolerated, message))
        })
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable pointer slot contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_get_scene(
    assembly: *const TsAssembly,
    out: *mut *const TsScene,
) -> u32 {
    // SAFETY: non-NULL out is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if assembly.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: the assembly remains live throughout this query.
        let a = unsafe { &*assembly };
        // SAFETY: initialized output; transfers an independent scene acquisition.
        unsafe { out.write(Arc::into_raw(a.scene.clone())) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable pointer slot contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_assembly_get_appearance(
    assembly: *const TsAssembly,
    out: *mut *const TsAppearance,
) -> u32 {
    // SAFETY: non-NULL out is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if assembly.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: the assembly remains live throughout this query.
        let a = unsafe { &*assembly };
        // SAFETY: initialized output; transfers an independent appearance acquisition.
        unsafe { out.write(Arc::into_raw(a.appearance.clone())) };
        TS_OK
    })
}
