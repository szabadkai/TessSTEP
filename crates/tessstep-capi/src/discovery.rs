//! Root and unit discovery with representation selection (Milestone 22).
use crate::{
    TS_INVALID_ARGUMENT, TS_NOT_FOUND, TS_OK, TS_ROOT_BREP_WITH_VOIDS, TS_ROOT_FACETED_BREP,
    TS_ROOT_MANIFOLD_SOLID_BREP, TS_ROOT_SHELL_BASED_SURFACE_MODEL, TS_ROOT_TESSELLATED_SHELL,
    TS_ROOT_TESSELLATED_SOLID, TS_ROOT_TESSELLATED_SURFACE_SET, TsDocument, TsImportError,
    TsImportOptions, boundary, clear,
    import::{error_record, report},
    initialize,
    options::{Options, read_options},
    tessellated::array,
};
use std::{ptr, sync::Arc};
use tessstep_import::{
    ContextUnits, ImportSession, Pairing, PresentationRoot, RepresentationChoice,
    RepresentationPreference, RootRef, SolidKind, SolidRoot, SurfaceModelRoot, TessellatedKind,
    TessellatedRoot,
};
use tessstep_model::Document;
use tessstep_part21::EntityId;

pub const TS_PREFER_EXACT: u32 = 1;
pub const TS_PREFER_TESSELLATED: u32 = 2;
pub const TS_PAIRING_NONE: u32 = 0;
pub const TS_PAIRING_LINK: u32 = 1;
pub const TS_PAIRING_RELATIONSHIP: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsDiscoveryInfo {
    pub root_count: u64,
    pub solid_count: u64,
    pub tessellation_count: u64,
    pub presentation_count: u64,
    pub choice_count: u64,
    pub preference: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsContextUnits {
    pub context_id: u64,
    pub metres_per_unit: f64,
    pub radians_per_unit: f64,
    pub distance_uncertainty_m: f64,
    pub has_distance_uncertainty: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsDiscoveredRoot {
    pub root_id: u64,
    pub kind: u32,
    pub status: u32,
    pub link_id: u64,
    pub representations: *const u64,
    pub representation_count: usize,
    pub units: TsContextUnits,
    pub error: TsImportError,
}
impl Default for TsDiscoveredRoot {
    fn default() -> Self {
        Self {
            root_id: 0,
            kind: 0,
            status: 0,
            link_id: 0,
            representations: ptr::null(),
            representation_count: 0,
            units: TsContextUnits::default(),
            error: TsImportError::default(),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsDiscoveredPresentation {
    pub occurrence_id: u64,
    pub status: u32,
    pub reserved: u32,
    pub containers: *const u64,
    pub container_count: usize,
    pub representations: *const u64,
    pub representation_count: usize,
    pub units: TsContextUnits,
    pub error: TsImportError,
}
impl Default for TsDiscoveredPresentation {
    fn default() -> Self {
        Self {
            occurrence_id: 0,
            status: 0,
            reserved: 0,
            containers: ptr::null(),
            container_count: 0,
            representations: ptr::null(),
            representation_count: 0,
            units: TsContextUnits::default(),
            error: TsImportError::default(),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsRepresentationChoice {
    pub selected: usize,
    pub pairing: u32,
    pub reserved: u32,
    pub alternatives: *const usize,
    pub alternative_count: usize,
}
impl Default for TsRepresentationChoice {
    fn default() -> Self {
        Self {
            selected: 0,
            pairing: 0,
            reserved: 0,
            alternatives: ptr::null(),
            alternative_count: 0,
        }
    }
}

/// Units or why none apply, as C scalars.
#[derive(Debug)]
struct Units {
    status: u32,
    units: TsContextUnits,
    error: TsImportError,
}
impl Units {
    fn new(
        document: &tessstep_model::Document,
        units: &Result<ContextUnits, tessstep_import::Error>,
    ) -> Self {
        match units {
            Ok(u) => Self {
                status: TS_OK,
                units: TsContextUnits {
                    context_id: u.context.get(),
                    metres_per_unit: u.length.scale(),
                    radians_per_unit: u.plane_angle.scale(),
                    distance_uncertainty_m: u.distance_uncertainty.unwrap_or(0.),
                    has_distance_uncertainty: u32::from(u.distance_uncertainty.is_some()),
                    reserved: 0,
                },
                error: TsImportError::default(),
            },
            Err(e) => {
                let (status, error) = error_record(document, e);
                Self {
                    status,
                    units: TsContextUnits::default(),
                    error,
                }
            }
        }
    }
}
#[derive(Debug)]
struct Root {
    id: u64,
    kind: u32,
    link: u64,
    representations: Vec<u64>,
    units: Units,
}
#[derive(Debug)]
struct Presentation {
    id: u64,
    containers: Vec<u64>,
    representations: Vec<u64>,
    units: Units,
}
#[derive(Debug)]
struct Choice {
    selected: usize,
    alternatives: Vec<usize>,
    pairing: u32,
}
/// Private owned records implementing the C queries; no kernel layouts exposed.
#[derive(Debug)]
pub struct TsDiscovery {
    info: TsDiscoveryInfo,
    roots: Vec<Root>,
    presentations: Vec<Presentation>,
    choices: Vec<Choice>,
}

fn ids(ids: &[EntityId]) -> Vec<u64> {
    ids.iter().map(|id| id.get()).collect()
}

/// The Rust preference of a C value.
pub(crate) fn preference(value: u32) -> Option<RepresentationPreference> {
    match value {
        TS_PREFER_EXACT => Some(RepresentationPreference::Exact),
        TS_PREFER_TESSELLATED => Some(RepresentationPreference::Tessellated),
        _ => None,
    }
}

/// Every discovery result, borrowed from the free functions' owned results or from
/// a session's cache.
pub(crate) struct Found<'r> {
    pub solids: &'r [SolidRoot],
    pub tessellations: &'r [TessellatedRoot],
    pub choices: &'r [RepresentationChoice],
    pub presentations: &'r [PresentationRoot],
    pub surface_models: &'r [SurfaceModelRoot],
    pub preference: RepresentationPreference,
}

/// Discover through a session (whose options must be `options`), or through the free
/// functions. Surface models, not cached by sessions, use the free function.
pub(crate) fn discover(
    document: &Document,
    session: Option<&ImportSession<'_>>,
    preference: RepresentationPreference,
    options: &Options,
) -> Result<TsDiscovery, tessstep_import::Error> {
    let import = options.import;
    let surface_models = if options.surface_models {
        tessstep_import::discover_surface_models(document, import)?
    } else {
        Vec::new()
    };
    if let Some(s) = session {
        return Ok(build(
            document,
            Found {
                solids: s.solids()?,
                tessellations: s.tessellations()?,
                choices: s.representation_choices(preference)?,
                presentations: s.presentations()?,
                surface_models: &surface_models,
                preference,
            },
        ));
    }
    let solids = tessstep_import::discover_solids(document, import)?;
    let tessellations = tessstep_import::discover_tessellations(document, import)?;
    let choices = tessstep_import::select_representations(
        document,
        &solids,
        &tessellations,
        preference,
        import,
    )?;
    let presentations = tessstep_import::discover_presentations(document, import)?;
    Ok(build(
        document,
        Found {
            solids: &solids,
            tessellations: &tessellations,
            choices: &choices,
            presentations: &presentations,
            surface_models: &surface_models,
            preference,
        },
    ))
}

/// Copy discovery results into private C records once.
fn build(document: &Document, found: Found<'_>) -> TsDiscovery {
    let Found {
        solids,
        tessellations,
        choices,
        presentations,
        surface_models,
        preference,
    } = found;
    let mut roots: Vec<Root> = solids
        .iter()
        .map(|s| Root {
            id: s.entity.get(),
            kind: match s.kind {
                SolidKind::ManifoldSolidBrep => TS_ROOT_MANIFOLD_SOLID_BREP,
                SolidKind::BrepWithVoids => TS_ROOT_BREP_WITH_VOIDS,
                SolidKind::FacetedBrep => TS_ROOT_FACETED_BREP,
            },
            link: 0,
            representations: ids(&s.representations),
            units: Units::new(document, &s.units),
        })
        .collect();
    roots.extend(tessellations.iter().map(|t| Root {
        id: t.entity.get(),
        kind: match t.kind {
            TessellatedKind::Solid => TS_ROOT_TESSELLATED_SOLID,
            TessellatedKind::Shell => TS_ROOT_TESSELLATED_SHELL,
            TessellatedKind::SurfaceSet => TS_ROOT_TESSELLATED_SURFACE_SET,
        },
        link: t.link.map_or(0, EntityId::get),
        representations: ids(&t.representations),
        units: Units::new(document, &t.units),
    }));
    // Surface models follow every root that representation selection indexes.
    roots.extend(surface_models.iter().map(|m| Root {
        id: m.entity.get(),
        kind: TS_ROOT_SHELL_BASED_SURFACE_MODEL,
        link: 0,
        representations: ids(&m.representations),
        units: Units::new(document, &m.units),
    }));
    // Shape-root indices: solids first, then tessellations.
    let index = |r: RootRef| match r {
        RootRef::Solid(i) => i,
        RootRef::Tessellation(i) => solids.len() + i,
    };
    let choices: Vec<Choice> = choices
        .iter()
        .map(|c| Choice {
            selected: index(c.selected),
            alternatives: c.alternatives.iter().map(|&a| index(a)).collect(),
            pairing: match c.pairing {
                None => TS_PAIRING_NONE,
                Some(Pairing::Link) => TS_PAIRING_LINK,
                Some(Pairing::Relationship) => TS_PAIRING_RELATIONSHIP,
            },
        })
        .collect();
    let presentations: Vec<Presentation> = presentations
        .iter()
        .map(|p| Presentation {
            id: p.entity.get(),
            containers: ids(&p.containers),
            representations: ids(&p.representations),
            units: Units::new(document, &p.units),
        })
        .collect();
    TsDiscovery {
        info: TsDiscoveryInfo {
            root_count: roots.len() as u64,
            solid_count: solids.len() as u64,
            tessellation_count: tessellations.len() as u64,
            presentation_count: presentations.len() as u64,
            choice_count: choices.len() as u64,
            preference: match preference {
                RepresentationPreference::Exact => TS_PREFER_EXACT,
                RepresentationPreference::Tessellated => TS_PREFER_TESSELLATED,
            },
            reserved: 0,
        },
        roots,
        presentations,
        choices,
    }
}

/// # Safety
/// Follow tessstep.h live-document, options and disjoint output-slot contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_discover(
    document: *const TsDocument,
    preference: u32,
    options: *const TsImportOptions,
    out: *mut *const TsDiscovery,
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
        let Some(preference) = self::preference(preference) else {
            return TS_INVALID_ARGUMENT;
        };
        // SAFETY: the outputs cleared above.
        unsafe {
            publish(
                document,
                discover(document, None, preference, &o),
                out,
                error,
            )
        }
    })
}

/// Publish a discovery or its located error.
///
/// # Safety
/// Cleared, writable output slots of the header contract.
pub(crate) unsafe fn publish(
    document: &Document,
    result: Result<TsDiscovery, tessstep_import::Error>,
    out: *mut *const TsDiscovery,
    error: *mut TsImportError,
) -> u32 {
    match result {
        Ok(discovery) => {
            // SAFETY: cleared output slot; ownership transfers once after all work.
            unsafe { out.write(Arc::into_raw(Arc::new(discovery))) };
            TS_OK
        }
        // SAFETY: the optional error record follows the header contract.
        Err(e) => unsafe { report(document, &e, error) },
    }
}

/// # Safety
/// The non-NULL discovery must be live for the entire call; each retain needs release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_discovery_retain(discovery: *const TsDiscovery) -> u32 {
    boundary(|| {
        if discovery.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: returned by this library with a positive strong count.
        unsafe { Arc::increment_strong_count(discovery) };
        TS_OK
    })
}
/// # Safety
/// Releases exactly one acquisition; NULL is permitted. Follow tessstep.h lifetimes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_discovery_release(discovery: *const TsDiscovery) {
    if !discovery.is_null() {
        // SAFETY: caller transfers one live strong reference from discovery/retain.
        drop(unsafe { Arc::from_raw(discovery) });
    }
}

/// Copy one record out of a live discovery.
///
/// # Safety
/// Non-NULL out is writable storage; non-NULL discovery is live for the call.
unsafe fn query<T: Default>(
    discovery: *const TsDiscovery,
    out: *mut T,
    get: impl FnOnce(&TsDiscovery) -> Option<T>,
) -> u32 {
    // SAFETY: non-NULL out is a valid writable C record.
    if !unsafe { clear(out) } || discovery.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    // SAFETY: caller keeps a live acquisition throughout this query.
    match get(unsafe { &*discovery }) {
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
pub unsafe extern "C" fn ts_discovery_get_info(
    discovery: *const TsDiscovery,
    out: *mut TsDiscoveryInfo,
) -> u32 {
    // SAFETY: forwarded caller contract.
    boundary(|| unsafe { query(discovery, out, |d| Some(d.info)) })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts; the
/// representation array is borrowed from the discovery.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_discovery_root_at(
    discovery: *const TsDiscovery,
    index: usize,
    out: *mut TsDiscoveredRoot,
) -> u32 {
    boundary(|| unsafe {
        // SAFETY: forwarded caller contract; arrays borrow the live discovery.
        query(discovery, out, |d| {
            d.roots.get(index).map(|r| TsDiscoveredRoot {
                root_id: r.id,
                kind: r.kind,
                status: r.units.status,
                link_id: r.link,
                representations: array(&r.representations),
                representation_count: r.representations.len(),
                units: r.units.units,
                error: r.units.error,
            })
        })
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts; the
/// container and representation arrays are borrowed from the discovery.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_discovery_presentation_at(
    discovery: *const TsDiscovery,
    index: usize,
    out: *mut TsDiscoveredPresentation,
) -> u32 {
    boundary(|| unsafe {
        // SAFETY: forwarded caller contract; arrays borrow the live discovery.
        query(discovery, out, |d| {
            d.presentations
                .get(index)
                .map(|p| TsDiscoveredPresentation {
                    occurrence_id: p.id,
                    status: p.units.status,
                    reserved: 0,
                    containers: array(&p.containers),
                    container_count: p.containers.len(),
                    representations: array(&p.representations),
                    representation_count: p.representations.len(),
                    units: p.units.units,
                    error: p.units.error,
                })
        })
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts; the
/// alternative array is borrowed from the discovery.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_discovery_choice_at(
    discovery: *const TsDiscovery,
    index: usize,
    out: *mut TsRepresentationChoice,
) -> u32 {
    boundary(|| unsafe {
        // SAFETY: forwarded caller contract; arrays borrow the live discovery.
        query(discovery, out, |d| {
            d.choices.get(index).map(|c| TsRepresentationChoice {
                selected: c.selected,
                pairing: c.pairing,
                reserved: 0,
                alternatives: array(&c.alternatives),
                alternative_count: c.alternatives.len(),
            })
        })
    })
}
