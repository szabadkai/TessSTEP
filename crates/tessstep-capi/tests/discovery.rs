use std::{f64::consts::PI, ptr, slice};
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
/// The array must be borrowed from live storage; NULL only with a zero count.
unsafe fn ids<T: Copy>(data: *const T, count: usize) -> Vec<T> {
    if count == 0 {
        assert!(data.is_null());
        return Vec::new();
    }
    // SAFETY: caller keeps the owner alive.
    unsafe { slice::from_raw_parts(data, count) }.to_vec()
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

fn info() -> TsSolidInfo {
    TsSolidInfo {
        struct_size: TS_SOLID_INFO_SIZE_1,
        ..TsSolidInfo::default()
    }
}

/// Import a solid root and return its status, mesh view statistics and info.
///
/// # Safety
/// `doc` must be a live document.
unsafe fn import(
    doc: *const TsDocument,
    root: u64,
    metres: f64,
    radians: f64,
    options: &TsImportOptions,
) -> (u32, Option<TsMeshView>, TsSolidInfo, TsImportError) {
    let (mut mesh, mut info, mut error) = (ptr::null(), info(), TsImportError::default());
    // SAFETY: live document, complete records and disjoint outputs.
    unsafe {
        let status = ts_document_import_solid(
            doc, root, metres, radians, options, &mut mesh, &mut info, &mut error,
        );
        if mesh.is_null() {
            return (status, None, info, error);
        }
        let mut view = TsMeshView::default();
        assert_eq!(ts_mesh_get_view(mesh, &mut view), TS_OK);
        // The view stays valid only while the mesh lives; copy the scalars we need.
        let statistics = TsMeshView {
            positions: ptr::null(),
            triangles: ptr::null(),
            normals: ptr::null(),
            uvs: ptr::null(),
            face_ids: ptr::null(),
            ..view
        };
        ts_mesh_release(mesh);
        (status, Some(statistics), info, error)
    }
}

#[test]
fn c_abi_milestone_22_layouts() {
    use std::mem::offset_of;
    assert_eq!(size_of::<TsImportOptions>(), 168);
    assert_eq!(offset_of!(TsImportOptions, flags), 8);
    assert_eq!(offset_of!(TsImportOptions, max_work), 16);
    assert_eq!(offset_of!(TsImportOptions, model_distance_m), 32);
    assert_eq!(offset_of!(TsImportOptions, chord_m), 48);
    assert_eq!(offset_of!(TsImportOptions, max_tessellation_work), 64);
    assert_eq!(offset_of!(TsImportOptions, max_triangles), 88);
    assert_eq!(offset_of!(TsImportOptions, sewing_factor), 96);
    assert_eq!(offset_of!(TsImportOptions, deviation_factor), 104);
    assert_eq!(offset_of!(TsImportOptions, max_assembly_instances), 112);
    assert_eq!(
        offset_of!(TsImportOptions, max_session_decoded_entities),
        128
    );
    assert_eq!(offset_of!(TsImportOptions, max_session_triangles), 144);
    assert_eq!(offset_of!(TsImportOptions, max_presentation_depth), 160);
    assert_eq!(size_of::<TsSolidInfo>(), 216);
    assert_eq!(offset_of!(TsSolidInfo, off_surface_edges), 104);
    assert_eq!(offset_of!(TsSolidInfo, largest_deviation_m), 128);
    assert_eq!(offset_of!(TsSolidInfo, merged_vertices), 144);
    assert_eq!(offset_of!(TsSolidInfo, sewing_tolerance_m), 208);
    assert_eq!(offset_of!(TsSolidInfo, root_id), 16);
    assert_eq!(offset_of!(TsSolidInfo, inferred_outer_bounds), 40);
    assert_eq!(offset_of!(TsSolidInfo, rejected_pcurves), 96);
    assert_eq!(size_of::<TsDiscoveryInfo>(), 48);
    assert_eq!(size_of::<TsContextUnits>(), 40);
    assert_eq!(size_of::<TsDiscoveredRoot>(), 112);
    assert_eq!(offset_of!(TsDiscoveredRoot, units), 40);
    assert_eq!(offset_of!(TsDiscoveredRoot, error), 80);
    assert_eq!(size_of::<TsDiscoveredPresentation>(), 120);
    assert_eq!(offset_of!(TsDiscoveredPresentation, units), 48);
    assert_eq!(offset_of!(TsDiscoveredPresentation, error), 88);
    assert_eq!(size_of::<TsRepresentationChoice>(), 32);
    assert_eq!(offset_of!(TsRepresentationChoice, alternatives), 16);
}

#[test]
fn c_abi_import_options_revisions_defaults_and_document_sizing() {
    let o = options();
    assert_eq!((o.struct_size, o.abi_version, o.flags), (168, 1, 0));
    assert_eq!((o.max_work, o.max_records), (5_000_000, 100_000));
    assert_eq!(
        (o.model_distance_m, o.chord_m, o.normal_angle_rad),
        (1e-7, 1e-4, 0.1)
    );
    assert_eq!(
        (o.max_tessellation_work, o.max_evaluations),
        (50_000_000, 10_000_000)
    );
    assert_eq!((o.max_vertices, o.max_triangles), (1_000_000, 2_000_000));
    assert_eq!((o.sewing_factor, o.deviation_factor), (10., 16.));
    assert_eq!(
        (o.max_assembly_instances, o.max_assembly_depth),
        (100_000, 1024)
    );
    assert_eq!(
        (o.max_polyline_points, o.max_presentation_depth),
        (4_000_000, 16)
    );
    assert_eq!(
        (
            o.max_session_decoded_entities,
            o.max_session_adapted_entities,
            o.max_session_triangles
        ),
        (1_000_000, 1_000_000, 20_000_000)
    );
    // SAFETY: writable records, live documents, balanced releases.
    unsafe {
        // Unknown revision sizes, smaller or larger, write nothing; 96 and 152 were
        // unreleased drafts of the record and are not revisions.
        for size in [0, 4, 95, 96, 97, 104, 152, 160, 176] {
            let mut untouched = TsImportOptions {
                max_work: 7,
                ..TsImportOptions::default()
            };
            assert_eq!(
                ts_import_options_init(&mut untouched, size),
                TS_INVALID_ARGUMENT
            );
            assert_eq!((untouched.struct_size, untouched.max_work), (168, 7));
        }
        // A record naming an unpublished size is rejected by every entry point.
        let doc = parse(include_str!("../../../corpus/geometry/brep-cylinder.step"));
        for size in [96, 152] {
            let draft = TsImportOptions {
                struct_size: size,
                ..options()
            };
            assert_eq!(import(doc, 1000, 0.001, 1., &draft).0, TS_INVALID_ARGUMENT);
        }
        ts_document_release(doc);
        assert_eq!(
            ts_import_options_init(ptr::null_mut(), 168),
            TS_INVALID_ARGUMENT
        );
        // Sized from the document, at least 512 work units and a record per entity.
        let doc = parse(include_str!("../../../corpus/geometry/brep-cylinder.step"));
        let mut sized = TsImportOptions::default();
        assert_eq!(ts_import_options_for_document(doc, &mut sized, 168), TS_OK);
        assert_eq!((sized.max_work, sized.max_records), (5_000_000, 100_000));
        assert_eq!((sized.struct_size, sized.chord_m), (168, 1e-4));
        assert_eq!(sized.max_session_triangles, 20_000_000);
        ts_document_release(doc);
        let big = (0..20_000)
            .map(|i| format!("#{}=CARTESIAN_POINT('',(0.,0.,{i}.));", i + 1))
            .collect::<String>();
        let text = format!(
            "ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');\
             FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));ENDSEC;DATA;{big}ENDSEC;END-ISO-10303-21;"
        );
        let doc = parse(&text);
        assert_eq!(ts_import_options_for_document(doc, &mut sized, 168), TS_OK);
        assert_eq!((sized.max_work, sized.max_records), (20_000 * 512, 100_000));
        // 256 triangles per entity stays below the 20M floor at 20,000 entities.
        assert_eq!(sized.max_session_decoded_entities, 1_000_000);
        ts_document_release(doc);
        assert_eq!(
            ts_import_options_for_document(ptr::null(), &mut sized, 168),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            (sized.struct_size, sized.max_work, sized.chord_m),
            (0, 0, 0.)
        );
    }
}

#[test]
fn c_abi_solid_import_reports_curved_adaptations_units_and_cavities() {
    // SAFETY: live documents, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!("../../../corpus/geometry/brep-cylinder.step"));
        let mut o = options();
        o.chord_m = 1e-6;
        let (status, view, info, error) = import(doc, 1000, 0.001, 1., &o);
        assert_eq!((status, error.stage), (TS_OK, 0));
        let view = view.unwrap();
        assert_eq!(
            (view.vertex_count, view.boundary_edges, view.components),
            (512, 0, 1)
        );
        let exact = PI * 100. * 20. * 1e-9;
        assert!(((view.signed_volume - exact) / exact).abs() <= 2. * 1e-6 / 0.01);
        assert_eq!(
            (info.struct_size, info.abi_version),
            (TS_SOLID_INFO_SIZE_1, 1)
        );
        assert_eq!(
            (info.kind, info.root_id, info.face_count, info.shell_count),
            (TS_ROOT_MANIFOLD_SOLID_BREP, 1000, 3, 1)
        );
        assert_eq!((info.inserted_seams, info.split_edges), (1, 0));
        // Budgets: zero means zero, and the failure names its stage.
        o.max_work = 0;
        let (status, view, info, error) = import(doc, 1000, 0.001, 1., &o);
        assert_eq!((status, error.stage), (TS_RESOURCE_LIMIT, 1));
        assert!(view.is_none());
        assert_eq!((info.struct_size, info.abi_version, info.kind), (216, 1, 0));
        o = options();
        o.max_vertices = 0;
        let (status, _, _, error) = import(doc, 1000, 0.001, 1., &o);
        assert_eq!((status, error.stage), (TS_RESOURCE_LIMIT, 4));
        ts_document_release(doc);

        // The plane-angle unit scales the cone semi-angle: 30 in degrees.
        let doc = parse(include_str!("../../../corpus/geometry/brep-cone.step"));
        let (status, view, info, _) = import(doc, 1000, 0.001, PI / 180., &options());
        assert_eq!((status, view.unwrap().boundary_edges), (TS_OK, 0));
        assert_eq!(info.inserted_seams, 1);
        let (status, view, _, error) = import(doc, 1000, 0.001, 1., &options());
        assert_ne!(status, TS_OK);
        assert!(view.is_none() && error.stage == 2 && error.entity_id != 0);
        ts_document_release(doc);

        // A BREP_WITH_VOIDS cavity meshes as a second, inward component.
        let doc = parse(include_str!(
            "../../../corpus/geometry/brep-sphere-cavity.step"
        ));
        let (status, view, info, _) = import(doc, 1000, 0.001, 1., &options());
        assert_eq!(status, TS_OK);
        assert_eq!((info.kind, info.shell_count), (TS_ROOT_BREP_WITH_VOIDS, 2));
        assert_eq!(view.unwrap().components, 2);
        ts_document_release(doc);

        // A FACETED_BREP root uses the faceted profile and reports no adaptations.
        let doc = parse(include_str!("../../../corpus/geometry/box.step"));
        let (status, view, info, _) = import(doc, 1000, 0.001, 1., &options());
        assert_eq!(status, TS_OK);
        let view = view.unwrap();
        assert_eq!((view.vertex_count, view.triangle_count), (8, 12));
        assert!((view.signed_volume - 6e-6).abs() < 1e-17);
        assert_eq!(
            (info.kind, info.face_count, info.inserted_seams),
            (TS_ROOT_FACETED_BREP, 6, 0)
        );
        let (status, _, _, error) = import(doc, 9999, 0.001, 1., &options());
        assert_eq!((status, error.entity_id), (TS_NOT_FOUND, 9999));
        ts_document_release(doc);

        // The strict flag rejects $ in derived ORIENTED_EDGE endpoint slots.
        let doc = parse(include_str!(
            "../../../corpus/geometry/planar-placeholder.step"
        ));
        assert_eq!(import(doc, 1000, 0.001, 1., &options()).0, TS_OK);
        let mut strict = options();
        strict.flags = TS_IMPORT_STRICT;
        let (status, _, _, error) = import(doc, 1000, 0.001, 1., &strict);
        assert_eq!((status, error.stage), (TS_INVALID_GEOMETRY, 1));
        ts_document_release(doc);
    }
}

#[test]
fn c_abi_solid_import_validates_every_argument_and_clears_outputs() {
    // SAFETY: live documents, complete records; sentinel pointers are only written.
    unsafe {
        let doc = parse(include_str!("../../../corpus/geometry/brep-cylinder.step"));
        let invalid = [
            TsImportOptions {
                flags: 32,
                ..options()
            },
            TsImportOptions {
                reserved: 1,
                ..options()
            },
            TsImportOptions {
                abi_version: 2,
                ..options()
            },
            TsImportOptions {
                struct_size: 88,
                ..options()
            },
            TsImportOptions {
                chord_m: 0.,
                ..options()
            },
            TsImportOptions {
                model_distance_m: f64::NAN,
                ..options()
            },
            TsImportOptions {
                normal_angle_rad: 4.,
                ..options()
            },
            TsImportOptions {
                flags: 16,
                ..options()
            },
            TsImportOptions {
                deviation_factor: 0.5,
                ..options()
            },
            TsImportOptions {
                deviation_factor: f64::INFINITY,
                ..options()
            },
            TsImportOptions {
                sewing_factor: 0.,
                ..options()
            },
            TsImportOptions {
                struct_size: 104,
                ..options()
            },
        ];
        for o in &invalid {
            let (status, view, info, error) = import(doc, 1000, 0.001, 1., o);
            assert_eq!((status, error.stage), (TS_INVALID_ARGUMENT, 0));
            assert!(view.is_none() && info.kind == 0);
        }
        for (metres, radians) in [(0., 1.), (0.001, 0.), (f64::INFINITY, 1.), (0.001, -1.)] {
            assert_eq!(
                import(doc, 1000, metres, radians, &options()).0,
                TS_INVALID_ARGUMENT
            );
        }
        assert_eq!(import(doc, 0, 0.001, 1., &options()).0, TS_INVALID_ARGUMENT);
        let mut mesh = ptr::dangling();
        let mut error = TsImportError {
            stage: 9,
            ..TsImportError::default()
        };
        // An unknown info revision is rejected without writing it.
        let mut bad = TsSolidInfo {
            struct_size: 96,
            root_id: 5,
            ..TsSolidInfo::default()
        };
        assert_eq!(
            ts_document_import_solid(
                doc,
                1000,
                0.001,
                1.,
                ptr::null(),
                &mut mesh,
                &mut bad,
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        assert!(mesh.is_null() && error.stage == 0 && bad.root_id == 5);
        // NULL options and info select the defaults.
        assert_eq!(
            ts_document_import_solid(
                doc,
                1000,
                0.001,
                1.,
                ptr::null(),
                &mut mesh,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            TS_OK
        );
        ts_mesh_release(mesh);
        assert_eq!(
            ts_document_import_solid(
                ptr::null(),
                1000,
                0.001,
                1.,
                ptr::null(),
                &mut mesh,
                ptr::null_mut(),
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        assert!(mesh.is_null());
        assert_eq!(
            ts_document_import_solid(
                doc,
                1000,
                0.001,
                1.,
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        ts_document_release(doc);
    }
}

#[test]
fn c_abi_discovery_selects_representations_and_reports_units() {
    // SAFETY: live handles, complete records, balanced releases.
    unsafe {
        let doc = parse(include_str!(
            "../../../corpus/geometry/tessellated-selection.step"
        ));
        let (mut discovery, mut error) = (ptr::null(), TsImportError::default());
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
        let mut info = TsDiscoveryInfo::default();
        assert_eq!(ts_discovery_get_info(discovery, &mut info), TS_OK);
        assert_eq!(
            (
                info.root_count,
                info.solid_count,
                info.tessellation_count,
                info.presentation_count,
                info.choice_count,
                info.preference
            ),
            (5, 2, 3, 0, 3, TS_PREFER_EXACT)
        );
        let mut roots = Vec::new();
        for i in 0..info.root_count as usize {
            let mut root = TsDiscoveredRoot::default();
            assert_eq!(ts_discovery_root_at(discovery, i, &mut root), TS_OK);
            assert_eq!(root.status, TS_OK);
            assert_eq!(
                (root.units.metres_per_unit, root.units.radians_per_unit),
                (0.001, 1.)
            );
            assert_eq!(root.units.has_distance_uncertainty, 1);
            assert!((root.units.distance_uncertainty_m - 1e-8).abs() < 1e-22);
            assert_eq!(
                ids(root.representations, root.representation_count).len(),
                1
            );
            roots.push((root.root_id, root.kind, root.link_id));
        }
        assert_eq!(
            roots,
            [
                (1000, TS_ROOT_MANIFOLD_SOLID_BREP, 0),
                (11000, TS_ROOT_FACETED_BREP, 0),
                (21000, TS_ROOT_TESSELLATED_SOLID, 1000),
                (31000, TS_ROOT_TESSELLATED_SHELL, 30021),
                (40002, TS_ROOT_TESSELLATED_SURFACE_SET, 0),
            ]
        );
        let choices = |discovery| {
            (0..3)
                .map(|i| {
                    let mut c = TsRepresentationChoice::default();
                    assert_eq!(ts_discovery_choice_at(discovery, i, &mut c), TS_OK);
                    (
                        c.selected,
                        ids(c.alternatives, c.alternative_count),
                        c.pairing,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            choices(discovery),
            [
                (0, vec![2], TS_PAIRING_LINK),
                (1, vec![3], TS_PAIRING_RELATIONSHIP),
                (4, vec![], TS_PAIRING_NONE),
            ]
        );
        let mut c = TsRepresentationChoice::default();
        assert_eq!(ts_discovery_choice_at(discovery, 3, &mut c), TS_NOT_FOUND);
        let mut root = TsDiscoveredRoot::default();
        assert_eq!(ts_discovery_root_at(discovery, 5, &mut root), TS_NOT_FOUND);
        assert!(root.representations.is_null() && root.root_id == 0);
        let mut p = TsDiscoveredPresentation::default();
        assert_eq!(
            ts_discovery_presentation_at(discovery, 0, &mut p),
            TS_NOT_FOUND
        );

        // Both selected solids import with their discovered units after the
        // document-independent discovery outlives a retain/release pair.
        assert_eq!(ts_discovery_retain(discovery), TS_OK);
        ts_discovery_release(discovery);
        for index in [0, 1] {
            assert_eq!(ts_discovery_root_at(discovery, index, &mut root), TS_OK);
            let (status, view, info, _) = import(
                doc,
                root.root_id,
                root.units.metres_per_unit,
                root.units.radians_per_unit,
                &options(),
            );
            assert_eq!((status, info.kind), (TS_OK, root.kind));
            assert!((view.unwrap().signed_volume - 6e-6).abs() < 1e-17);
        }
        ts_discovery_release(discovery);

        let mut tessellated = ptr::null();
        assert_eq!(
            ts_document_discover(
                doc,
                TS_PREFER_TESSELLATED,
                ptr::null(),
                &mut tessellated,
                ptr::null_mut()
            ),
            TS_OK
        );
        ts_document_release(doc);
        assert_eq!(
            choices(tessellated),
            [
                (2, vec![0], TS_PAIRING_LINK),
                (3, vec![1], TS_PAIRING_RELATIONSHIP),
                (4, vec![], TS_PAIRING_NONE),
            ]
        );
        assert_eq!(ts_discovery_get_info(tessellated, &mut info), TS_OK);
        assert_eq!(info.preference, TS_PREFER_TESSELLATED);
        ts_discovery_release(tessellated);
    }
}

#[test]
fn c_abi_discovery_reports_presentations_unit_errors_and_budgets() {
    // SAFETY: live handles, complete records, balanced releases; sentinel pointers
    // are only written.
    unsafe {
        let doc = parse(include_str!("../../../corpus/geometry/presentation.step"));
        let (mut discovery, mut error) = (ptr::null(), TsImportError::default());
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
        let mut p = TsDiscoveredPresentation::default();
        assert_eq!(ts_discovery_presentation_at(discovery, 0, &mut p), TS_OK);
        assert_eq!((p.occurrence_id, p.status), (300, TS_OK));
        assert_eq!(ids(p.containers, p.container_count), [310]);
        assert_eq!(ids(p.representations, p.representation_count), [320, 321]);
        assert_eq!(p.units.metres_per_unit, 0.001);
        assert_eq!(ts_discovery_presentation_at(discovery, 1, &mut p), TS_OK);
        assert_eq!((p.occurrence_id, p.status), (301, TS_UNSUPPORTED));
        assert!(p.containers.is_null() && p.representations.is_null());
        assert_eq!((p.error.stage, p.error.entity_id), (1, 301));
        assert_eq!(p.units.metres_per_unit, 0.);
        ts_discovery_release(discovery);

        // A budget failure is the whole call's; invalid preferences and options fail
        // after clearing the outputs.
        let limited = TsImportOptions {
            max_work: 10,
            ..options()
        };
        discovery = ptr::dangling();
        assert_eq!(
            ts_document_discover(doc, TS_PREFER_EXACT, &limited, &mut discovery, &mut error),
            TS_RESOURCE_LIMIT
        );
        assert!(discovery.is_null() && error.stage == 1);
        for preference in [0, 3] {
            assert_eq!(
                ts_document_discover(doc, preference, ptr::null(), &mut discovery, &mut error),
                TS_INVALID_ARGUMENT
            );
        }
        let invalid = TsImportOptions {
            flags: 16,
            ..options()
        };
        assert_eq!(
            ts_document_discover(doc, TS_PREFER_EXACT, &invalid, &mut discovery, &mut error),
            TS_INVALID_ARGUMENT
        );
        assert!(discovery.is_null() && error.stage == 0);
        assert_eq!(
            ts_document_discover(
                ptr::null(),
                TS_PREFER_EXACT,
                ptr::null(),
                &mut discovery,
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(
            ts_document_discover(
                doc,
                TS_PREFER_EXACT,
                ptr::null(),
                ptr::null_mut(),
                &mut error
            ),
            TS_INVALID_ARGUMENT
        );
        ts_document_release(doc);

        // A root outside every representation has a located unit error, not a default.
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
        let mut root = TsDiscoveredRoot::default();
        assert_eq!(ts_discovery_root_at(discovery, 0, &mut root), TS_OK);
        assert_eq!(
            (root.root_id, root.kind, root.status),
            (1000, TS_ROOT_BREP_WITH_VOIDS, TS_UNSUPPORTED)
        );
        assert_eq!(
            (
                root.error.stage,
                root.error.entity_id,
                root.representation_count
            ),
            (1, 1000, 0)
        );
        assert!(root.error.end_offset > root.error.start_offset);
        ts_discovery_release(discovery);

        let mut info = TsDiscoveryInfo::default();
        assert_eq!(
            ts_discovery_get_info(ptr::null(), &mut info),
            TS_INVALID_ARGUMENT
        );
        assert_eq!(ts_discovery_retain(ptr::null()), TS_INVALID_ARGUMENT);
        ts_discovery_release(ptr::null());
    }
}
