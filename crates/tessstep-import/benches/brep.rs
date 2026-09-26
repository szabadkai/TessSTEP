//! Curved B-rep import and tessellation throughput, and per-root import cost as the
//! document grows: a document holding K renumbered copies of the authored washer.
use std::{fmt::Write, hint::black_box, time::Instant};
use tessstep_import::{
    ImportOptions, TessellationOptions, TessellationTolerance, import_brep_solid,
};
use tessstep_math::{Angle, AngleUnit, Length, LengthUnit, ModelTolerance};
use tessstep_part21::{EntityId, ParseLimits};

const WASHER: &str = include_str!("../../../corpus/geometry/brep-washer.step");

/// Renumber every `#id` by `offset`, so copies can share one DATA section.
fn renumber(data: &str, offset: u64) -> String {
    let mut out = String::with_capacity(data.len() + data.len() / 8);
    let mut chars = data.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '#' {
            out.push(c);
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while let Some(&(j, d)) = chars.peek() {
            if !d.is_ascii_digit() {
                break;
            }
            end = j + 1;
            chars.next();
        }
        let id: u64 = data[start..end].parse().expect("entity id");
        let _ = write!(out, "#{}", id + offset);
    }
    out
}

fn main() {
    let (header, rest) = WASHER.split_once("DATA;\n").unwrap();
    let body = rest.split_once("ENDSEC;").unwrap().0;
    let model =
        ModelTolerance::new(Length::metres(1e-8).unwrap(), Angle::radians(1e-8).unwrap()).unwrap();
    let chord =
        TessellationTolerance::new(Length::metres(1e-5).unwrap(), Angle::radians(0.1).unwrap())
            .unwrap();
    let import = |doc: &tessstep_model::Document, root: u64| {
        import_brep_solid(
            doc,
            EntityId::new(root).unwrap(),
            LengthUnit::MILLIMETRE,
            AngleUnit::RADIAN,
            model,
            ImportOptions::default(),
        )
        .unwrap()
    };
    let doc = tessstep_model::parse(WASHER.as_bytes(), ParseLimits::default()).unwrap();
    let solid = import(&doc, 1000);
    let mesh = solid
        .tessellate(chord, TessellationOptions::default())
        .unwrap();
    mesh.require_solid().unwrap();
    let triangles = mesh.data().triangles.len();
    let time = |f: &mut dyn FnMut()| {
        let start = Instant::now();
        let mut n = 0;
        while start.elapsed().as_secs_f64() < 1. {
            f();
            n += 1;
        }
        start.elapsed().as_secs_f64() / n as f64
    };
    let import_s = time(&mut || {
        black_box(import(&doc, 1000));
    });
    let tessellate_s = time(&mut || {
        black_box(
            solid
                .tessellate(chord, TessellationOptions::default())
                .unwrap(),
        );
    });
    println!(
        "washer: import {:.1} us, tessellate {:.2} ms ({triangles} triangles at 1e-5 m)",
        import_s * 1e6,
        tessellate_s * 1e3
    );
    for copies in [1u64, 16, 128] {
        let mut data = String::new();
        for k in 0..copies {
            data.push_str(&renumber(body, k * 10_000));
        }
        let text = format!("{header}DATA;\n{data}ENDSEC;\nEND-ISO-10303-21;\n");
        let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
        let per_root = time(&mut || {
            for k in 0..copies.min(16) {
                black_box(import(&doc, 1000 + k * 10_000));
            }
        }) / copies.min(16) as f64;
        println!(
            "{copies:>4} washers ({} entities): {:.1} us per root import",
            doc.entities().len(),
            per_root * 1e6
        );
    }
}
