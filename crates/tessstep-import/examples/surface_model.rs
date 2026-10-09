//! Usage: cargo run -p tessstep-import --example surface_model -- FILE ROOT_ID METRES_PER_UNIT
//!     [--degrees] [--model-tolerance METRES] [--chord METRES] [--normal-angle RADIANS] [--strict]
//! Imports one SHELL_BASED_SURFACE_MODEL with the curved B-rep profile, meshes each of
//! its shells as a shell (never as a solid) and reports profile/geometry/tessellation
//! stages and per-shell results as JSON. Plane angles are radians unless --degrees is
//! given; the model tolerance defaults to 1e-8 m, the chord to 1e-6 m and the normal
//! angle to 0.1 rad.
#![forbid(unsafe_code)]
use std::{fmt::Write, fs::File, io::BufReader};
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
    let normal = value("--normal-angle", 0.1)?;
    if args.len() != 3 {
        return Err("usage: surface_model FILE ROOT_ID METRES_PER_UNIT [--degrees] [--model-tolerance METRES] [--chord METRES] [--normal-angle RADIANS] [--strict]".into());
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
    let tolerance = TessellationTolerance::new(Length::metres(chord)?, Angle::radians(normal)?)?;
    let options = ImportOptions {
        strict,
        ..ImportOptions::default()
    };
    let imported = match import_surface_model(&doc, root, unit, angle, model, options) {
        Ok(imported) => imported,
        Err(e) => return Ok(failure(&e, "[]")),
    };
    // Every shell is meshed and reported; the first failing shell fails the model.
    let mut shells = String::new();
    let mut first = None;
    for (index, shell) in imported.shells().iter().enumerate() {
        if index > 0 {
            shells.push(',');
        }
        write!(
            shells,
            "{{\"id\":{},\"closed\":{},\"faces\":{},\"boundary_edges\":{},",
            shell.entity().get(),
            shell.closed(),
            shell.face_entities().len(),
            shell.boundary_edges()
        )?;
        match shell.tessellate(tolerance, TessellationOptions::default()) {
            Ok(mesh) => {
                let stats = mesh.statistics();
                write!(
                    shells,
                    "\"status\":\"accepted\",\"vertices\":{},\"triangles\":{},\"mesh_boundary_edges\":{},\"components\":{},\"watertight\":{},\"signed_volume_m3\":{},\"area_m2\":{}}}",
                    mesh.data().positions.len(),
                    mesh.data().triangles.len(),
                    stats.boundary_edges,
                    stats.components,
                    mesh.is_watertight(),
                    stats.signed_volume,
                    area(&mesh)
                )?;
            }
            Err(e) => {
                write!(shells, "\"status\":\"{}\"}}", status(&e))?;
                first.get_or_insert(e);
            }
        }
    }
    let shells = format!("[{shells}]");
    if let Some(e) = first {
        return Ok(failure(&e, &shells));
    }
    let a = imported.adaptations();
    println!(
        "{{\"format_version\":1,\"scope\":\"surface-model\",\"status\":\"accepted\",\"profile\":\"accepted\",\"geometry\":\"accepted\",\"tessellation\":\"accepted\",\"inferred_outer_bounds\":{},\"inserted_seams\":{},\"split_edges\":{},\"collapsed_edges\":{},\"shells\":{shells}}}",
        a.inferred_outer_bounds, a.inserted_seams, a.split_edges, a.collapsed_edges
    );
    Ok(true)
}

/// Total triangle area: the shell's surface area within the chord-dependent bound.
fn area(mesh: &tessstep_mesh::Mesh) -> f64 {
    let d = mesh.data();
    d.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| d.positions[i as usize]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.
        })
        .sum()
}

fn status(e: &Error) -> &'static str {
    match e.kind {
        ErrorKind::Unsupported => "unsupported",
        ErrorKind::ResourceLimit => "resource_limit",
        _ => "rejected",
    }
}

fn failure(e: &Error, shells: &str) -> bool {
    let status = status(e);
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
        "{{\"format_version\":1,\"scope\":\"surface-model\",\"status\":\"{status}\",\"profile\":\"{profile}\",\"geometry\":\"{geometry}\",\"tessellation\":\"{tessellation}\",\"entity\":{},\"shells\":{shells}}}",
        e.entity.map_or(0, EntityId::get)
    );
    false
}
