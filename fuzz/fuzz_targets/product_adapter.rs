#![no_main]
use libfuzzer_sys::fuzz_target;
#[allow(dead_code)]
mod schema {
    include!("../../crates/tessstep-ap242/tests/support/schema.rs");
}
fuzz_target!(|bytes: &[u8]| {
    let limits = tessstep_part21::ParseLimits {
        max_input_bytes: 65_536,
        max_entities: 512,
        max_total_values: 4096,
        max_nesting_depth: 32,
        ..Default::default()
    };
    if let Ok(document) = tessstep_model::parse(bytes, limits) {
        if let Ok(decoded) = tessstep_model::decode::decode(
            &document,
            &schema::SCHEMA_SET,
            "product_test",
            tessstep_model::decode::Limits {
                max_work: 100_000,
                max_depth: 32,
            },
        ) {
            let limits = tessstep_ap242::Limits {
                max_work: 50_000,
                max_text_bytes: 4096,
                max_unit_depth: 16,
            };
            assert_eq!(
                tessstep_ap242::adapt(&decoded, "product_test", limits),
                tessstep_ap242::adapt(&decoded, "product_test", limits)
            );
        }
    }
});
