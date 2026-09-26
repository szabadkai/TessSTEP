//! Usage: cargo run -p tessstep-import --example presentation -- FILE ROOT_ID METRES_PER_UNIT [--strict]
//! Reports a selected tessellated annotation occurrence or geometric set as JSON:
//! profile and geometry stages, then graphics counts. Presentation graphics are not
//! meshes, so the tessellation stage is not applicable.
#![forbid(unsafe_code)]
use std::{fs::File, io::BufReader};
use tessstep_import::*;
use tessstep_math::LengthUnit;
use tessstep_part21::{EntityId, ParseLimits};
fn main() {
    match run() {
        Ok(accepted) => std::process::exit(i32::from(!accepted)),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let strict = args.len() == 5 && args[4] == "--strict";
    if args.len() != 4 && !strict {
        return Err("usage: presentation FILE ROOT_ID METRES_PER_UNIT [--strict]".into());
    }
    let root = EntityId::new(args[2].parse()?).ok_or("root must be nonzero")?;
    let unit = LengthUnit::metres_per_unit(args[3].parse()?)?;
    let doc = tessstep_model::parse(
        BufReader::new(File::open(&args[1])?),
        ParseLimits::default(),
    )?;
    match import_presentation(
        &doc,
        root,
        unit,
        ImportOptions {
            strict,
            ..ImportOptions::default()
        },
        PresentationLimits::default(),
    ) {
        Ok(p) => {
            let count = |kind| p.items().iter().filter(|i| i.kind == kind).count();
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for q in p.positions() {
                for k in 0..3 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
            }
            let bounds = if p.positions().is_empty() {
                "null".to_string()
            } else {
                format!("[{:?},{:?}]", lo, hi)
            };
            println!(
                "{{\"format_version\":1,\"scope\":\"presentation\",\"status\":\"accepted\",\"profile\":\"accepted\",\"geometry\":\"accepted\",\"tessellation\":\"not_applicable\",\"occurrence\":{},\"geometric_set\":{},\"styles\":{},\"curve_sets\":{},\"point_sets\":{},\"surface_sets\":{},\"repositioned_items\":{},\"vertices\":{},\"polylines\":{},\"polyline_points\":{},\"points\":{},\"triangles\":{},\"skipped_degenerate\":{},\"zero_area_triangles\":{},\"pnmax_deviations\":{},\"bounds_m\":{bounds}}}",
                p.occurrence().map_or(0, EntityId::get),
                p.geometric_set().get(),
                p.styles().len(),
                count(PresentationItemKind::CurveSet),
                count(PresentationItemKind::PointSet),
                count(PresentationItemKind::SurfaceSet),
                p.items().iter().filter(|i| i.placements > 0).count(),
                p.positions().len(),
                p.polyline_count(),
                p.polyline_points().len(),
                p.points().len(),
                p.triangles().len(),
                p.skipped_degenerate(),
                p.zero_area_triangles(),
                p.pnmax_deviations(),
            );
            Ok(true)
        }
        Err(e) => {
            let status = match e.kind {
                ErrorKind::Unsupported => "unsupported",
                ErrorKind::ResourceLimit => "resource_limit",
                _ => "rejected",
            };
            let (profile, geometry) = if e.stage == Stage::Profile {
                (status, "not_run")
            } else {
                ("accepted", status)
            };
            // Error detail is stderr, keeping bounded JSON free of unescaped strings.
            eprintln!("{e}");
            println!(
                "{{\"format_version\":1,\"scope\":\"presentation\",\"status\":\"{status}\",\"profile\":\"{profile}\",\"geometry\":\"{geometry}\",\"tessellation\":\"not_applicable\",\"entity\":{}}}",
                e.entity.map_or(0, EntityId::get)
            );
            Ok(false)
        }
    }
}
