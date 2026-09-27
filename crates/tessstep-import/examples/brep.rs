//! Usage: cargo run -p tessstep-import --example brep -- FILE ROOT_ID METRES_PER_UNIT
//!     [--degrees] [--model-tolerance METRES] [--chord METRES] [--strict]
//! Imports one MANIFOLD_SOLID_BREP with the curved B-rep profile and reports
//! profile/geometry/tessellation stages independently as JSON. Plane angles are
//! radians unless --degrees is given; the model tolerance defaults to 1e-8 m.
#![forbid(unsafe_code)]
use std::{fs::File, io::BufReader};
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
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
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| {
        let found = args.iter().position(|a| a == name);
        found.map(|i| args.remove(i)).is_some()
    };
    let strict = flag("--strict");
    let degrees = flag("--degrees");
    let mut value = |name: &str, default: f64| -> Result<f64, Box<dyn std::error::Error>> {
        Ok(match args.iter().position(|a| a == name) {
            Some(i) => {
                let v = args.get(i + 1).ok_or("option needs a value")?.parse()?;
                args.drain(i..i + 2);
                v
            }
            None => default,
        })
    };
    let model_tolerance = value("--model-tolerance", 1e-8)?;
    let chord = value("--chord", 1e-6)?;
    if args.len() != 3 {
        return Err("usage: brep FILE ROOT_ID METRES_PER_UNIT [--degrees] [--model-tolerance METRES] [--chord METRES] [--strict]".into());
    }
    let root = EntityId::new(args[1].parse()?).ok_or("root must be nonzero")?;
    let unit = LengthUnit::metres_per_unit(args[2].parse()?)?;
    let angle = if degrees {
        AngleUnit::DEGREE
    } else {
        AngleUnit::RADIAN
    };
    let doc = tessstep_model::parse(
        BufReader::new(File::open(&args[0])?),
        ParseLimits::default(),
    )?;
    let model = ModelTolerance::new(Length::metres(model_tolerance)?, Angle::radians(1e-8)?)?;
    let tolerance = TessellationTolerance::new(Length::metres(chord)?, Angle::radians(0.1)?)?;
    let options = ImportOptions {
        strict,
        ..ImportOptions::default()
    };
    let imported = import_brep_solid(&doc, root, unit, angle, model, options);
    let adaptations = imported
        .as_ref()
        .map(|s| s.adaptations())
        .unwrap_or_default();
    let result =
        imported.and_then(|solid| solid.tessellate(tolerance, TessellationOptions::default()));
    let adapted = format!(
        "\"inferred_outer_bounds\":{},\"inserted_seams\":{},\"split_edges\":{},\"recharted_spheres\":{},\"collapsed_edges\":{},\"collapsed_faces\":{}",
        adaptations.inferred_outer_bounds,
        adaptations.inserted_seams,
        adaptations.split_edges,
        adaptations.recharted_spheres,
        adaptations.collapsed_edges,
        adaptations.collapsed_faces
    );
    match result {
        Ok(mesh) => {
            println!(
                "{{\"format_version\":1,\"scope\":\"brep-solid\",\"status\":\"accepted\",\"profile\":\"accepted\",\"geometry\":\"accepted\",\"tessellation\":\"accepted\",\"vertices\":{},\"triangles\":{},\"boundary_edges\":{},\"components\":{},\"volume_m3\":{},{adapted}}}",
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
                "{{\"format_version\":1,\"scope\":\"brep-solid\",\"status\":\"{status}\",\"profile\":\"{profile}\",\"geometry\":\"{geometry}\",\"tessellation\":\"{tessellation}\",\"entity\":{},{adapted}}}",
                e.entity.map_or(0, EntityId::get)
            );
            Ok(false)
        }
    }
}
