use std::ptr;
use tessstep_capi::*;

#[test]
fn c_abi_document_and_diagnostic_contract() {
    // SAFETY: these tests obey the C protocol with valid storage and balanced ownership.
    unsafe {
        let mut document = ptr::null();
        let mut report = ptr::null_mut();
        assert_eq!(
            ts_document_parse(ptr::null(), 0, ptr::null(), &mut document, &mut report),
            TS_PARSE_ERROR
        );
        assert!(document.is_null());
        assert!(!report.is_null());
        let mut d = TsDiagnostic::default();
        assert_eq!(ts_diagnostics_get(report, 0, &mut d), TS_OK);
        assert_eq!(d.severity, 1);
        ts_diagnostics_release(report);
        let input = include_bytes!("../../../corpus/part21/valid/values.step");
        assert_eq!(
            ts_document_parse(
                input.as_ptr(),
                input.len(),
                ptr::null(),
                &mut document,
                &mut report
            ),
            TS_OK
        );
        assert!(report.is_null());
        assert_eq!(ts_document_retain(document), TS_OK);
        ts_document_release(document);
        let mut info = TsDocumentInfo::default();
        assert_eq!(ts_document_get_info(document, &mut info), TS_OK);
        assert!(info.entity_count > 0);
        assert_eq!(ts_document_diagnostics(document, &mut report), TS_OK);
        ts_document_release(document);
        let mut count = 0;
        assert_eq!(ts_diagnostics_count(report, &mut count), TS_OK);
        ts_diagnostics_release(report);
    }
}

#[test]
fn c_abi_frozen_layouts() {
    assert_eq!(size_of::<TsParseOptions>(), 88);
    assert_eq!(std::mem::offset_of!(TsParseOptions, max_sections), 80);
    assert_eq!(size_of::<TsDocumentInfo>(), 24);
    assert_eq!(size_of::<TsEntityInfo>(), 16);
    assert_eq!(size_of::<TsStringView>(), 16);
    assert_eq!(size_of::<TsDiagnostic>(), 80);
    assert_eq!(std::mem::offset_of!(TsDiagnostic, code), 48);
    assert_eq!(std::mem::offset_of!(TsDiagnostic, message), 64);
}

#[test]
fn c_abi_budget_and_failure_states() {
    let input = include_bytes!("../../../corpus/part21/valid/values.step");
    for budget in 0..10 {
        let mut options = TsParseOptions::default();
        let field = match budget {
            0 => &mut options.max_input_bytes,
            1 => &mut options.max_token_bytes,
            2 => &mut options.max_string_bytes,
            3 => &mut options.max_entities,
            4 => &mut options.max_nesting_depth,
            5 => &mut options.max_aggregate_elements,
            6 => &mut options.max_total_values,
            7 => &mut options.max_symbols,
            8 => &mut options.max_records,
            _ => &mut options.max_sections,
        };
        *field = 0;
        // SAFETY: valid options and input, independent output slots, report released once.
        unsafe {
            let mut doc = ptr::null();
            let mut report = ptr::null_mut();
            assert_eq!(
                ts_document_parse(input.as_ptr(), input.len(), &options, &mut doc, &mut report),
                TS_RESOURCE_LIMIT,
                "budget {budget}"
            );
            assert!(doc.is_null());
            assert!(!report.is_null());
            ts_diagnostics_release(report);
        }
    }
    // SAFETY: sentinel pointer values are only placed in output slots, never
    // dereferenced or released; failures must overwrite them without reading them.
    unsafe {
        let mut doc = ptr::dangling();
        let mut report = ptr::dangling_mut();
        assert_eq!(
            ts_document_parse(ptr::null(), 1, ptr::null(), &mut doc, &mut report),
            TS_INVALID_ARGUMENT
        );
        assert!(doc.is_null() && report.is_null());
        report = ptr::dangling_mut();
        assert_eq!(
            ts_document_parse(ptr::null(), 0, ptr::null(), ptr::null_mut(), &mut report),
            TS_INVALID_ARGUMENT
        );
        assert!(report.is_null());
        doc = ptr::dangling();
        assert_eq!(
            ts_document_parse(ptr::null(), 0, ptr::null(), &mut doc, ptr::null_mut()),
            TS_INVALID_ARGUMENT
        );
        assert!(doc.is_null());
        report = ptr::dangling_mut();
        assert_eq!(
            ts_document_diagnostics(ptr::null(), &mut report),
            TS_INVALID_ARGUMENT
        );
        assert!(report.is_null());
    }
}
