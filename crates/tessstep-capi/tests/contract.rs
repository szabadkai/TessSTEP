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

#[test]
fn c_abi_mesh_owned_buffers_limits_and_failures() {
    assert_eq!(size_of::<TsMeshOptions>(), 40);
    assert_eq!(size_of::<TsMeshView>(), 80);
    assert_eq!(std::mem::offset_of!(TsMeshView, face_ids), 48);
    assert_eq!(std::mem::offset_of!(TsMeshView, signed_volume), 72);
    let mut p = [0., 0., 0., 1., 0., 0., 0., 1., 0.];
    let t = [0, 1, 2];
    // SAFETY: valid input arrays and disjoint output slots, balanced mesh references.
    unsafe {
        let mut mesh = ptr::null();
        let mut view = TsMeshView::default();
        let mut options = TsMeshOptions::default();
        assert_eq!(ts_mesh_options_init(&mut options), TS_OK);
        assert_eq!(
            ts_mesh_create(p.as_ptr(), 3, t.as_ptr(), 1, &options, &mut mesh),
            TS_OK
        );
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        assert_eq!(view.vertex_count, 3);
        assert_eq!(view.triangle_count, 1);
        assert_eq!(view.boundary_edges, 3);
        p[0] = 7.;
        assert_eq!(*view.positions, 0.);
        assert_eq!(*view.normals.add(2), 1.);
        let address = view.positions;
        assert_eq!(ts_mesh_retain(mesh), TS_OK);
        ts_mesh_release(mesh);
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        assert_eq!(view.positions, address);
        ts_mesh_release(mesh);
        assert_eq!(
            ts_mesh_get_view(ptr::null(), &mut view),
            TS_INVALID_ARGUMENT
        );
        assert!(view.positions.is_null());
        options.require_solid = 1;
        p[0] = 0.;
        assert_eq!(
            ts_mesh_create(p.as_ptr(), 3, t.as_ptr(), 1, &options, &mut mesh),
            TS_INVALID_MESH
        );
        assert!(mesh.is_null());
        options.max_vertices = 0;
        assert_eq!(
            ts_mesh_create(p.as_ptr(), 3, t.as_ptr(), 1, &options, &mut mesh),
            TS_RESOURCE_LIMIT
        );
        assert!(mesh.is_null());
        assert_eq!(
            ts_mesh_create(ptr::null(), 0, ptr::null(), 0, ptr::null(), &mut mesh),
            TS_INVALID_MESH
        );
        assert_eq!(
            ts_mesh_create(ptr::null(), 1, t.as_ptr(), 1, ptr::null(), &mut mesh),
            TS_INVALID_ARGUMENT
        );
        options = TsMeshOptions::default();
        options.reserved = 1;
        assert_eq!(
            ts_mesh_create(p.as_ptr(), 3, t.as_ptr(), 1, &options, &mut mesh),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_mesh_get_view(ptr::null(), ptr::null_mut()),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(ts_mesh_retain(ptr::null()), TS_INVALID_ARGUMENT);
        ts_mesh_release(ptr::null());
    }
}

#[test]
fn c_abi_scene_ownership_layouts_and_failure_outputs() {
    assert_eq!(size_of::<TsSceneOptions>(), 40);
    assert_eq!(size_of::<TsSceneAsset>(), 16);
    assert_eq!(size_of::<TsSceneInstance>(), 120);
    assert_eq!(size_of::<TsSceneInstanceInfo>(), 232);
    assert_eq!(std::mem::offset_of!(TsSceneInstanceInfo, mirrored), 224);
    assert_eq!(size_of::<TsSceneInfo>(), 16);
    // SAFETY: valid aligned storage, library handles, and balanced acquisitions.
    unsafe {
        let p = [0., 0., 0., 1., 0., 0., 0., 1., 0.];
        let t = [0, 1, 2];
        let mut mesh = ptr::null();
        assert_eq!(
            ts_mesh_create(p.as_ptr(), 3, t.as_ptr(), 1, ptr::null(), &mut mesh),
            TS_OK
        );
        let assets = [TsSceneAsset { id: 9, mesh }];
        let mut nodes = [TsSceneInstance {
            id: 7,
            asset_id: 9,
            linear: [-1., 0., 0., 0., 1., 0., 0., 0., 1.],
            ..Default::default()
        }];
        let mut scene = ptr::null();
        assert_eq!(
            ts_scene_create(
                assets.as_ptr(),
                1,
                nodes.as_ptr(),
                1,
                ptr::null(),
                &mut scene
            ),
            TS_OK
        );
        ts_mesh_release(mesh);
        let mut acquired = ptr::null();
        assert_eq!(ts_scene_asset_mesh(scene, 9, &mut acquired), TS_OK);
        assert_eq!(acquired, mesh);
        assert_eq!(ts_scene_retain(scene), TS_OK);
        ts_scene_release(scene);
        let mut info = TsSceneInstanceInfo::default();
        assert_eq!(ts_scene_instance_at(scene, 0, &mut info), TS_OK);
        assert_eq!(info.mirrored, 1);
        let mut baked = ptr::null();
        assert_eq!(
            ts_scene_bake_instance(scene, 7, ptr::null(), &mut baked),
            TS_OK
        );
        let mut view = TsMeshView::default();
        assert_eq!(ts_mesh_get_view(baked, &mut view), TS_OK);
        assert_eq!(*view.positions.add(3), -1.);
        assert_eq!(*view.triangles.add(1), 2);
        ts_mesh_release(baked);
        let mut o = TsMeshOptions {
            max_work: 0,
            ..Default::default()
        };
        assert_eq!(
            ts_scene_bake_instance(scene, 7, &o, &mut baked),
            TS_RESOURCE_LIMIT
        );
        assert!(baked.is_null());
        o.abi_version = 99;
        assert_eq!(
            ts_scene_bake_instance(scene, 7, &o, &mut baked),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_scene_bake_instance(scene, 55, ptr::null(), &mut baked),
            TS_NOT_FOUND
        );
        ts_scene_release(scene);
        assert_eq!(ts_mesh_get_view(acquired, &mut view), TS_OK);
        assert_eq!(*view.positions.add(3), 1.);
        nodes[0].parent_id = 7;
        assert_eq!(
            ts_scene_create(
                assets.as_ptr(),
                1,
                nodes.as_ptr(),
                1,
                ptr::null(),
                &mut scene
            ),
            TS_INVALID_SCENE
        );
        assert!(scene.is_null());
        nodes[0].parent_id = 0;
        nodes[0].linear[0] = f64::NAN;
        assert_eq!(
            ts_scene_create(
                assets.as_ptr(),
                1,
                nodes.as_ptr(),
                1,
                ptr::null(),
                &mut scene
            ),
            TS_INVALID_SCENE
        );
        let mut opts = TsSceneOptions {
            max_assets: 0,
            ..Default::default()
        };
        assert_eq!(
            ts_scene_create(assets.as_ptr(), 1, nodes.as_ptr(), 1, &opts, &mut scene),
            TS_RESOURCE_LIMIT
        );
        opts.abi_version = 99;
        assert_eq!(
            ts_scene_create(ptr::null(), 0, ptr::null(), 0, &opts, &mut scene),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_scene_create(ptr::null(), 1, ptr::null(), 0, ptr::null(), &mut scene),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_scene_instance_at(ptr::null(), 0, &mut info),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(info.source.id, 0);
        assert_eq!(
            ts_scene_get_info(ptr::null(), ptr::null_mut()),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(ts_scene_retain(ptr::null()), TS_INVALID_ARGUMENT);
        ts_scene_release(ptr::null());
        ts_mesh_release(acquired);
    }
}

#[test]
fn c_abi_appearance_records_ownership_and_failure_outputs() {
    assert_eq!(size_of::<TsAppearanceOptions>(), 32);
    assert_eq!(size_of::<TsMaterial>(), 40);
    assert_eq!(size_of::<TsStyleTarget>(), 24);
    assert_eq!(size_of::<TsStyleBinding>(), 32);
    assert_eq!(size_of::<TsResolvedMaterial>(), 64);
    assert_eq!(std::mem::offset_of!(TsResolvedMaterial, source), 40);
    assert_eq!(size_of::<TsAppearanceInfo>(), 16);
    // SAFETY: readable aligned arrays and output storage, live library handles,
    // and balanced acquisitions including the retained scene and mesh chain.
    unsafe {
        let xyz = [0., 0., 0., 1., 0., 0., 0., 1., 0.];
        let tri = [0, 1, 2];
        let mut mesh = ptr::null();
        assert_eq!(
            ts_mesh_create(xyz.as_ptr(), 3, tri.as_ptr(), 1, ptr::null(), &mut mesh),
            TS_OK
        );
        let assets = [TsSceneAsset { id: 1, mesh }];
        let nodes = [TsSceneInstance {
            id: 1,
            asset_id: 1,
            linear: [1., 0., 0., 0., 1., 0., 0., 0., 1.],
            ..Default::default()
        }];
        let mut scene = ptr::null();
        assert_eq!(
            ts_scene_create(
                assets.as_ptr(),
                1,
                nodes.as_ptr(),
                1,
                ptr::null(),
                &mut scene
            ),
            TS_OK
        );
        ts_mesh_release(mesh);
        let mut materials = [TsMaterial {
            id: 42,
            rgba: [0.2, 0.3, 0.4, 0.],
        }];
        let mut bindings = [TsStyleBinding {
            target: TsStyleTarget {
                kind: 2,
                id: 1,
                face_id: 0,
                reserved: 0,
            },
            material_id: 42,
        }];
        let mut appearance = ptr::null();
        assert_eq!(
            ts_appearance_create(
                scene,
                materials.as_ptr(),
                1,
                bindings.as_ptr(),
                1,
                ptr::null(),
                &mut appearance
            ),
            TS_OK
        );
        let mut resolved = TsResolvedMaterial::default();
        assert_eq!(
            ts_appearance_resolve_triangle(appearance, 1, 0, &mut resolved),
            TS_OK
        );
        assert_eq!(resolved.material.id, 42);
        assert_eq!(resolved.material.rgba[3], 0.);
        assert_eq!(resolved.source.kind, 2);
        assert_eq!(resolved.source.face_id, 0);
        materials[0].rgba[3] = 1.;
        assert_eq!(
            ts_appearance_resolve_triangle(appearance, 1, 0, &mut resolved),
            TS_OK
        );
        assert_eq!(resolved.material.rgba[3], 0.);
        assert_eq!(ts_appearance_retain(appearance), TS_OK);
        ts_appearance_release(appearance);
        let mut info = TsAppearanceInfo::default();
        assert_eq!(ts_appearance_get_info(appearance, &mut info), TS_OK);
        assert_eq!(info.material_count, 1);
        let mut material = TsMaterial::default();
        assert_eq!(
            ts_appearance_material_at(appearance, 0, &mut material),
            TS_OK
        );
        assert_eq!(material.rgba[3], 0.);
        let mut binding = TsStyleBinding::default();
        assert_eq!(ts_appearance_binding_at(appearance, 0, &mut binding), TS_OK);
        assert_eq!(binding.material_id, 42);
        assert_eq!(
            ts_appearance_resolve_triangle(appearance, 1, 1, &mut resolved),
            TS_NOT_FOUND
        );
        assert_eq!(resolved.material.id, 0);
        assert_eq!(resolved.source.kind, 0);
        assert_eq!(
            ts_appearance_material_at(appearance, 99, &mut material),
            TS_NOT_FOUND
        );
        assert_eq!(material.id, 0);
        assert_eq!(
            ts_appearance_binding_at(appearance, 99, &mut binding),
            TS_NOT_FOUND
        );
        assert_eq!(binding.material_id, 0);
        let mut retained = ptr::null();
        ts_scene_release(scene);
        assert_eq!(ts_appearance_get_scene(appearance, &mut retained), TS_OK);
        assert_eq!(scene, retained);
        ts_appearance_release(appearance);
        assert_eq!(ts_scene_asset_mesh(retained, 1, &mut mesh), TS_OK);
        let mut view = TsMeshView::default();
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        assert_eq!(*view.positions.add(3), 1.);
        ts_mesh_release(mesh);
        let mut bad = ptr::null();
        for v in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            materials[0].rgba[0] = v;
            assert_eq!(
                ts_appearance_create(
                    retained,
                    materials.as_ptr(),
                    1,
                    bindings.as_ptr(),
                    1,
                    ptr::null(),
                    &mut bad
                ),
                TS_INVALID_APPEARANCE
            );
            assert!(bad.is_null());
        }
        materials[0].rgba[0] = 0.;
        bindings[0].target.reserved = 1;
        assert_eq!(
            ts_appearance_create(
                retained,
                materials.as_ptr(),
                1,
                bindings.as_ptr(),
                1,
                ptr::null(),
                &mut bad
            ),
            TS_INVALID_ARGUMENT
        );
        bindings[0].target.reserved = 0;
        bindings[0].target.kind = 99;
        assert_eq!(
            ts_appearance_create(
                retained,
                materials.as_ptr(),
                1,
                bindings.as_ptr(),
                1,
                ptr::null(),
                &mut bad
            ),
            TS_INVALID_ARGUMENT
        );
        bindings[0].target.kind = 2;
        bindings[0].target.face_id = 99;
        assert_eq!(
            ts_appearance_create(
                retained,
                materials.as_ptr(),
                1,
                bindings.as_ptr(),
                1,
                ptr::null(),
                &mut bad
            ),
            TS_INVALID_APPEARANCE
        );
        let mut options = TsAppearanceOptions {
            max_work: 0,
            ..Default::default()
        };
        assert_eq!(
            ts_appearance_create(
                retained,
                materials.as_ptr(),
                1,
                bindings.as_ptr(),
                1,
                &options,
                &mut bad
            ),
            TS_RESOURCE_LIMIT
        );
        options.abi_version = 99;
        assert_eq!(
            ts_appearance_create(retained, ptr::null(), 0, ptr::null(), 0, &options, &mut bad),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_appearance_create(
                retained,
                ptr::null(),
                1,
                ptr::null(),
                0,
                ptr::null(),
                &mut bad
            ),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_appearance_create(
                retained,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                &mut bad
            ),
            TS_OK
        );
        assert_eq!(
            ts_appearance_resolve_triangle(bad, 1, 0, &mut resolved),
            TS_OK
        );
        assert_eq!(resolved.material.id, 0);
        ts_appearance_release(bad);
        ts_scene_release(retained);
        assert_eq!(
            ts_appearance_create(
                ptr::null(),
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                &mut bad
            ),
            TS_INVALID_ARGUMENT
        );
        assert!(bad.is_null());
        assert_eq!(
            ts_appearance_resolve_triangle(ptr::null(), 1, 0, &mut resolved),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(ts_appearance_retain(ptr::null()), TS_INVALID_ARGUMENT);
        assert_eq!(
            ts_appearance_options_init(ptr::null_mut()),
            TS_INVALID_ARGUMENT
        );
        ts_appearance_release(ptr::null());
    }
}

#[test]
fn c_abi_faceted_import_ownership_errors_and_limits() {
    assert_eq!(size_of::<TsFacetedOptions>(), 80);
    assert_eq!(std::mem::offset_of!(TsFacetedOptions, max_work), 48);
    assert_eq!(size_of::<TsImportError>(), 32);
    // SAFETY: live handles, complete disjoint records and balanced releases.
    unsafe {
        let input = include_bytes!("../../../corpus/geometry/box.step");
        let mut doc = ptr::null();
        let mut report = ptr::null_mut();
        assert_eq!(
            ts_document_parse(
                input.as_ptr(),
                input.len(),
                ptr::null(),
                &mut doc,
                &mut report
            ),
            TS_OK
        );
        ts_diagnostics_release(report);
        let mut mesh = ptr::null();
        let mut error = TsImportError::default();
        assert_eq!(
            ts_document_tessellate_faceted(doc, 1000, 0.001, ptr::null(), &mut mesh, &mut error),
            TS_OK
        );
        let mut failed = mesh;
        for unit in [0., -1., f64::NAN, f64::INFINITY] {
            assert_eq!(
                ts_document_tessellate_faceted(
                    doc,
                    1000,
                    unit,
                    ptr::null(),
                    &mut failed,
                    &mut error
                ),
                TS_INVALID_ARGUMENT
            );
            assert!(failed.is_null());
            assert_eq!(error.stage, 0);
        }
        for options in [
            TsFacetedOptions {
                max_work: 0,
                ..TsFacetedOptions::default()
            },
            TsFacetedOptions {
                max_records: 0,
                ..TsFacetedOptions::default()
            },
            TsFacetedOptions {
                max_vertices: 0,
                ..TsFacetedOptions::default()
            },
            TsFacetedOptions {
                max_triangles: 0,
                ..TsFacetedOptions::default()
            },
        ] {
            assert_eq!(
                ts_document_tessellate_faceted(doc, 1000, 0.001, &options, &mut failed, &mut error),
                TS_RESOURCE_LIMIT
            );
            assert!(failed.is_null());
            assert_ne!(error.stage, 0);
        }
        let bad = TsFacetedOptions {
            abi_version: 2,
            ..TsFacetedOptions::default()
        };
        assert_eq!(
            ts_document_tessellate_faceted(doc, 1000, 0.001, &bad, &mut failed, &mut error),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(error.stage, 0);
        assert_eq!(
            ts_document_tessellate_faceted(doc, 99999, 0.001, ptr::null(), &mut failed, &mut error),
            TS_NOT_FOUND
        );
        assert_eq!(error.entity_id, 99999);
        ts_document_release(doc);
        let mut view = TsMeshView::default();
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        assert_eq!(
            (view.vertex_count, view.triangle_count, view.boundary_edges),
            (8, 12, 0)
        );
        assert!((view.signed_volume - 6e-6).abs() < 1e-15);
        assert_eq!(*view.face_ids, 106);
        ts_mesh_release(mesh);
    }
}

#[test]
fn c_abi_planar_import_owns_mesh_and_preserves_error_contract() {
    assert_eq!(size_of::<TsPlanarOptions>(), 80);
    // SAFETY: complete disjoint records, live handles, balanced releases.
    unsafe {
        let bytes = include_bytes!("../../../corpus/geometry/planar-box.step");
        let mut doc = ptr::null();
        let mut report = ptr::null_mut();
        assert_eq!(
            ts_document_parse(
                bytes.as_ptr(),
                bytes.len(),
                ptr::null(),
                &mut doc,
                &mut report
            ),
            TS_OK
        );
        ts_diagnostics_release(report);
        let mut options = TsPlanarOptions::default();
        assert_eq!(ts_planar_options_init(&mut options), TS_OK);
        let mut mesh = ptr::null();
        let mut error = TsImportError::default();
        assert_eq!(
            ts_document_tessellate_planar(doc, 1000, 0.001, &options, &mut mesh, &mut error),
            TS_OK
        );
        let mut failed = mesh;
        options.max_records = 0;
        assert_eq!(
            ts_document_tessellate_planar(doc, 1000, 0.001, &options, &mut failed, &mut error),
            TS_RESOURCE_LIMIT
        );
        assert!(failed.is_null());
        assert_ne!(error.stage, 0);
        assert_eq!(
            ts_document_tessellate_planar(doc, 1000, 0., ptr::null(), &mut failed, &mut error),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(error.stage, 0);
        assert_eq!(
            ts_document_tessellate_planar(
                ptr::null(),
                1000,
                1.,
                ptr::null(),
                &mut failed,
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        ts_document_release(doc);
        let mut view = TsMeshView::default();
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        assert_eq!(
            (view.vertex_count, view.triangle_count, view.boundary_edges),
            (8, 12, 0)
        );
        assert!((view.signed_volume - 6e-6).abs() < 1e-15);
        ts_mesh_release(mesh);
    }
}
