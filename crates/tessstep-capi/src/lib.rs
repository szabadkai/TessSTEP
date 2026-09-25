//! ABI 1 bridge. The public contract and layouts are in include/tessstep/tessstep.h.
//! Opaque handles expose no Rust representation. This is the only unsafe crate.
#![deny(unsafe_op_in_unsafe_fn)]

mod appearance;
pub use appearance::*;
mod mesh;
mod scene;
pub use mesh::*;
pub use scene::*;

use std::{panic::catch_unwind, ptr, sync::Arc};
use tessstep_model::Document;
use tessstep_part21::{Diagnostic, DiagnosticCode, EntityId, ParseLimits, Severity};

pub const TS_OK: u32 = 0;
pub const TS_INVALID_ARGUMENT: u32 = 1;
pub const TS_PARSE_ERROR: u32 = 2;
pub const TS_UNSUPPORTED: u32 = 3;
pub const TS_RESOURCE_LIMIT: u32 = 4;
pub const TS_NOT_FOUND: u32 = 5;
pub const TS_INTERNAL_ERROR: u32 = 6;
pub const TS_INVALID_MESH: u32 = 7;
pub const TS_INVALID_SCENE: u32 = 8;
pub const TS_INVALID_APPEARANCE: u32 = 9;

#[derive(Debug)]
pub struct TsDocument {
    document: Document,
    entities: Vec<TsEntityInfo>,
}
#[derive(Debug)]
pub struct TsDiagnostics(Vec<Diagnostic>);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TsParseOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub max_input_bytes: u64,
    pub max_token_bytes: u64,
    pub max_string_bytes: u64,
    pub max_entities: u64,
    pub max_nesting_depth: u64,
    pub max_aggregate_elements: u64,
    pub max_total_values: u64,
    pub max_symbols: u64,
    pub max_records: u64,
    pub max_sections: u64,
}
impl Default for TsParseOptions {
    fn default() -> Self {
        let l = ParseLimits::default();
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            max_input_bytes: l.max_input_bytes,
            max_token_bytes: l.max_token_bytes as u64,
            max_string_bytes: l.max_string_bytes as u64,
            max_entities: l.max_entities as u64,
            max_nesting_depth: l.max_nesting_depth as u64,
            max_aggregate_elements: l.max_aggregate_elements as u64,
            max_total_values: l.max_total_values as u64,
            max_symbols: l.max_symbols as u64,
            max_records: l.max_records as u64,
            max_sections: l.max_sections as u64,
        }
    }
}
impl TsParseOptions {
    fn limits(self) -> Result<ParseLimits, u32> {
        if self.struct_size != size_of::<Self>() as u32 || self.abi_version != 1 {
            return Err(TS_INVALID_ARGUMENT);
        }
        let count = |v| usize::try_from(v).map_err(|_| TS_INVALID_ARGUMENT);
        Ok(ParseLimits {
            max_input_bytes: self.max_input_bytes,
            max_token_bytes: count(self.max_token_bytes)?,
            max_string_bytes: count(self.max_string_bytes)?,
            max_entities: count(self.max_entities)?,
            max_nesting_depth: count(self.max_nesting_depth)?,
            max_aggregate_elements: count(self.max_aggregate_elements)?,
            max_total_values: count(self.max_total_values)?,
            max_symbols: count(self.max_symbols)?,
            max_records: count(self.max_records)?,
            max_sections: count(self.max_sections)?,
        })
    }
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TsDocumentInfo {
    pub entity_count: u64,
    pub header_count: u64,
    pub data_section_count: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TsEntityInfo {
    pub id: u64,
    pub record_count: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TsStringView {
    pub data: *const std::ffi::c_char,
    pub size: usize,
}
impl Default for TsStringView {
    fn default() -> Self {
        Self {
            data: ptr::null(),
            size: 0,
        }
    }
}
impl TsStringView {
    fn new(text: &str) -> Self {
        Self {
            data: text.as_ptr().cast(),
            size: text.len(),
        }
    }
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TsDiagnostic {
    pub severity: u32,
    pub reserved: u32,
    pub entity_id: u64,
    pub start_offset: u64,
    pub end_offset: u64,
    pub line: u64,
    pub column: u64,
    pub code: TsStringView,
    pub message: TsStringView,
}

// No AssertUnwindSafe: captured state must genuinely meet UnwindSafe. All work
// precedes output publication. Forget an unexpected panic payload because its
// destructor is user-defined Rust code and could itself panic across the ABI.
fn boundary(f: impl FnOnce() -> u32 + std::panic::UnwindSafe) -> u32 {
    match catch_unwind(f) {
        Ok(status) => status,
        Err(payload) => {
            std::mem::forget(payload);
            TS_INTERNAL_ERROR
        }
    }
}

// SAFETY: the caller provides either NULL or writable, aligned, disjoint output
// storage. Initialize by writing, never read/drop uninitialized caller memory.
unsafe fn clear<T: Default>(out: *mut T) -> bool {
    // SAFETY: the caller provides the same output storage contract.
    unsafe { initialize(out, T::default()) }
}

// Pointer outputs use explicit null values: raw pointers do not implement
// Default on the minimum supported Rust 1.85 toolchain.
// SAFETY: same output storage contract as clear.
unsafe fn initialize<T>(out: *mut T, value: T) -> bool {
    if out.is_null() {
        return false;
    }
    unsafe { out.write(value) };
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn ts_api_version() -> u32 {
    1
}

/// # Safety
/// Follow the output storage contract in tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_parse_options_init(out: *mut TsParseOptions) -> u32 {
    boundary(|| {
        // SAFETY: caller promises valid writable output when non-NULL.
        if unsafe { clear(out) } {
            TS_OK
        } else {
            TS_INVALID_ARGUMENT
        }
    })
}

/// # Safety
/// Follow tessstep.h pointer/count, options, and disjoint output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_parse(
    data: *const u8,
    size: usize,
    options: *const TsParseOptions,
    out: *mut *const TsDocument,
    error_report: *mut *mut TsDiagnostics,
) -> u32 {
    // SAFETY: both outputs, when non-NULL, are valid independent pointer slots.
    let outputs_valid = unsafe { initialize(out, ptr::null()) }
        & unsafe { initialize(error_report, ptr::null_mut()) };
    if !outputs_valid {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if (data.is_null() && size != 0) || size > isize::MAX as usize {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: options is borrowed, aligned, and valid for the call if non-NULL.
        let options = if options.is_null() {
            TsParseOptions::default()
        } else {
            unsafe { *options }
        };
        let limits = match options.limits() {
            Ok(limits) => limits,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees size valid readable bytes. NULL with zero size
        // is handled separately because Rust slices require a non-NULL pointer.
        let input = if size == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(data, size) }
        };
        match tessstep_model::parse(input, limits) {
            Ok(document) => {
                let entities = document
                    .entities()
                    .iter()
                    .map(|entity| TsEntityInfo {
                        id: entity.id.get(),
                        record_count: entity.kind.records().len() as u64,
                    })
                    .collect();
                let owned = Arc::new(TsDocument { document, entities });
                // SAFETY: output is valid and cleared; publish only after all fallible work.
                unsafe { out.write(Arc::into_raw(owned)) };
                TS_OK
            }
            Err(diagnostic) => {
                let status = match diagnostic.code {
                    DiagnosticCode::LimitExceeded => TS_RESOURCE_LIMIT,
                    DiagnosticCode::UnsupportedFeature => TS_UNSUPPORTED,
                    _ => TS_PARSE_ERROR,
                };
                let report = Box::new(TsDiagnostics(vec![diagnostic]));
                // SAFETY: output is valid; ownership transfers to the C caller.
                unsafe { error_report.write(Box::into_raw(report)) };
                status
            }
        }
    })
}

/// # Safety
/// document must be NULL or a live acquired handle, as described in tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_retain(document: *const TsDocument) -> u32 {
    if document.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: live handles originate in Arc::into_raw; a reference remains alive.
        unsafe { Arc::increment_strong_count(document) };
        TS_OK
    })
}
/// # Safety
/// Release exactly one acquired reference, or NULL. See tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_release(document: *const TsDocument) {
    if !document.is_null() {
        boundary(|| {
            // SAFETY: caller transfers one live reference back to its allocator.
            unsafe { drop(Arc::from_raw(document)) };
            TS_OK
        });
    }
}
/// # Safety
/// Follow the live handle and output storage contracts in tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_get_info(
    document: *const TsDocument,
    out: *mut TsDocumentInfo,
) -> u32 {
    // SAFETY: caller provides valid output storage when non-NULL.
    if !unsafe { clear(out) } || document.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: document stays alive for the call; shared reads only.
        let doc = unsafe { &(*document).document };
        let info = TsDocumentInfo {
            entity_count: doc.entities().len() as u64,
            header_count: doc.headers().len() as u64,
            data_section_count: doc.data_sections().len() as u64,
        };
        // SAFETY: valid writable output, disjoint from document.
        unsafe { out.write(info) };
        TS_OK
    })
}
/// # Safety
/// Follow the live handle and output storage contracts in tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_entity_at(
    document: *const TsDocument,
    index: usize,
    out: *mut TsEntityInfo,
) -> u32 {
    // SAFETY: caller provides valid output storage when non-NULL.
    if !unsafe { clear(out) } || document.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: immutable handle stays alive throughout the call.
        let entities = unsafe { &(*document).entities };
        let Some(info) = entities.get(index) else {
            return TS_NOT_FOUND;
        };
        // SAFETY: output is valid and disjoint from the input handle.
        unsafe { out.write(*info) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h; returned text is borrowed from the live document.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_record_name(
    document: *const TsDocument,
    id: u64,
    component: usize,
    out: *mut TsStringView,
) -> u32 {
    // SAFETY: caller provides valid output storage when non-NULL.
    if !unsafe { clear(out) } || document.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        let Some(id) = EntityId::new(id) else {
            return TS_NOT_FOUND;
        };
        // SAFETY: immutable handle stays alive throughout the call and text borrow.
        let doc = unsafe { &(*document).document };
        let Some(entity) = doc.entities().get(id) else {
            return TS_NOT_FOUND;
        };
        let Some(record) = entity.kind.records().get(component) else {
            return TS_NOT_FOUND;
        };
        // SAFETY: output valid; text storage remains owned by the live document.
        unsafe { out.write(TsStringView::new(&record.name)) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h; successful output acquires an independent report.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_document_diagnostics(
    document: *const TsDocument,
    out: *mut *mut TsDiagnostics,
) -> u32 {
    // SAFETY: caller provides valid output storage when non-NULL.
    if !unsafe { initialize(out, ptr::null_mut()) } || document.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: immutable handle stays alive throughout the call.
        let diagnostics = unsafe { &(*document).document }.diagnostics();
        let report = Box::new(TsDiagnostics(diagnostics));
        // SAFETY: valid output; ownership transfers only after all fallible work.
        unsafe { out.write(Box::into_raw(report)) };
        TS_OK
    })
}
/// # Safety
/// Release exactly one acquired report, or NULL. See tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_diagnostics_release(report: *mut TsDiagnostics) {
    if !report.is_null() {
        boundary(|| {
            // SAFETY: uniquely owned report was acquired with Box::into_raw.
            unsafe { drop(Box::from_raw(report)) };
            TS_OK
        });
    }
}
/// # Safety
/// Follow the live report and output storage contracts in tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_diagnostics_count(
    report: *const TsDiagnostics,
    out: *mut usize,
) -> u32 {
    // SAFETY: caller provides valid output storage when non-NULL.
    if !unsafe { clear(out) } || report.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: report is live and immutable; output is valid and disjoint.
        unsafe { out.write((*report).0.len()) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h; output text is borrowed from report until its release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_diagnostics_get(
    report: *const TsDiagnostics,
    index: usize,
    out: *mut TsDiagnostic,
) -> u32 {
    // SAFETY: caller provides valid output storage when non-NULL.
    if !unsafe { clear(out) } || report.is_null() {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: immutable report stays alive throughout the call and text borrow.
        let diagnostics = unsafe { &(*report).0 };
        let Some(d) = diagnostics.get(index) else {
            return TS_NOT_FOUND;
        };
        let result = TsDiagnostic {
            severity: match d.severity {
                Severity::Error => 1,
                Severity::Warning => 2,
            },
            reserved: 0,
            entity_id: d.entity.map_or(0, EntityId::get),
            start_offset: d.source_span.start.offset,
            end_offset: d.source_span.end.offset,
            line: d.source_span.start.line,
            column: d.source_span.start.column,
            code: TsStringView::new(d.code.as_str()),
            message: TsStringView::new(&d.message),
        };
        // SAFETY: output storage valid and disjoint from the report.
        unsafe { out.write(result) };
        TS_OK
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panic_is_contained_even_with_panicking_payload_destructor() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("payload drop");
            }
        }
        assert_eq!(
            boundary(|| std::panic::panic_any(Payload)),
            TS_INTERNAL_ERROR
        );
        assert_eq!(boundary(|| TS_OK), TS_OK);
    }
    #[test]
    fn handles_are_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<TsDocument>();
        check::<TsDiagnostics>();
        check::<TsScene>();
        check::<TsAppearance>();
    }
}
