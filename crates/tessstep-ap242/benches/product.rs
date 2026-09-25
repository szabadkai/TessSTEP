use std::{
    hint::black_box,
    time::{Duration, Instant},
};
#[allow(dead_code)]
mod schema {
    include!("../tests/support/schema.rs");
}
fn main() {
    let base = include_str!("../../../corpus/product/assembly.step");
    let mut extra = String::new();
    for id in 100..10_100 {
        extra.push_str(&format!(
            "#{id}=NEXT_ASSEMBLY_USAGE_OCCURRENCE('repeat',#5,#6);\n"
        ));
    }
    let input = base.replace("ENDSEC;\nEND-ISO", &(extra + "ENDSEC;\nEND-ISO"));
    let document = tessstep_model::parse(input.as_bytes(), Default::default()).unwrap();
    let decoded = tessstep_model::decode::decode(
        &document,
        &schema::SCHEMA_SET,
        "product_test",
        Default::default(),
    )
    .unwrap();
    let once = || {
        let model =
            tessstep_ap242::adapt(black_box(&decoded), "product_test", Default::default()).unwrap();
        assert_eq!(model.parts().occurrences.len(), 10_002);
        black_box(model);
    };
    once();
    let start = Instant::now();
    let mut iterations = 0;
    while start.elapsed() < Duration::from_secs(2) {
        once();
        iterations += 1;
    }
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "product adapter: {} entities, {} bytes, {iterations} iterations, {seconds:.3} s, {:.3} ms/input",
        document.entities().len(),
        input.len(),
        seconds * 1000.0 / f64::from(iterations)
    );
}
