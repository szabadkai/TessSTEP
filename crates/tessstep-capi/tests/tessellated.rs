use std::{ptr, slice};
use tessstep_capi::*;

/// # Safety
/// Returns an owned document acquisition; release it with ts_document_release.
unsafe fn parse(text: &str) -> *const TsDocument {
    let mut doc = ptr::null();
    let mut report = ptr::null_mut();
    // SAFETY: readable input for the call and disjoint writable output slots.
    unsafe {
        assert_eq!(
            ts_document_parse(
                text.as_ptr(),
                text.len(),
                ptr::null(),
                &mut doc,
                &mut report
            ),
            TS_OK
        );
        ts_diagnostics_release(report);
    }
    doc
}

#[test]
fn c_abi_tessellated_layouts() {
    assert_eq!(size_of::<TsTessellatedOptions>(), 48);
    assert_eq!(std::mem::offset_of!(TsTessellatedOptions, flags), 40);
    assert_eq!(size_of::<TsTessellatedInfo>(), 80);
    assert_eq!(
        std::mem::offset_of!(TsTessellatedInfo, pnmax_deviations),
        72
    );
    assert_eq!(size_of::<TsPresentationOptions>(), 64);
    assert_eq!(std::mem::offset_of!(TsPresentationOptions, flags), 56);
    assert_eq!(size_of::<TsPresentationInfo>(), 64);
    assert_eq!(size_of::<TsPresentationItem>(), 24);
    assert_eq!(size_of::<TsPresentationView>(), 104);
    assert_eq!(std::mem::offset_of!(TsPresentationView, point_count), 96);
}

#[test]
fn c_abi_tessellated_import_owns_mesh_and_reports_provenance() {
    let text = include_str!("../../../corpus/geometry/tessellated-connected.step");
    // SAFETY: complete disjoint records, live handles, balanced releases.
    unsafe {
        let doc = parse(text);
        let mut options = TsTessellatedOptions::default();
        assert_eq!(
            ts_tessellated_options_init(ptr::null_mut()),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(ts_tessellated_options_init(&mut options), TS_OK);
        assert_eq!((options.struct_size, options.flags), (48, 0));
        let (mut mesh, mut info, mut error) = (
            ptr::null(),
            TsTessellatedInfo::default(),
            TsImportError::default(),
        );
        assert_eq!(
            ts_document_import_tessellated(
                doc, 1000, 0.001, &options, &mut mesh, &mut info, &mut error
            ),
            TS_OK
        );
        assert_eq!(
            (
                info.kind,
                info.root_id,
                info.link_id,
                info.face_count,
                info.edge_count
            ),
            (TS_TESSELLATED_SOLID, 1000, 0, 6, 13)
        );
        assert_eq!(
            (info.vertex_count, info.joined_points, info.pnmax_deviations),
            (8, 24, 0)
        );
        assert_eq!(error.stage, 0);
        // Budget failures clear every output and publish no mesh.
        let mut failed = mesh;
        options.max_records = 3;
        assert_eq!(
            ts_document_import_tessellated(
                doc,
                1000,
                0.001,
                &options,
                &mut failed,
                &mut info,
                &mut error
            ),
            TS_RESOURCE_LIMIT
        );
        assert!(failed.is_null());
        assert_eq!((info.kind, info.face_count), (0, 0));
        assert_eq!(error.stage, 2);
        // Records, flags and arguments are validated before any work.
        options = TsTessellatedOptions::default();
        for (field, status) in [
            (0, TS_OK),
            (1, TS_INVALID_ARGUMENT),
            (2, TS_INVALID_ARGUMENT),
        ] {
            let mut bad = options;
            match field {
                1 => bad.flags = 2,
                2 => bad.reserved = 1,
                _ => bad.flags = TS_IMPORT_STRICT,
            }
            assert_eq!(
                ts_document_import_tessellated(
                    doc,
                    1000,
                    0.001,
                    &bad,
                    &mut failed,
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                status
            );
            ts_mesh_release(failed);
        }
        assert_eq!(
            ts_document_import_tessellated(
                doc,
                1000,
                0.,
                ptr::null(),
                &mut failed,
                &mut info,
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_document_import_tessellated(
                doc,
                9999,
                0.001,
                ptr::null(),
                &mut failed,
                &mut info,
                &mut error
            ),
            TS_NOT_FOUND
        );
        assert_eq!(
            ts_document_import_tessellated(
                doc,
                10,
                0.001,
                ptr::null(),
                &mut failed,
                &mut info,
                &mut error
            ),
            TS_UNSUPPORTED
        );
        assert_eq!((error.entity_id, error.stage), (10, 2));
        assert!(error.end_offset > error.start_offset);
        assert_eq!(
            ts_document_import_tessellated(
                ptr::null(),
                1000,
                0.001,
                ptr::null(),
                &mut failed,
                &mut info,
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_document_import_tessellated(
                doc,
                1000,
                0.001,
                ptr::null(),
                ptr::null_mut(),
                &mut info,
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        // The mesh is independent of the document.
        ts_document_release(doc);
        let mut view = TsMeshView::default();
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        assert_eq!(
            (view.vertex_count, view.triangle_count, view.boundary_edges),
            (8, 12, 0)
        );
        assert!((view.signed_volume - 6e-6).abs() < 1e-15);
        assert_eq!(*view.face_ids, 100);
        ts_mesh_release(mesh);
        // Strict rejects the tolerated pnmax deviation; the default counts it.
        let understated = include_str!("../../../corpus/geometry/tessellated-cube.step")
            .replace("('bottom',#1,8,(),$,(),", "('bottom',#1,7,(),$,(),");
        let doc = parse(&understated);
        assert_eq!(
            ts_document_import_tessellated(
                doc,
                1000,
                0.001,
                ptr::null(),
                &mut mesh,
                &mut info,
                &mut error
            ),
            TS_OK
        );
        assert_eq!(info.pnmax_deviations, 1);
        ts_mesh_release(mesh);
        options.flags = TS_IMPORT_STRICT;
        assert_eq!(
            ts_document_import_tessellated(
                doc, 1000, 0.001, &options, &mut mesh, &mut info, &mut error
            ),
            TS_INVALID_GEOMETRY
        );
        assert_eq!((error.entity_id, info.kind), (100, 0));
        ts_document_release(doc);
    }
}

#[test]
fn c_abi_presentation_views_are_zero_copy_and_retained() {
    let text = include_str!("../../../corpus/geometry/presentation.step");
    // SAFETY: complete disjoint records, live handles, balanced releases.
    unsafe {
        let doc = parse(text);
        let mut options = TsPresentationOptions::default();
        assert_eq!(ts_presentation_options_init(&mut options), TS_OK);
        assert_eq!((options.struct_size, options.max_depth), (64, 16));
        let (mut presentation, mut error) = (ptr::null(), TsImportError::default());
        assert_eq!(
            ts_document_import_presentation(
                doc,
                300,
                0.001,
                &options,
                &mut presentation,
                &mut error
            ),
            TS_OK
        );
        ts_document_release(doc);
        let mut info = TsPresentationInfo::default();
        assert_eq!(ts_presentation_get_info(presentation, &mut info), TS_OK);
        assert_eq!(
            (
                info.root_id,
                info.occurrence_id,
                info.geometric_set_id,
                info.style_count,
                info.item_count
            ),
            (300, 300, 100, 1, 4)
        );
        assert_eq!((info.skipped_degenerate, info.zero_area_triangles), (1, 1));
        let mut style = 0;
        assert_eq!(ts_presentation_style_at(presentation, 0, &mut style), TS_OK);
        assert_eq!(style, 203);
        assert_eq!(
            ts_presentation_style_at(presentation, 1, &mut style),
            TS_NOT_FOUND
        );
        assert_eq!(style, 0);
        let mut item = TsPresentationItem::default();
        assert_eq!(ts_presentation_item_at(presentation, 3, &mut item), TS_OK);
        assert_eq!(
            (
                item.entity_id,
                item.kind,
                item.supplied_normals,
                item.placements
            ),
            (150, TS_PRESENTATION_CURVE_SET, 0, 2)
        );
        assert_eq!(ts_presentation_item_at(presentation, 1, &mut item), TS_OK);
        assert_eq!(
            (item.kind, item.supplied_normals),
            (TS_PRESENTATION_SURFACE_SET, 1)
        );
        assert_eq!(
            ts_presentation_item_at(presentation, 4, &mut item),
            TS_NOT_FOUND
        );
        let mut view = TsPresentationView::default();
        assert_eq!(ts_presentation_get_view(presentation, &mut view), TS_OK);
        assert_eq!(
            (
                view.vertex_count,
                view.polyline_count,
                view.polyline_point_count
            ),
            (12, 3, 9)
        );
        assert_eq!((view.triangle_count, view.point_count), (3, 1));
        let offsets = slice::from_raw_parts(view.polyline_offsets, view.polyline_count + 1);
        assert_eq!(offsets, [0, 5, 7, 9]);
        assert_eq!(
            slice::from_raw_parts(view.polyline_items, 3),
            [110, 110, 150]
        );
        assert_eq!(slice::from_raw_parts(view.positions, 3), [0.1, 0., 0.]);
        assert_eq!(*view.point_items, 130);
        // Retained views keep the same buffers after the original acquisition ends.
        assert_eq!(ts_presentation_retain(presentation), TS_OK);
        ts_presentation_release(presentation);
        let mut again = TsPresentationView::default();
        assert_eq!(ts_presentation_get_view(presentation, &mut again), TS_OK);
        assert_eq!(again.positions, view.positions);
        ts_presentation_release(presentation);
        // Empty arrays are NULL; failures clear outputs.
        let doc = parse(text);
        assert_eq!(
            ts_document_import_presentation(
                doc,
                301,
                0.001,
                ptr::null(),
                &mut presentation,
                &mut error
            ),
            TS_OK
        );
        assert_eq!(ts_presentation_get_view(presentation, &mut view), TS_OK);
        assert!(view.triangles.is_null() && view.points.is_null() && view.triangle_count == 0);
        assert!(!view.polyline_offsets.is_null());
        let mut failed = presentation;
        options.max_depth = 0;
        assert_eq!(
            ts_document_import_presentation(doc, 300, 0.001, &options, &mut failed, &mut error),
            TS_RESOURCE_LIMIT
        );
        assert!(failed.is_null());
        assert_eq!(
            ts_document_import_presentation(doc, 110, 0.001, ptr::null(), &mut failed, &mut error),
            TS_UNSUPPORTED
        );
        assert_eq!((error.stage, error.entity_id), (2, 110));
        options = TsPresentationOptions::default();
        options.abi_version = 2;
        assert_eq!(
            ts_document_import_presentation(doc, 300, 0.001, &options, &mut failed, &mut error),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(error.stage, 0);
        ts_document_release(doc);
        assert_eq!(
            ts_presentation_get_info(ptr::null(), &mut info),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(info.root_id, 0);
        assert_eq!(
            ts_presentation_get_view(ptr::null(), &mut view),
            TS_INVALID_ARGUMENT
        );
        assert!(view.positions.is_null());
        assert_eq!(ts_presentation_retain(ptr::null()), TS_INVALID_ARGUMENT);
        ts_presentation_release(ptr::null());
        ts_presentation_release(presentation);
    }
}
