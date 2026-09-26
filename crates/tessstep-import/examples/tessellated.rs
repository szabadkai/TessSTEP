//! Usage: cargo run -p tessstep-import --example tessellated -- FILE ROOT_ID METRES_PER_UNIT
//! Reports selected-root profile and geometry/topology stages of an existing
//! tessellation as JSON. No tessellation is performed, so that stage is not applicable.
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
    if args.len() != 4 {
        return Err("usage: tessellated FILE ROOT_ID METRES_PER_UNIT".into());
    }
    let root = EntityId::new(args[2].parse()?).ok_or("root must be nonzero")?;
    let unit = LengthUnit::metres_per_unit(args[3].parse()?)?;
    let doc = tessstep_model::parse(
        BufReader::new(File::open(&args[1])?),
        ParseLimits::default(),
    )?;
    let result = import_tessellated(
        &doc,
        root,
        unit,
        ImportLimits::default(),
        tessstep_mesh::Limits::default(),
    );
    match result {
        Ok(imported) => {
            let mesh = imported.mesh();
            let kind = match imported.kind() {
                TessellatedKind::Solid => "solid",
                TessellatedKind::Shell => "shell",
                TessellatedKind::SurfaceSet => "surface_set",
            };
            println!(
                "{{\"format_version\":1,\"scope\":\"tessellated\",\"status\":\"accepted\",\"kind\":\"{kind}\",\"profile\":\"accepted\",\"geometry\":\"accepted\",\"tessellation\":\"not_applicable\",\"faces\":{},\"linked_faces\":{},\"skipped_degenerate\":{},\"vertices\":{},\"triangles\":{},\"boundary_edges\":{},\"components\":{},\"volume_m3\":{}}}",
                imported.faces().len(),
                imported
                    .faces()
                    .iter()
                    .filter(|f| f.geometric_link.is_some())
                    .count(),
                imported.skipped_degenerate(),
                mesh.data().positions.len(),
                mesh.data().triangles.len(),
                mesh.statistics().boundary_edges,
                mesh.statistics().components,
                mesh.statistics().signed_volume
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
                "{{\"format_version\":1,\"scope\":\"tessellated\",\"status\":\"{status}\",\"profile\":\"{profile}\",\"geometry\":\"{geometry}\",\"tessellation\":\"not_applicable\",\"entity\":{}}}",
                e.entity.map_or(0, EntityId::get)
            );
            Ok(false)
        }
    }
}
