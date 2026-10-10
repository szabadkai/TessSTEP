//! Usage: cargo run -p tessstep-import --example assembly -- [--meshes] [--strict] SCHEMA FILE
//! Links the product structure of FILE (products, definitions, next assembly usage
//! occurrences and their context dependent or mapped-item placements) with the
//! bundled product profile and reports it as the corpus product stage
//! (`product-structure`, format 1). SCHEMA is the declared AP schema selected by the
//! corpus runner; it is reported, and the bounded profile is the same for every AP.
//! Without --meshes no shape is imported: `placed_roots` counts discovered shape
//! roots that a placed representation holds. With --meshes every selected root is
//! imported with its context units (chord 0.1 mm, model tolerance the declared
//! uncertainty floored at 1e-7 m) and placed in a scene, and surface styles are
//! adapted to its appearance. `mapped_placements` and the `tolerated` encoding
//! counts are written only when nonzero; --strict rejects those encodings and the
//! exporter slips the parser otherwise accepts (`Conformance::Strict`). Parse budgets
//! are sized from the file length (`ParseLimits::for_input`), as the corpus runner's
//! physical stage sizes them. Exit status 0 means accepted.
#![forbid(unsafe_code)]
use std::{collections::BTreeSet, fmt::Write, fs::File, io::BufReader};
use tessstep_import::*;
use tessstep_part21::{Conformance, ParseLimits};

fn main() {
    match run() {
        Ok(accepted) => std::process::exit(i32::from(!accepted)),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}

fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .map(|i| args.remove(i))
            .is_some()
    };
    let meshes = flag("--meshes");
    let strict = flag("--strict");
    let [schema, path] = args.as_slice() else {
        return Err("usage: assembly [--meshes] [--strict] SCHEMA FILE".into());
    };
    let file = File::open(path)?;
    let limits = ParseLimits::for_input(file.metadata()?.len());
    let conformance = if strict {
        Conformance::Strict
    } else {
        Conformance::Tolerant
    };
    let doc = tessstep_model::parse_with(BufReader::new(file), limits, conformance)?;
    let mut out = format!(
        "{{\"format_version\":1,\"scope\":\"product-structure\",\"declared\":\"{}\",\"entity_count\":{}",
        escape(schema),
        doc.entities().len()
    );
    let mut options = AssemblyImportOptions::default();
    options.assembly.import.strict = strict;
    // One session discovers roots once for the report, the assembly and its styles.
    let session = ImportSession::new(&doc, options.assembly.import);
    let solids = session.solids()?;
    let tessellations = session.tessellations()?;
    let discovered: BTreeSet<_> = solids
        .iter()
        .map(|s| s.entity)
        .chain(tessellations.iter().map(|t| t.entity))
        .collect();
    let result = if meshes {
        session
            .import_assembly(options)
            .map(|i| (i.assembly, Some(i.roots)))
    } else {
        session
            .link_assembly(&[], options.assembly)
            .map(|a| (a, None))
    };
    let accepted = match result {
        Ok((assembly, roots)) => {
            // An excluded occurrence or placement leaves the structure incomplete, and
            // a structure whose every record was excluded is not an absent one.
            let empty = assembly.products().is_empty() && assembly.occurrence_count() == 0;
            let structural = assembly.excluded().iter().find(|e| e.structural || empty);
            let status = if let Some(first) = structural {
                match first.error.kind {
                    ErrorKind::Unsupported => "unsupported",
                    ErrorKind::ResourceLimit => "resource_limit",
                    _ => "rejected",
                }
            } else if empty {
                "not_applicable"
            } else {
                "accepted"
            };
            let placed: BTreeSet<_> = if meshes {
                assembly.nodes().iter().filter_map(|n| n.root).collect()
            } else {
                assembly.unimported().iter().copied().collect()
            };
            let leaves = assembly.nodes().iter().filter(|n| n.root.is_some()).count();
            write!(
                out,
                ",\"status\":\"{status}\",\"products\":{},\"occurrences\":{},\"definition_nodes\":{},\"asset_instances\":{},\"placed_roots\":{},\"unplaced_roots\":{},\"excluded\":{}",
                assembly.products().len(),
                assembly.occurrence_count(),
                assembly.nodes().len() - leaves,
                leaves,
                placed.len(),
                discovered.difference(&placed).count(),
                assembly.excluded().len()
            )?;
            if let Some(first) = structural.or(assembly.excluded().first()) {
                write!(
                    out,
                    ",\"excluded_first\":\"{}\"",
                    escape(&first.error.to_string())
                )?;
            }
            if assembly.mapped_placements() > 0 {
                write!(
                    out,
                    ",\"mapped_placements\":{}",
                    assembly.mapped_placements()
                )?;
            }
            let t = assembly.tolerated();
            if t.total() > 0 {
                write!(
                    out,
                    ",\"tolerated\":{{\"explicit_si_dimensions\":{},\"unset_si_dimensions\":{},\"omitted_conversion_dimensions\":{},\"untyped_measures\":{},\"unordered_complex\":{}",
                    t.explicit_si_dimensions,
                    t.unset_si_dimensions,
                    t.omitted_conversion_dimensions,
                    t.untyped_measures,
                    t.unordered_complex
                )?;
                // Written only when nonzero, so earlier product results stay unchanged.
                if t.reversed_transform_items > 0 {
                    write!(
                        out,
                        ",\"reversed_transform_items\":{}",
                        t.reversed_transform_items
                    )?;
                }
                out.push('}');
            }
            if let Some(roots) = roots {
                // World bounds and volume of every placed asset instance, in metres.
                let (mut lo, mut hi, mut volume) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3], 0.);
                let scene = assembly.scene();
                for resolved in scene.instances() {
                    let Some(asset) = resolved.source.asset.and_then(|a| scene.asset(a)) else {
                        continue;
                    };
                    let t = resolved.world_transform;
                    for &p in &asset.mesh.data().positions {
                        let w = t
                            .transform_point(tessstep_math::Point3::new(p)?)?
                            .coordinates();
                        for k in 0..3 {
                            lo[k] = lo[k].min(w[k]);
                            hi[k] = hi[k].max(w[k]);
                        }
                    }
                    // A mirrored instance is rendered (and baked) with reversed
                    // winding, so its volume scales by |det|, not by det.
                    let l = t.linear();
                    let det = l[0][0] * (l[1][1] * l[2][2] - l[1][2] * l[2][1])
                        - l[0][1] * (l[1][0] * l[2][2] - l[1][2] * l[2][0])
                        + l[0][2] * (l[1][0] * l[2][1] - l[1][1] * l[2][0]);
                    volume += asset.mesh.statistics().signed_volume * det.abs();
                }
                if lo[0].is_finite() {
                    write!(
                        out,
                        ",\"world_bounds_m\":[{:?},{:?}],\"volume_m3\":{volume:e}",
                        lo, hi
                    )?;
                }
                match session.import_appearance(&assembly, StyleOptions::default()) {
                    Ok(styles) => {
                        let c = styles.counts();
                        let a = styles.appearance();
                        write!(
                            out,
                            ",\"styles\":{{\"status\":\"accepted\",\"styled_items\":{},\"asset_styles\":{},\"face_styles\":{},\"unimported\":{},\"unsupported_targets\":{},\"no_surface_colour\":{},\"context_styles\":{},\"conflicts\":{},\"complex_items\":{},\"layers\":{},\"invisibility\":{},\"excluded\":{},\"excluded_first\":\"{}\",\"materials\":{},\"bindings\":{}}}",
                            c.styled_items,
                            c.asset_styles,
                            c.face_styles,
                            c.unimported,
                            c.unsupported_targets.values().sum::<usize>(),
                            c.no_surface_colour,
                            c.context_styles,
                            c.conflicts,
                            c.complex_items,
                            c.layers,
                            c.invisibility,
                            styles.excluded().len(),
                            styles
                                .excluded()
                                .first()
                                .map_or(String::new(), |e| escape(&e.error.to_string())),
                            a.materials().len(),
                            a.bindings().len()
                        )?;
                    }
                    Err(e) => write!(
                        out,
                        ",\"styles\":{{\"status\":\"rejected\",\"message\":\"{}\"}}",
                        escape(&e.to_string())
                    )?,
                }
                let imported = roots.iter().filter(|r| r.result.is_ok()).count();
                write!(
                    out,
                    ",\"imported_roots\":{imported},\"failed_imports\":{},\"unimported_placed\":{}",
                    roots.len() - imported,
                    assembly.unimported().len()
                )?;
            }
            status == "accepted"
        }
        Err(e) => {
            let status = match e.kind {
                ErrorKind::Unsupported => "unsupported",
                ErrorKind::ResourceLimit => "resource_limit",
                _ => "rejected",
            };
            write!(
                out,
                ",\"status\":\"{status}\",\"kind\":\"{:?}\",\"stage\":\"{:?}\",\"entity\":{},\"message\":\"{}\"",
                e.kind,
                e.stage,
                e.entity.map_or("null".into(), |id| id.get().to_string()),
                escape(&e.message)
            )?;
            false
        }
    };
    println!("{out}}}");
    Ok(accepted)
}
