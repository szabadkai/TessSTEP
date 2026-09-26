//! Existing-tessellation import throughput on a generated closed grid box, and
//! presentation-graphics import throughput on a generated repositioned annotation.
use std::{collections::BTreeMap, fmt::Write, hint::black_box, time::Instant};
use tessstep_import::{ImportOptions, PresentationLimits, import_presentation, import_tessellated};
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
    presentation();
}

/// A repositioned annotation of S curve sets (ten 20-point strips each) and S fill
/// fans (200 triangles each), every set over its own coordinate list.
fn presentation() {
    const S: usize = 100;
    let mut data = String::new();
    let mut children = Vec::new();
    for k in 0..S {
        let points: Vec<String> = (0..200)
            .map(|i| format!("({}.,{}.,0.)", i % 20 + k, i / 20))
            .collect();
        let strips: Vec<String> = (0..10)
            .map(|r| {
                let strip: Vec<String> = (1..=20).map(|c| (20 * r + c).to_string()).collect();
                format!("({})", strip.join(","))
            })
            .collect();
        let fan: Vec<String> = (1..=202).map(|i| i.to_string()).collect();
        let fill: Vec<String> = (0..202)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 201.;
                format!("({:.6},{:.6},0.)", k as f64 + a.cos(), a.sin())
            })
            .collect();
        let (curve, list, surface, fill_list) =
            (1000 + 4 * k, 1001 + 4 * k, 1002 + 4 * k, 1003 + 4 * k);
        let _ = writeln!(
            data,
            "#{list}=COORDINATES_LIST('',200,({}));\n#{curve}=TESSELLATED_CURVE_SET('',#{list},({}));\n\
             #{fill_list}=COORDINATES_LIST('',202,({}));\n\
             #{surface}=COMPLEX_TRIANGULATED_SURFACE_SET('',#{fill_list},202,((0.,0.,1.)),(),(),(({})));",
            points.join(","),
            strips.join(","),
            fill.join(","),
            fan.join(",")
        );
        children.extend([format!("#{curve}"), format!("#{surface}")]);
    }
    data.push_str(&format!(
        "#1=CARTESIAN_POINT('',(100.,0.,0.));#2=DIRECTION('',(1.,0.,0.));#3=DIRECTION('',(0.,1.,0.));\
         #4=AXIS2_PLACEMENT_3D('',#1,#2,#3);\n#5=(GEOMETRIC_REPRESENTATION_ITEM()REPOSITIONED_TESSELLATED_ITEM(#4)\
         REPRESENTATION_ITEM('')TESSELLATED_GEOMETRIC_SET(({}))TESSELLATED_ITEM());\n\
         #300=TESSELLATED_ANNOTATION_OCCURRENCE('',(),#5);\n",
        children.join(",")
    ));
    let text = format!(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');\
         FILE_SCHEMA(('AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF'));ENDSEC;DATA;\n{data}ENDSEC;END-ISO-10303-21;\n"
    );
    let doc = tessstep_model::parse(text.as_bytes(), ParseLimits::default()).unwrap();
    let options = ImportOptions {
        max_work: 50_000_000,
        ..ImportOptions::default()
    };
    let import = || {
        import_presentation(
            &doc,
            EntityId::new(300).unwrap(),
            LengthUnit::MILLIMETRE,
            options,
            PresentationLimits::default(),
        )
        .unwrap()
    };
    let warm = import();
    assert_eq!(warm.polyline_points().len(), S * 200);
    assert_eq!(warm.triangles().len(), S * 200);
    assert_eq!(warm.positions().len(), S * 402);
    let start = Instant::now();
    let mut iterations = 0;
    while start.elapsed().as_secs_f64() < 2. {
        black_box(import());
        iterations += 1;
    }
    let elapsed = start.elapsed();
    println!(
        "presentation: {} bytes, {} vertices, {} polylines, {} triangles; {iterations} imports in {elapsed:?}; {:.0} vertices/s",
        text.len(),
        warm.positions().len(),
        warm.polyline_count(),
        warm.triangles().len(),
        (warm.positions().len() * iterations) as f64 / elapsed.as_secs_f64()
    );
}
