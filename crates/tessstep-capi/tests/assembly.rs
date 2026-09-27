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

/// # Safety
/// The view must borrow live storage.
unsafe fn text(view: TsStringView) -> String {
    // SAFETY: caller keeps the owner alive.
    let bytes = unsafe { slice::from_raw_parts(view.data.cast::<u8>(), view.size) };
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn c_abi_assembly_layouts() {
    assert_eq!(size_of::<TsSchemaInfo>(), 32);
    assert_eq!(size_of::<TsAssemblyOptions>(), 72);
    assert_eq!(std::mem::offset_of!(TsAssemblyOptions, flags), 64);
    assert_eq!(size_of::<TsAssemblyInfo>(), 72);
    assert_eq!(size_of::<TsAssemblyNode>(), 40);
    assert_eq!(size_of::<TsAssemblyProduct>(), 48);
    assert_eq!(size_of::<TsAssemblyRoot>(), 56);
    assert_eq!(std::mem::offset_of!(TsAssemblyRoot, error), 24);
    assert_eq!(size_of::<TsAssemblyExclusion>(), 48);
    assert_eq!(size_of::<TsStyleInfo>(), 96);
}

#[test]
fn c_abi_schema_selection_names_the_protocol() {
    // SAFETY: live handles and writable records; balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/assembly-nested.step"
        ));
        let mut info = TsSchemaInfo::default();
        assert_eq!(ts_document_get_schema(doc, &mut info), TS_OK);
        assert_eq!((info.protocol, info.schema_count), (TS_PROTOCOL_AP214, 1));
        assert_eq!(
            text(info.declared),
            "AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }"
        );
        assert_eq!(
            ts_document_get_schema(ptr::null(), &mut info),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(info.protocol, 0, "cleared on failure");
        ts_document_release(doc);
        let doc = parse(include_str!("../../../corpus/geometry/box.step"));
        assert_eq!(ts_document_get_schema(doc, &mut info), TS_OK);
        assert_eq!(info.protocol, TS_PROTOCOL_AP203);
        ts_document_release(doc);
    }
}

#[test]
fn c_abi_assembly_import_places_colours_and_reports() {
    // SAFETY: complete disjoint records, live handles, balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/assembly-nested.step"
        ));
        let mut options = TsAssemblyOptions::default();
        assert_eq!(ts_assembly_options_init(&mut options), TS_OK);
        let (mut assembly, mut error) = (ptr::null(), TsImportError::default());
        assert_eq!(
            ts_document_import_assembly(doc, &options, &mut assembly, &mut error),
            TS_OK
        );
        // The assembly is independent of the document.
        ts_document_release(doc);
        let mut info = TsAssemblyInfo::default();
        assert_eq!(ts_assembly_get_info(assembly, &mut info), TS_OK);
        assert_eq!(
            (
                info.node_count,
                info.product_count,
                info.occurrence_count,
                info.root_count,
                info.imported_root_count,
                info.complete
            ),
            (11, 3, 4, 1, 1, 1)
        );
        let mut names = Vec::new();
        for i in 0..info.product_count as usize {
            let mut product = TsAssemblyProduct::default();
            assert_eq!(ts_assembly_product_at(assembly, i, &mut product), TS_OK);
            names.push(text(product.name));
        }
        names.sort();
        assert_eq!(names, ["block", "pair", "rig"]);
        let mut root = TsAssemblyRoot::default();
        assert_eq!(ts_assembly_root_at(assembly, 0, &mut root), TS_OK);
        assert_eq!(
            (root.kind, root.selected, root.status),
            (TS_ROOT_FACETED_BREP, 1, TS_OK)
        );
        assert_eq!(ts_assembly_root_at(assembly, 1, &mut root), TS_NOT_FOUND);
        let mut leaves = 0;
        for i in 0..info.node_count as usize {
            let mut node = TsAssemblyNode::default();
            assert_eq!(ts_assembly_node_at(assembly, i, &mut node), TS_OK);
            leaves += usize::from(node.root_id != 0);
        }
        assert_eq!(leaves, 4);
        let mut scene = ptr::null();
        assert_eq!(ts_assembly_get_scene(assembly, &mut scene), TS_OK);
        let mut scene_info = TsSceneInfo::default();
        assert_eq!(ts_scene_get_info(scene, &mut scene_info), TS_OK);
        assert_eq!((scene_info.asset_count, scene_info.instance_count), (1, 11));
        let mut appearance = ptr::null();
        assert_eq!(ts_assembly_get_appearance(assembly, &mut appearance), TS_OK);
        let mut style = TsStyleInfo::default();
        assert_eq!(ts_assembly_get_style_info(assembly, &mut style), TS_OK);
        assert_eq!(style.styled_items, 0);
        ts_assembly_release(assembly);
        // Scene and appearance acquisitions outlive the assembly.
        assert_eq!(ts_scene_get_info(scene, &mut scene_info), TS_OK);
        ts_appearance_release(appearance);
        ts_scene_release(scene);

        let doc = parse(include_str!(
            "../../../corpus/geometry/assembly-styled.step"
        ));
        assert_eq!(
            ts_document_import_assembly(doc, ptr::null(), &mut assembly, &mut error),
            TS_OK
        );
        assert_eq!(ts_assembly_get_style_info(assembly, &mut style), TS_OK);
        assert_eq!(
            (style.styled_items, style.asset_styles, style.face_styles),
            (3, 1, 1)
        );
        assert_eq!(ts_assembly_get_appearance(assembly, &mut appearance), TS_OK);
        let mut a = TsAppearanceInfo::default();
        assert_eq!(ts_appearance_get_info(appearance, &mut a), TS_OK);
        assert_eq!((a.material_count, a.binding_count), (2, 2));
        ts_appearance_release(appearance);
        ts_assembly_release(assembly);
        ts_document_release(doc);

        // A missing placement fails at the product stage with its located entity.
        let doc = parse(include_str!(
            "../../../corpus/geometry/assembly-missing-placement.step"
        ));
        assert_eq!(
            ts_document_import_assembly(doc, ptr::null(), &mut assembly, &mut error),
            TS_INVALID_GEOMETRY
        );
        assert!(assembly.is_null());
        assert_eq!(error.stage, 5);
        assert_ne!(error.entity_id, 0);
        let bad = TsAssemblyOptions {
            chord_m: 0.,
            ..TsAssemblyOptions::default()
        };
        assert_eq!(
            ts_document_import_assembly(doc, &bad, &mut assembly, &mut error),
            TS_INVALID_ARGUMENT
        );
        ts_document_release(doc);
    }
}
