//! Usage: cargo run -p tessstep-import --example faceted -- FILE ROOT_ID METRES_PER_UNIT
//! Reports selected-root profile/geometry/tessellation stages independently as JSON.
#![forbid(unsafe_code)]
use std::{fs::File, io::BufReader};
use tessstep_import::*;
use tessstep_math::{Angle, Length, LengthUnit, ModelTolerance};
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
        return Err("usage: faceted FILE ROOT_ID METRES_PER_UNIT".into());
    }
    let root = EntityId::new(args[2].parse()?).ok_or("root must be nonzero")?;
    let unit = LengthUnit::metres_per_unit(args[3].parse()?)?;
    let doc = tessstep_model::parse(
        BufReader::new(File::open(&args[1])?),
        ParseLimits::default(),
    )?;
    let model = ModelTolerance::new(Length::metres(1e-8)?, Angle::radians(1e-8)?)?;
    let tolerance = TessellationTolerance::new(Length::metres(1e-6)?, Angle::radians(0.1)?)?;
    let result = import_faceted_solid(&doc, root, unit, model, ImportOptions::default())
        .and_then(|solid| solid.tessellate(tolerance, TessellationOptions::default()));
    match result {
        Ok(mesh) => {
            println!(
                "{{\"format_version\":1,\"scope\":\"faceted-solid\",\"status\":\"accepted\",\"profile\":\"accepted\",\"geometry\":\"accepted\",\"tessellation\":\"accepted\",\"vertices\":{},\"triangles\":{},\"boundary_edges\":{},\"components\":{},\"volume_m3\":{}}}",
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
            let profile = if e.stage == Stage::Profile {
                status
            } else {
                "accepted"
            };
            let geometry = match e.stage {
                Stage::Profile => "not_run",
                Stage::Geometry | Stage::Topology => status,
                _ => "accepted",
            };
            let tessellation = if e.stage == Stage::Tessellation {
                status
            } else {
                "not_run"
            };
            // Error detail is stderr, keeping bounded JSON free of unescaped strings.
            eprintln!("{e}");
            println!(
                "{{\"format_version\":1,\"scope\":\"faceted-solid\",\"status\":\"{status}\",\"profile\":\"{profile}\",\"geometry\":\"{geometry}\",\"tessellation\":\"{tessellation}\",\"entity\":{}}}",
                e.entity.map_or(0, EntityId::get)
            );
            Ok(false)
        }
    }
}
