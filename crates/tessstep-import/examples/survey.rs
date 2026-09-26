//! Usage: cargo run -p tessstep-import --example survey -- FILE METRES_PER_UNIT [MAX_ROOTS]
//! Imports every MANIFOLD_SOLID_BREP, BREP_WITH_VOIDS and FACETED_BREP root with the
//! matching selected-root profile and reports per-root stage outcomes as JSON.
//! The unit scale is an explicit caller assumption; it is not read from the file.
#![forbid(unsafe_code)]
use std::{fmt::Write, fs::File, io::BufReader, time::Instant};
use tessstep_import::*;
use tessstep_math::{Angle, Length, LengthUnit, ModelTolerance};
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
    let roots: Vec<(EntityId, &str)> = doc
        .entities()
        .iter()
        .filter_map(|e| {
            let names = e.kind.records().iter().map(|r| r.name.as_ref());
            let mut found = None;
            for name in names {
                for root in ["BREP_WITH_VOIDS", "FACETED_BREP", "MANIFOLD_SOLID_BREP"] {
                    if name.eq_ignore_ascii_case(root) && found.is_none_or(|f| f > root) {
                        found = Some(root);
                    }
                }
            }
            found.map(|root| (e.id, root))
        })
        .collect();
    let model = ModelTolerance::new(Length::metres(1e-8)?, Angle::radians(1e-8)?)?;
    let tolerance = TessellationTolerance::new(Length::metres(1e-6)?, Angle::radians(0.1)?)?;
    write!(
        out,
        ",\"parse\":\"accepted\",\"root_count\":{},\"truncated\":{},\"roots\":[",
        roots.len(),
        roots.len() > max_roots
    )?;
    for (index, &(id, kind)) in roots.iter().take(max_roots).enumerate() {
        let started = Instant::now();
        let faceted = kind == "FACETED_BREP";
        let imported = if faceted {
            import_faceted_solid(&doc, id, unit, model, ImportOptions::default())
        } else {
            import_planar_solid(&doc, id, unit, model, ImportOptions::default())
        };
        let result =
            imported.and_then(|solid| solid.tessellate(tolerance, TessellationOptions::default()));
        if index > 0 {
            out.push(',');
        }
        write!(
            out,
            "{{\"id\":{},\"type\":\"{kind}\",\"profile\":\"{}\",",
            id.get(),
            if faceted { "faceted" } else { "planar" }
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
        write!(out, ",\"seconds\":{:.6}}}", started.elapsed().as_secs_f64())?;
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
