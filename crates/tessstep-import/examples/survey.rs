//! Usage: cargo run -p tessstep-import --example survey -- FILE METRES_PER_UNIT [MAX_ROOTS]
//! Imports every MANIFOLD_SOLID_BREP, BREP_WITH_VOIDS and FACETED_BREP root with the
//! matching selected-root profile and reports per-root stage outcomes as JSON.
//! Each root uses the units of its representation context when discovery finds them,
//! and the declared length uncertainty as its model tolerance, floored at 1e-7 m;
//! METRES_PER_UNIT, radians and 1e-7 m are the explicit fallback. Each root reports
//! which applied, the declared uncertainty and the tolerance and chord it used.
#![forbid(unsafe_code)]
use std::{fmt::Write, fs::File, io::BufReader, time::Instant};
use tessstep_import::*;
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_model::Document;
use tessstep_part21::{EntityId, ParseLimits};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: survey FILE METRES_PER_UNIT [MAX_ROOTS]".into());
    }
    let scale: f64 = args[2].parse()?;
    let unit = LengthUnit::metres_per_unit(scale)?;
    let max_roots: usize = args.get(3).map_or(Ok(1000), |s| s.parse())?;
    let mut out =
        format!("{{\"format_version\":1,\"scope\":\"solid-survey\",\"metres_per_unit\":{scale}");
    let doc = match tessstep_model::parse(
        BufReader::new(File::open(&args[1])?),
        ParseLimits::default(),
    ) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("{e}");
            println!(
                "{out},\"parse\":\"rejected\",\"root_count\":0,\"truncated\":false,\"roots\":[]}}"
            );
            return Ok(());
        }
    };
    let roots = discover_solids(&doc, ImportOptions::default())?;
    write!(
        out,
        ",\"parse\":\"accepted\",\"root_count\":{},\"truncated\":{},\"roots\":[",
        roots.len(),
        roots.len() > max_roots
    )?;
    for (index, root) in roots.iter().take(max_roots).enumerate() {
        let started = Instant::now();
        let (id, kind) = (root.entity, root.kind.step_name());
        let faceted = root.kind == SolidKind::FacetedBrep;
        // Context units when the file declares them; otherwise the explicit command-line
        // length scale and radians. The model tolerance is the declared length
        // uncertainty, but never below 1e-7 m: several exporters declare values their
        // written geometry cannot meet (curves 10-30 nm off 5-10 nm uncertainties).
        let (length, angle, declared, units) = match &root.units {
            Ok(u) => (
                u.length,
                u.plane_angle,
                u.distance_uncertainty,
                if u.distance_uncertainty.is_some() {
                    "context"
                } else {
                    "context_units"
                },
            ),
            Err(_) => (unit, AngleUnit::RADIAN, None, "assumed"),
        };
        let distance = declared.unwrap_or(1e-7).max(1e-7);
        let model = ModelTolerance::new(Length::metres(distance)?, Angle::radians(1e-8)?)?;
        let imported = if faceted {
            import_faceted_solid(&doc, id, length, model, ImportOptions::default())
        } else {
            import_brep_solid(&doc, id, length, angle, model, ImportOptions::default())
        };
        // Chord 1e-3 of the solid's vertex bounding-box diagonal, within 1e-6..1e-3 m.
        let mut chord = 1e-6;
        let result = imported.and_then(|solid| {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for v in &solid.brep().data().vertices {
                for (k, x) in v.position.coordinates().into_iter().enumerate() {
                    lo[k] = lo[k].min(x);
                    hi[k] = hi[k].max(x);
                }
            }
            let diagonal = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f64>().sqrt();
            if diagonal.is_finite() {
                chord = (diagonal * 1e-3).clamp(1e-6, 1e-3);
            }
            let tolerance = TessellationTolerance::new(
                Length::metres(chord).expect("finite chord"),
                Angle::radians(0.1).expect("finite angle"),
            )
            .expect("valid tolerance");
            solid.tessellate(tolerance, TessellationOptions::default())
        });
        if index > 0 {
            out.push(',');
        }
        write!(
            out,
            "{{\"id\":{},\"type\":\"{kind}\",\"profile\":\"{}\",\"units\":\"{units}\",\"metres_per_unit\":{},\"model_tolerance_m\":{distance:e},\"declared_uncertainty_m\":{},",
            id.get(),
            if faceted { "faceted" } else { "brep" },
            length.scale(),
            declared.map_or("null".to_string(), |d| format!("{d:e}"))
        )?;
        match result {
            Ok(mesh) => write!(
                out,
                "\"status\":\"accepted\",\"stages\":{{\"profile\":\"accepted\",\"geometry\":\"accepted\",\"tessellation\":\"accepted\"}},\"vertices\":{},\"triangles\":{},\"volume_m3\":{}",
                mesh.data().positions.len(),
                mesh.data().triangles.len(),
                mesh.statistics().signed_volume
            )?,
            Err(e) => failure(&mut out, &doc, &e)?,
        }
        write!(
            out,
            ",\"chord_m\":{chord:e},\"seconds\":{:.6}}}",
            started.elapsed().as_secs_f64()
        )?;
    }
    out.push_str("]}");
    println!("{out}");
    Ok(())
}

fn failure(out: &mut String, doc: &Document, e: &Error) -> std::fmt::Result {
    let status = match e.kind {
        ErrorKind::Unsupported => "unsupported",
        ErrorKind::ResourceLimit => "resource_limit",
        _ => "rejected",
    };
    let (profile, geometry, tessellation) = match e.stage {
        Stage::Profile => (status, "not_run", "not_run"),
        Stage::Geometry | Stage::Topology => ("accepted", status, "not_run"),
        Stage::Tessellation => ("accepted", "accepted", status),
    };
    let kind = match e.kind {
        ErrorKind::InvalidOptions => "invalid_options",
        ErrorKind::MissingEntity => "missing_entity",
        ErrorKind::Unsupported => "unsupported",
        ErrorKind::InvalidGeometry => "invalid_geometry",
        ErrorKind::ResourceLimit => "resource_limit",
    };
    let entity_type = e
        .entity
        .and_then(|id| doc.entities().get(id))
        .map(|entity| {
            entity
                .kind
                .records()
                .iter()
                .map(|r| r.name.as_ref())
                .collect::<Vec<_>>()
                .join("+")
        })
        .unwrap_or_default();
    write!(
        out,
        "\"status\":\"{status}\",\"stages\":{{\"profile\":\"{profile}\",\"geometry\":\"{geometry}\",\"tessellation\":\"{tessellation}\"}},\"failed_stage\":\"{}\",\"kind\":\"{kind}\",\"entity\":{},\"entity_type\":",
        format!("{:?}", e.stage).to_lowercase(),
        e.entity.map_or(0, EntityId::get)
    )?;
    json_string(out, &entity_type)?;
    out.push_str(",\"message\":");
    json_string(out, &e.message)
}

fn json_string(out: &mut String, text: &str) -> std::fmt::Result {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                write!(out, "\\u{:04x}", c as u32)?
            }
            c => out.push(c),
        }
    }
    out.push('"');
    Ok(())
}
