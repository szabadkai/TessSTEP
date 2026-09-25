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
            let first = tessstep_ap242::adapt(&decoded, "product_test", limits);
            assert_eq!(
                first,
                tessstep_ap242::adapt(&decoded, "product_test", limits)
            );
            if let Ok(model) = first {
                if let Some(&root) = model.roots().first() {
                    if let Some(&rep) = model.representations_of(root).and_then(|r| r.first()) {
                        let limits = tessstep_product::ExpansionLimits {
                            max_work: 30_000,
                            max_instances: 64,
                            max_depth: 32,
                        };
                        assert_eq!(
                            model.expand(root, rep, &Default::default(), limits),
                            model.expand(root, rep, &Default::default(), limits)
                        );
                    }
                }
            }
        }
    }
});
