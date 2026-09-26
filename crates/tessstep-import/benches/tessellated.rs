//! Existing-tessellation import throughput on a generated closed grid box.
use std::{collections::BTreeMap, fmt::Write, hint::black_box, time::Instant};
use tessstep_import::{ImportOptions, import_tessellated};
use tessstep_math::LengthUnit;
use tessstep_part21::{EntityId, ParseLimits};
const N: i64 = 100;
fn main() {
    // Each cube face is an N x N quad grid written as one strip per row. Frames are
    // right-handed with U x V outward, so interleaving rows (r+1, r) keeps winding.
    let faces: [([i64; 3], [i64; 3], [i64; 3]); 6] = [
        ([0, 0, 0], [0, 1, 0], [1, 0, 0]),
        ([0, 0, N], [1, 0, 0], [0, 1, 0]),
        ([0, 0, 0], [1, 0, 0], [0, 0, 1]),
        ([0, N, 0], [0, 0, 1], [1, 0, 0]),
        ([0, 0, 0], [0, 0, 1], [0, 1, 0]),
        ([N, 0, 0], [0, 1, 0], [0, 0, 1]),
    ];
    let mut index = BTreeMap::new();
    let mut coordinates = String::new();
    let mut strips = Vec::new();
    for (origin, u, v) in faces {
        let mut face = String::new();
        for r in 0..N {
            let mut strip = Vec::new();
            for c in 0..=N {
                for row in [r + 1, r] {
                    let p = [0, 1, 2].map(|i| origin[i] + c * u[i] + row * v[i]);
                    let next = index.len() + 1;
                    let id = *index.entry(p).or_insert_with(|| {
                        let _ = write!(
                            coordinates,
                            "{}({}.,{}.,{}.)",
                            if next == 1 { "" } else { "," },
                            p[0],
                            p[1],
                            p[2]
                        );
                        next
                    });
                    strip.push(id.to_string());
                }
            }
            let _ = write!(
                face,
                "{}({})",
                if r == 0 { "" } else { "," },
                strip.join(",")
            );
        }
        strips.push(face);
    }
    let npoints = index.len();
    let mut data = format!("#1=COORDINATES_LIST('',{npoints},({coordinates}));\n");
    for (i, face) in strips.iter().enumerate() {
        let _ = writeln!(
            data,
            "#{}=COMPLEX_TRIANGULATED_FACE('',#1,{npoints},(),$,(),({face}),());",
            10 + i
        );
    }
    data.push_str("#1000=TESSELLATED_SOLID('',(#10,#11,#12,#13,#14,#15),$);\n");
    let text = format!(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');\
         FILE_SCHEMA(('AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF'));ENDSEC;DATA;\n{data}ENDSEC;END-ISO-10303-21;\n"
    );
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    let limits = ImportOptions {
        max_work: 50_000_000,
        max_records: 100_000,
        ..ImportOptions::default()
    };
    let import = || {
        import_tessellated(
            &doc,
            EntityId::new(1000).unwrap(),
            LengthUnit::MILLIMETRE,
            limits,
            tessstep_mesh::Limits::default(),
        )
        .unwrap()
    };
    let warm = import();
    warm.mesh().require_solid().unwrap();
    let triangles = warm.mesh().data().triangles.len();
    assert_eq!(triangles, 12 * (N * N) as usize);
    assert!((warm.mesh().statistics().signed_volume - (N as f64 * 1e-3).powi(3)).abs() < 1e-12);
    let start = Instant::now();
    let mut iterations = 0;
    while start.elapsed().as_secs_f64() < 2. {
        black_box(import());
        iterations += 1;
    }
    let elapsed = start.elapsed();
    println!(
        "tessellated solid: {} bytes, {npoints} points, {triangles} triangles; {iterations} imports in {elapsed:?}; {:.0} triangles/s",
        text.len(),
        (triangles * iterations) as f64 / elapsed.as_secs_f64()
    );
}
