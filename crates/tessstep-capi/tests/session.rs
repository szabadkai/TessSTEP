//! Milestone 22.10 second pass: repair and tolerance-policy revisions, surface models,
//! import sessions and assembly import with the extensible options.
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

fn options() -> TsImportOptions {
    let mut options = TsImportOptions::default();
    // SAFETY: a complete writable record of the current revision.
    assert_eq!(
        unsafe { ts_import_options_init(&mut options, size_of::<TsImportOptions>()) },
        TS_OK
    );
    options
}

/// Import a solid with a revision-2 info record; returns the status, the mesh's
/// signed volume and component count, the info and the error.
///
/// # Safety
/// `import` must follow the tessstep.h solid-import contract.
unsafe fn solid(
    import: impl FnOnce(*mut *const TsMesh, *mut TsSolidInfo, *mut TsImportError) -> u32,
) -> (u32, Option<(f64, u64)>, TsSolidInfo, TsImportError) {
    let mut info = TsSolidInfo {
        struct_size: TS_SOLID_INFO_SIZE_1,
        ..TsSolidInfo::default()
    };
    let (mut mesh, mut error) = (ptr::null(), TsImportError::default());
    let status = import(&mut mesh, &mut info, &mut error);
    if mesh.is_null() {
        return (status, None, info, error);
    }
    let mut view = TsMeshView::default();
    // SAFETY: a live mesh acquisition, released once.
    unsafe {
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        ts_mesh_release(mesh);
    }
    (
        status,
        Some((view.signed_volume, view.components)),
        info,
        error,
    )
}

/// # Safety
/// `doc` must be a live document.
unsafe fn import(
    doc: *const TsDocument,
    options: &TsImportOptions,
) -> (u32, Option<(f64, u64)>, TsSolidInfo, TsImportError) {
    // SAFETY: forwarded live document and disjoint outputs.
    unsafe {
        solid(|out, info, error| {
            ts_document_import_solid(doc, 1000, 0.001, 1., options, out, info, error)
        })
    }
}

#[test]
fn c_abi_second_pass_layouts() {
    use std::mem::offset_of;
    assert_eq!(size_of::<TsSurfaceModelInfo>(), 136);
    assert_eq!(offset_of!(TsSurfaceModelInfo, inferred_outer_bounds), 48);
    assert_eq!(size_of::<TsSurfaceShell>(), 48);
    assert_eq!(offset_of!(TsSurfaceShell, faces), 16);
    assert_eq!(size_of::<TsToleratedEncodings>(), 40);
    assert_eq!(size_of::<TsAssemblyAdaptations>(), 48);
    assert_eq!(size_of::<TsSessionStatistics>(), 72);
}

#[test]
fn c_abi_repair_is_opt_in_and_reported_in_the_second_info_revision() {
    // SAFETY: live documents, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/brep-repair-duplicated.step"
        ));
        // Strict rejection of topology defects stays the default.
        let (status, mesh, info, error) = import(doc, &options());
        assert_eq!(
            (status, error.stage, error.entity_id),
            (TS_INVALID_GEOMETRY, 3, 1000)
        );
        assert!(mesh.is_none() && info.merged_vertices == 0);
        let repair = TsImportOptions {
            flags: TS_IMPORT_REPAIR_SEW | TS_IMPORT_REPAIR_ORIENT,
            ..options()
        };
        let (status, mesh, info, _) = import(doc, &repair);
        assert_eq!(status, TS_OK);
        // Within the merge distance times the 10 x 20 x 30 mm box's area.
        let area = 2. * (200. + 300. + 600.) * 1e-6;
        assert!((mesh.unwrap().0 - 6e-6).abs() <= info.max_merge_distance_m * area);
        assert_eq!(
            (info.struct_size, info.abi_version),
            (TS_SOLID_INFO_SIZE_1, 1)
        );
        assert_eq!((info.merged_vertices, info.merged_edges), (1, 3));
        // Ten times the 1e-7 m model tolerance; the copied corner is 5e-8 m away and
        // the sewn B-rep is checked at the model tolerance plus that distance.
        assert!((info.sewing_tolerance_m - 1e-6).abs() < 1e-18);
        assert!((info.max_merge_distance_m - 5e-8).abs() < 1e-12);
        assert!((info.working_tolerance_m - (1e-7 + info.max_merge_distance_m)).abs() < 1e-18);
        // A tighter sewing factor names the distance it would need.
        let tight = TsImportOptions {
            sewing_factor: 0.1,
            ..repair
        };
        let (status, _, _, error) = import(doc, &tight);
        assert_eq!((status, error.stage), (TS_INVALID_GEOMETRY, 3));
        // An unpublished info size (the 104-byte draft) is rejected and not written.
        let mut draft = TsSolidInfo {
            struct_size: 104,
            merged_vertices: 77,
            ..TsSolidInfo::default()
        };
        let mut out = ptr::null();
        assert_eq!(
            ts_document_import_solid(
                doc,
                1000,
                0.001,
                1.,
                &repair,
                &mut out,
                &mut draft,
                ptr::null_mut()
            ),
            TS_INVALID_ARGUMENT
        );
        assert!(out.is_null() && draft.merged_vertices == 77 && draft.root_id == 0);
        ts_document_release(doc);

        // Orientation alone reverses an inside-out shell.
        let doc = parse(include_str!(
            "../../../corpus/geometry/brep-repair-inside-out.step"
        ));
        assert_eq!(import(doc, &options()).0, TS_INVALID_GEOMETRY);
        let orient = TsImportOptions {
            flags: TS_IMPORT_REPAIR_ORIENT,
            ..options()
        };
        let (status, mesh, info, _) = import(doc, &orient);
        assert_eq!(
            (status, info.reoriented_shells, info.merged_edges),
            (TS_OK, 1, 0)
        );
        assert!((mesh.unwrap().0 - 6e-6).abs() < 1e-13);
        assert_eq!(info.sewing_tolerance_m, 0.);
        ts_document_release(doc);
    }
}

#[test]
fn c_abi_tolerance_policy_counts_deviations_and_names_the_working_tolerance() {
    // SAFETY: live documents, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/brep-near-surface.step"
        ));
        // The fixture's 1 um uncertainty; the top circle lies 3 um off its cylinder.
        let near = TsImportOptions {
            model_distance_m: 1e-6,
            ..options()
        };
        let (status, mesh, info, _) = import(doc, &near);
        assert_eq!(status, TS_OK);
        assert_eq!(mesh.unwrap().1, 1);
        assert_eq!(
            (
                info.off_surface_edges,
                info.off_curve_vertices,
                info.inserted_seams
            ),
            (1, 0, 1)
        );
        assert!((info.largest_deviation_m - 3e-6).abs() < 1e-12);
        assert!((info.working_tolerance_m - (1e-6 + 2. * info.largest_deviation_m)).abs() < 1e-18);
        // The chord floors the accepted deviation: a factor of 2 (2 um) rejects the
        // 3 um edge under a 1 um chord and accepts it under the default 0.1 mm chord.
        let (status, _, info, error) = import(
            doc,
            &TsImportOptions {
                deviation_factor: 2.,
                chord_m: 1e-6,
                ..near
            },
        );
        assert_eq!((status, error.stage), (TS_INVALID_GEOMETRY, 2));
        assert_eq!(info.off_surface_edges, 0);
        let (status, _, info, _) = import(
            doc,
            &TsImportOptions {
                deviation_factor: 2.,
                ..near
            },
        );
        assert_eq!((status, info.off_surface_edges), (TS_OK, 1));
        // A factor of 1, or strict import, accepts no deviation.
        for o in [
            TsImportOptions {
                deviation_factor: 1.,
                ..near
            },
            TsImportOptions {
                flags: TS_IMPORT_STRICT,
                ..near
            },
        ] {
            let (status, _, info, error) = import(doc, &o);
            assert_eq!(
                (status, error.stage, info.off_surface_edges),
                (TS_INVALID_GEOMETRY, 2, 0)
            );
            assert_ne!(error.entity_id, 0);
        }
        ts_document_release(doc);
        let doc = parse(include_str!(
            "../../../corpus/geometry/brep-vertex-off-curve.step"
        ));
        let (status, _, info, _) = import(doc, &near);
        assert_eq!((status, info.off_curve_vertices), (TS_OK, 1));
        ts_document_release(doc);
    }
}

/// # Safety
/// The model must be live.
unsafe fn model_info(model: *const TsSurfaceModel) -> TsSurfaceModelInfo {
    let mut info = TsSurfaceModelInfo {
        struct_size: TS_SURFACE_MODEL_INFO_SIZE_1,
        ..TsSurfaceModelInfo::default()
    };
    // SAFETY: a live model and a complete record of its revision.
    assert_eq!(
        unsafe { ts_surface_model_get_info(model, &mut info) },
        TS_OK
    );
    info
}

#[test]
fn c_abi_surface_models_import_as_shell_meshes_never_solids() {
    // SAFETY: live handles, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/surface-open-patches.step"
        ));
        let (mut model, mut error) = (ptr::null(), TsImportError::default());
        assert_eq!(
            ts_document_import_surface_model(
                doc,
                1000,
                0.001,
                1.,
                ptr::null(),
                &mut model,
                &mut error
            ),
            TS_OK
        );
        let info = model_info(model);
        assert_eq!(
            (
                info.root_id,
                info.shell_count,
                info.closed_shell_count,
                info.face_count,
                info.boundary_edges
            ),
            (1000, 1, 0, 3, 8)
        );
        let mut shell = TsSurfaceShell::default();
        assert_eq!(ts_surface_model_shell_at(model, 0, &mut shell), TS_OK);
        assert_eq!(
            (shell.entity_id, shell.closed, shell.boundary_edges),
            (105, 0, 8)
        );
        let faces = slice::from_raw_parts(shell.faces, shell.face_count).to_vec();
        assert_eq!(faces.len(), 3);
        assert!(shell.working_tolerance_m > 0.);
        let mut mesh = ptr::null();
        assert_eq!(ts_surface_model_shell_mesh(model, 0, &mut mesh), TS_OK);
        let mut view = TsMeshView::default();
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        // One open component: boundary edges, never a solid.
        assert!(view.boundary_edges > 0 && view.components == 1);
        let ids = slice::from_raw_parts(view.face_ids, view.triangle_count);
        assert!(ids.iter().all(|id| faces.contains(id)));
        assert_eq!(
            ts_surface_model_shell_at(model, 1, &mut shell),
            TS_NOT_FOUND
        );
        assert!(shell.faces.is_null() && shell.entity_id == 0);
        let mut missing = ptr::dangling();
        assert_eq!(
            ts_surface_model_shell_mesh(model, 1, &mut missing),
            TS_NOT_FOUND
        );
        assert!(missing.is_null());
        assert_eq!(ts_surface_model_retain(model), TS_OK);
        ts_surface_model_release(model);
        ts_surface_model_release(model);
        // The mesh acquisition outlives the model and the document.
        ts_document_release(doc);
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        ts_mesh_release(mesh);

        // Closed shells are watertight shells, not solids: no volume check, and a
        // solid root is not a surface model.
        let doc = parse(include_str!(
            "../../../corpus/geometry/surface-closed-shells.step"
        ));
        assert_eq!(
            ts_document_import_surface_model(
                doc,
                1000,
                0.001,
                1.,
                ptr::null(),
                &mut model,
                &mut error
            ),
            TS_OK
        );
        let info = model_info(model);
        assert_eq!(
            (
                info.shell_count,
                info.closed_shell_count,
                info.boundary_edges,
                info.inserted_seams
            ),
            (2, 2, 0, 1)
        );
        for index in 0..2 {
            let mut mesh = ptr::null();
            assert_eq!(ts_surface_model_shell_mesh(model, index, &mut mesh), TS_OK);
            let mut view = TsMeshView::default();
            assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
            assert_eq!((view.boundary_edges, view.components), (0, 1));
            ts_mesh_release(mesh);
        }
        // Unknown info revisions are not written; NULL handles are invalid.
        let mut bad = TsSurfaceModelInfo {
            struct_size: 8,
            root_id: 5,
            ..TsSurfaceModelInfo::default()
        };
        assert_eq!(
            ts_surface_model_get_info(model, &mut bad),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(bad.root_id, 5);
        ts_surface_model_release(model);
        model = ptr::dangling();
        let cylinder = parse(include_str!("../../../corpus/geometry/brep-cylinder.step"));
        assert_eq!(
            ts_document_import_surface_model(
                cylinder,
                1000,
                0.001,
                1.,
                ptr::null(),
                &mut model,
                &mut error
            ),
            TS_UNSUPPORTED
        );
        assert!(model.is_null() && error.entity_id == 1000);
        ts_document_release(cylinder);

        // Discovery reports surface models only when asked, after every other root.
        let mut discovery = ptr::null();
        assert_eq!(
            ts_document_discover(
                doc,
                TS_PREFER_EXACT,
                ptr::null(),
                &mut discovery,
                &mut error
            ),
            TS_OK
        );
        let mut found = TsDiscoveryInfo::default();
        assert_eq!(ts_discovery_get_info(discovery, &mut found), TS_OK);
        assert_eq!(found.root_count, 0);
        ts_discovery_release(discovery);
        let surfaces = TsImportOptions {
            flags: TS_DISCOVER_SURFACE_MODELS,
            ..options()
        };
        assert_eq!(
            ts_document_discover(doc, TS_PREFER_EXACT, &surfaces, &mut discovery, &mut error),
            TS_OK
        );
        ts_document_release(doc);
        assert_eq!(ts_discovery_get_info(discovery, &mut found), TS_OK);
        assert_eq!(
            (
                found.root_count,
                found.solid_count,
                found.tessellation_count,
                found.choice_count
            ),
            (3, 0, 0, 0)
        );
        let mut root = TsDiscoveredRoot::default();
        assert_eq!(ts_discovery_root_at(discovery, 0, &mut root), TS_OK);
        assert_eq!(
            (
                root.root_id,
                root.kind,
                root.status,
                root.units.metres_per_unit
            ),
            (1000, TS_ROOT_SHELL_BASED_SURFACE_MODEL, TS_OK, 0.001)
        );
        ts_discovery_release(discovery);
    }
}

#[test]
fn c_abi_session_owns_its_document_and_shares_discovery_and_decodes() {
    // SAFETY: live handles, complete records, balanced releases; sentinel pointers
    // are only written.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/tessellated-selection.step"
        ));
        let mut session = ptr::null_mut();
        assert_eq!(
            ts_import_session_create(doc, ptr::null(), &mut session),
            TS_OK
        );
        // The session retains the document.
        ts_document_release(doc);
        let (mut discovery, mut error) = (ptr::null(), TsImportError::default());
        assert_eq!(
            ts_import_session_discover(session, TS_PREFER_EXACT, &mut discovery, &mut error),
            TS_OK
        );
        let mut info = TsDiscoveryInfo::default();
        assert_eq!(ts_discovery_get_info(discovery, &mut info), TS_OK);
        assert_eq!((info.root_count, info.choice_count), (5, 3));
        ts_discovery_release(discovery);
        // Discovery is cached: a second call runs only the new preference.
        assert_eq!(
            ts_import_session_discover(session, TS_PREFER_EXACT, &mut discovery, &mut error),
            TS_OK
        );
        ts_discovery_release(discovery);
        let mut statistics = TsSessionStatistics::default();
        assert_eq!(
            ts_import_session_get_statistics(session, &mut statistics),
            TS_OK
        );
        let discoveries = statistics.discoveries;
        assert_eq!(
            discoveries, 4,
            "solids, tessellations, exact choices, presentations"
        );

        let roots = [1000, 11000, 21000, 31000, 40002, 77];
        let mut shared = 99;
        assert_eq!(
            ts_import_session_prepare(session, roots.as_ptr(), roots.len(), &mut shared),
            TS_OK
        );
        assert_eq!(
            ts_import_session_get_statistics(session, &mut statistics),
            TS_OK
        );
        assert_eq!(shared as u64, statistics.shared_roots);
        assert_eq!(
            (shared, statistics.shared_decodes),
            (3, 1),
            "the three tessellations"
        );
        for root in [1000, 11000] {
            let (status, mesh, info, _) = solid(|out, info, error| {
                ts_import_session_import_solid(session, root, 0.001, 1., out, info, error)
            });
            assert_eq!((status, info.root_id), (TS_OK, root));
            assert!((mesh.unwrap().0 - 6e-6).abs() < 1e-13);
        }
        let (mut mesh, mut tessellated) = (ptr::null(), TsTessellatedInfo::default());
        assert_eq!(
            ts_import_session_import_tessellated(
                session,
                21000,
                0.001,
                &mut mesh,
                &mut tessellated,
                &mut error
            ),
            TS_OK
        );
        assert_eq!(
            (tessellated.kind, tessellated.link_id),
            (TS_TESSELLATED_SOLID, 1000)
        );
        ts_mesh_release(mesh);
        assert_eq!(
            ts_import_session_get_statistics(session, &mut statistics),
            TS_OK
        );
        assert_eq!(statistics.imports, 3);
        assert!(statistics.triangles >= 36);
        // Failures are the free functions': located and typed.
        let (status, _, _, located) = solid(|out, info, error| {
            ts_import_session_import_solid(session, 9999, 0.001, 1., out, info, error)
        });
        assert_eq!((status, located.entity_id), (TS_NOT_FOUND, 9999));
        let (status, _, _, _) = solid(|out, info, error| {
            ts_import_session_import_solid(session, 1000, 0., 1., out, info, error)
        });
        assert_eq!(status, TS_INVALID_ARGUMENT);
        assert_eq!(
            ts_import_session_prepare(session, ptr::null(), 1, &mut shared),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(shared, 0);
        let zero = [0];
        assert_eq!(
            ts_import_session_prepare(session, zero.as_ptr(), 1, &mut shared),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_import_session_prepare(session, ptr::null(), 0, &mut shared),
            TS_OK
        );
        discovery = ptr::dangling();
        assert_eq!(
            ts_import_session_discover(session, 0, &mut discovery, &mut error),
            TS_INVALID_ARGUMENT
        );
        assert!(discovery.is_null());
        ts_import_session_release(session);

        // A session imports an assembly; its options and budgets are the session's.
        let doc = parse(include_str!(
            "../../../corpus/geometry/assembly-nested.step"
        ));
        let limited = TsImportOptions {
            max_session_triangles: 0,
            ..options()
        };
        assert_eq!(ts_import_session_create(doc, &limited, &mut session), TS_OK);
        let mut assembly = ptr::null();
        assert_eq!(
            ts_import_session_import_assembly(session, TS_PREFER_EXACT, &mut assembly, &mut error),
            TS_OK
        );
        let mut assembly_info = TsAssemblyInfo::default();
        assert_eq!(ts_assembly_get_info(assembly, &mut assembly_info), TS_OK);
        // No triangle budget: the root is reported, not imported.
        assert_eq!(
            (assembly_info.root_count, assembly_info.imported_root_count),
            (1, 0)
        );
        let mut root = TsAssemblyRoot::default();
        assert_eq!(ts_assembly_root_at(assembly, 0, &mut root), TS_OK);
        assert_eq!((root.status, root.error.stage), (TS_RESOURCE_LIMIT, 4));
        ts_assembly_release(assembly);
        ts_import_session_release(session);
        assert_eq!(
            ts_import_session_create(doc, ptr::null(), &mut session),
            TS_OK
        );
        assert_eq!(
            ts_import_session_import_assembly(session, TS_PREFER_EXACT, &mut assembly, &mut error),
            TS_OK
        );
        assert_eq!(ts_assembly_get_info(assembly, &mut assembly_info), TS_OK);
        assert_eq!(
            (assembly_info.node_count, assembly_info.imported_root_count),
            (11, 1)
        );
        ts_assembly_release(assembly);
        ts_import_session_release(session);

        // Invalid arguments leave no session.
        let invalid = TsImportOptions {
            reserved: 1,
            ..options()
        };
        session = ptr::dangling_mut();
        assert_eq!(
            ts_import_session_create(doc, &invalid, &mut session),
            TS_INVALID_ARGUMENT
        );
        assert!(session.is_null());
        assert_eq!(
            ts_import_session_create(ptr::null(), ptr::null(), &mut session),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_import_session_get_statistics(ptr::null(), &mut statistics),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(statistics.imports, 0);
        ts_import_session_release(ptr::null_mut());
        ts_document_release(doc);
    }
}

#[test]
fn c_abi_assembly_import_with_options_reports_placements_and_encodings() {
    // SAFETY: live handles, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/assembly-mapped.step"
        ));
        let (mut assembly, mut error) = (ptr::null(), TsImportError::default());
        assert_eq!(
            ts_document_import_assembly_with_options(
                doc,
                TS_PREFER_EXACT,
                ptr::null(),
                &mut assembly,
                &mut error
            ),
            TS_OK
        );
        ts_document_release(doc);
        let mut info = TsAssemblyInfo::default();
        assert_eq!(ts_assembly_get_info(assembly, &mut info), TS_OK);
        assert_eq!((info.occurrence_count, info.complete), (4, 1));
        let mut adaptations = TsAssemblyAdaptations::default();
        assert_eq!(
            ts_assembly_get_adaptations(assembly, &mut adaptations),
            TS_OK
        );
        assert_eq!(adaptations.mapped_placements, 6);
        assert_eq!(adaptations.tolerated.untyped_measures, 0);
        ts_assembly_release(assembly);

        let doc = parse(include_str!("../../../corpus/geometry/assembly-inch.step"));
        assert_eq!(
            ts_document_import_assembly_with_options(
                doc,
                TS_PREFER_EXACT,
                ptr::null(),
                &mut assembly,
                &mut error
            ),
            TS_OK
        );
        assert_eq!(
            ts_assembly_get_adaptations(assembly, &mut adaptations),
            TS_OK
        );
        assert_eq!(
            (
                adaptations.mapped_placements,
                adaptations.tolerated.omitted_conversion_dimensions
            ),
            (0, 1)
        );
        ts_assembly_release(assembly);
        // The frozen-options entry point reports the same adaptations.
        assert_eq!(
            ts_document_import_assembly(doc, ptr::null(), &mut assembly, &mut error),
            TS_OK
        );
        assert_eq!(
            ts_assembly_get_adaptations(assembly, &mut adaptations),
            TS_OK
        );
        assert_eq!(adaptations.tolerated.omitted_conversion_dimensions, 1);
        ts_assembly_release(assembly);
        // Strict import rejects the starred units: no root, an incomplete structure.
        let strict = TsImportOptions {
            flags: TS_IMPORT_STRICT,
            ..options()
        };
        assert_eq!(
            ts_document_import_assembly_with_options(
                doc,
                TS_PREFER_EXACT,
                &strict,
                &mut assembly,
                &mut error
            ),
            TS_OK
        );
        assert_eq!(ts_assembly_get_info(assembly, &mut info), TS_OK);
        assert_eq!((info.imported_root_count, info.complete), (0, 0));
        ts_assembly_release(assembly);
        assembly = ptr::dangling();
        assert_eq!(
            ts_document_import_assembly_with_options(
                doc,
                3,
                ptr::null(),
                &mut assembly,
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        assert!(assembly.is_null());
        assert_eq!(
            ts_assembly_get_adaptations(ptr::null(), &mut adaptations),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(adaptations.mapped_placements, 0);
        ts_document_release(doc);
    }
}

/// The session's last message, read through a buffer of `capacity` bytes.
///
/// # Safety
/// `session` must be live.
unsafe fn message(session: *const TsImportSession, capacity: usize) -> (u32, String, usize) {
    let mut buffer = vec![0x7f_u8; capacity];
    let mut length = 99;
    // SAFETY: a buffer of capacity bytes and a writable length.
    let status = unsafe {
        ts_import_session_last_message(
            session,
            if capacity == 0 {
                ptr::null_mut()
            } else {
                buffer.as_mut_ptr().cast()
            },
            capacity,
            &mut length,
        )
    };
    let text = buffer
        .iter()
        .position(|&b| b == 0)
        .map(|n| String::from_utf8(buffer[..n].to_vec()).unwrap())
        .unwrap_or_default();
    (status, text, length)
}

#[test]
fn c_abi_session_keeps_the_last_failure_message() {
    // SAFETY: live handles, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/brep-near-surface.step"
        ));
        let strict = TsImportOptions {
            model_distance_m: 1e-6,
            deviation_factor: 1.,
            ..options()
        };
        let mut session = ptr::null_mut();
        assert_eq!(ts_import_session_create(doc, &strict, &mut session), TS_OK);
        ts_document_release(doc);
        assert_eq!(message(session, 64), (TS_OK, String::new(), 0));
        let (status, _, _, error) = solid(|out, info, error| {
            ts_import_session_import_solid(session, 1000, 0.001, 1., out, info, error)
        });
        assert_eq!((status, error.stage), (TS_INVALID_GEOMETRY, 2));
        // The measured distance and the bound, as Rust reports them.
        let (status, text, length) = message(session, 1024);
        assert_eq!((status, length), (TS_OK, text.len()));
        assert!(
            text.contains("lies 3.000e-6 m from the face surface")
                && text.contains("beyond the model tolerance 1.000e-6 m"),
            "{text}"
        );
        // A short buffer receives a NUL-terminated prefix and the full length.
        let (_, prefix, full) = message(session, 8);
        assert_eq!((prefix.as_str(), full), (&text[..7], length));
        assert_eq!(message(session, 0).2, length);
        // An argument error and a success each clear it.
        let (status, _, _, _) = solid(|out, info, error| {
            ts_import_session_import_solid(session, 1000, 0., 1., out, info, error)
        });
        assert_eq!(status, TS_INVALID_ARGUMENT);
        assert_eq!(message(session, 64).2, 0);
        let (status, _, _, _) = solid(|out, info, error| {
            ts_import_session_import_solid(session, 1000, 0.001, 1., out, info, error)
        });
        assert_eq!(status, TS_INVALID_GEOMETRY);
        let mut discovery = ptr::null();
        assert_eq!(
            ts_import_session_discover(session, TS_PREFER_EXACT, &mut discovery, ptr::null_mut()),
            TS_OK
        );
        ts_discovery_release(discovery);
        assert_eq!(message(session, 64).2, 0);
        let mut length = 0;
        assert_eq!(
            ts_import_session_last_message(session, ptr::null_mut(), 4, &mut length),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_import_session_last_message(ptr::null(), ptr::null_mut(), 0, &mut length),
            TS_INVALID_ARGUMENT
        );
        ts_import_session_release(session);

        // A budget failure names the budget and its configured value.
        let doc = parse(include_str!("../../../corpus/geometry/brep-cylinder.step"));
        let limited = TsImportOptions {
            max_work: 0,
            ..options()
        };
        assert_eq!(ts_import_session_create(doc, &limited, &mut session), TS_OK);
        ts_document_release(doc);
        let (status, _, _, _) = solid(|out, info, error| {
            ts_import_session_import_solid(session, 1000, 0.001, 1., out, info, error)
        });
        assert_eq!(status, TS_RESOURCE_LIMIT);
        let (_, text, _) = message(session, 1024);
        assert!(text.contains("(max_work 0)"), "{text}");
        ts_import_session_release(session);
    }
}

#[test]
fn c_abi_session_imports_presentations_and_surface_models() {
    // SAFETY: live handles, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!("../../../corpus/geometry/presentation.step"));
        let mut session = ptr::null_mut();
        assert_eq!(
            ts_import_session_create(doc, ptr::null(), &mut session),
            TS_OK
        );
        ts_document_release(doc);
        let (mut presentation, mut error) = (ptr::null(), TsImportError::default());
        assert_eq!(
            ts_import_session_import_presentation(
                session,
                300,
                0.001,
                &mut presentation,
                &mut error
            ),
            TS_OK
        );
        let mut info = TsPresentationInfo::default();
        assert_eq!(ts_presentation_get_info(presentation, &mut info), TS_OK);
        assert_eq!((info.occurrence_id, info.item_count), (300, 4));
        ts_presentation_release(presentation);
        assert_eq!(
            ts_import_session_import_presentation(
                session,
                110,
                0.001,
                &mut presentation,
                &mut error
            ),
            TS_UNSUPPORTED
        );
        assert!(presentation.is_null() && error.entity_id == 110);
        assert!(message(session, 256).2 > 0);
        ts_import_session_release(session);
        // The presentation limits of the options apply.
        let doc = parse(include_str!("../../../corpus/geometry/presentation.step"));
        let shallow = TsImportOptions {
            max_presentation_depth: 0,
            ..options()
        };
        assert_eq!(ts_import_session_create(doc, &shallow, &mut session), TS_OK);
        ts_document_release(doc);
        assert_eq!(
            ts_import_session_import_presentation(
                session,
                300,
                0.001,
                &mut presentation,
                &mut error
            ),
            TS_RESOURCE_LIMIT
        );
        ts_import_session_release(session);

        let doc = parse(include_str!(
            "../../../corpus/geometry/surface-open-patches.step"
        ));
        assert_eq!(
            ts_import_session_create(doc, ptr::null(), &mut session),
            TS_OK
        );
        ts_document_release(doc);
        let mut model = ptr::null();
        assert_eq!(
            ts_import_session_import_surface_model(
                session, 1000, 0.001, 1., &mut model, &mut error
            ),
            TS_OK
        );
        let mut shell = TsSurfaceShell::default();
        assert_eq!(ts_surface_model_shell_at(model, 0, &mut shell), TS_OK);
        assert_eq!((shell.entity_id, shell.boundary_edges), (105, 8));
        ts_surface_model_release(model);
        model = ptr::dangling();
        assert_eq!(
            ts_import_session_import_surface_model(
                session, 9999, 0.001, 1., &mut model, &mut error
            ),
            TS_NOT_FOUND
        );
        assert!(model.is_null());
        ts_import_session_release(session);
    }
}

#[test]
fn c_abi_root_reports_carry_tolerated_encodings_and_failure_text() {
    assert_eq!(size_of::<TsRootReport>(), 56);
    assert_eq!(std::mem::offset_of!(TsRootReport, message), 40);
    // SAFETY: live handles, complete records, balanced releases.
    unsafe {
        let text = |view: TsStringView| {
            if view.size == 0 {
                assert!(view.data.is_null());
                return String::new();
            }
            String::from_utf8(slice::from_raw_parts(view.data.cast::<u8>(), view.size).to_vec())
                .unwrap()
        };
        // The inch context writes a starred conversion-based unit: counted per root.
        let doc = parse(include_str!("../../../corpus/geometry/assembly-inch.step"));
        let (mut assembly, mut error) = (ptr::null(), TsImportError::default());
        assert_eq!(
            ts_document_import_assembly(doc, ptr::null(), &mut assembly, &mut error),
            TS_OK
        );
        let mut report = TsRootReport::default();
        assert_eq!(ts_assembly_root_report_at(assembly, 0, &mut report), TS_OK);
        assert_eq!(report.tolerated.omitted_conversion_dimensions, 1);
        assert_eq!(text(report.message), "");
        assert_eq!(
            ts_assembly_root_report_at(assembly, 9, &mut report),
            TS_NOT_FOUND
        );
        ts_assembly_release(assembly);
        let mut discovery = ptr::null();
        assert_eq!(
            ts_document_discover(
                doc,
                TS_PREFER_EXACT,
                ptr::null(),
                &mut discovery,
                &mut error
            ),
            TS_OK
        );
        assert_eq!(
            ts_discovery_root_report_at(discovery, 0, &mut report),
            TS_OK
        );
        assert_eq!(report.tolerated.omitted_conversion_dimensions, 1);
        ts_discovery_release(discovery);
        ts_document_release(doc);

        // A failed root carries its message, borrowed from the assembly.
        let doc = parse(include_str!(
            "../../../corpus/geometry/assembly-nested.step"
        ));
        let limited = TsImportOptions {
            max_session_triangles: 0,
            ..options()
        };
        assert_eq!(
            ts_document_import_assembly_with_options(
                doc,
                TS_PREFER_EXACT,
                &limited,
                &mut assembly,
                &mut error
            ),
            TS_OK
        );
        ts_document_release(doc);
        assert_eq!(ts_assembly_root_report_at(assembly, 0, &mut report), TS_OK);
        let message = text(report.message);
        assert!(message.contains("(max_triangles 0)"), "{message}");
        ts_assembly_release(assembly);

        // A root without units reports why.
        let doc = parse(include_str!(
            "../../../corpus/geometry/brep-sphere-cavity.step"
        ));
        assert_eq!(
            ts_document_discover(
                doc,
                TS_PREFER_EXACT,
                ptr::null(),
                &mut discovery,
                &mut error
            ),
            TS_OK
        );
        ts_document_release(doc);
        assert_eq!(
            ts_discovery_root_report_at(discovery, 0, &mut report),
            TS_OK
        );
        assert_eq!(
            text(report.message),
            "no supported representation contains the root"
        );
        assert_eq!(report.tolerated.omitted_conversion_dimensions, 0);
        ts_discovery_release(discovery);
        assert_eq!(
            ts_discovery_root_report_at(ptr::null(), 0, &mut report),
            TS_INVALID_ARGUMENT
        );
        assert!(report.message.data.is_null());
    }
}
