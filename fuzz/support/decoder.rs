#[allow(dead_code)]
mod schema {
    include!("decoder_schema.rs");
}
pub fn exercise(data: &[u8]) {
    let limits = tessstep_part21::ParseLimits {
        max_input_bytes: 65536,
        max_token_bytes: 4096,
        max_string_bytes: 4096,
        max_entities: 128,
        max_nesting_depth: 16,
        max_aggregate_elements: 128,
        max_total_values: 2048,
        max_symbols: 128,
        max_records: 512,
        max_sections: 8,
    };
    let Ok(document) = tessstep_model::parse(data, limits) else {
        return;
    };
    let limits = tessstep_model::decode::Limits {
        max_work: 32768,
        max_depth: 32,
    };
    let a = tessstep_model::decode::decode(&document, &schema::SCHEMA_SET, "sample", limits);
    let b = tessstep_model::decode::decode(&document, &schema::SCHEMA_SET, "sample", limits);
    match (a, b) {
        (Ok(a), Ok(b)) => assert_eq!(
            a.entities().iter().map(|e| e.id).collect::<Vec<_>>(),
            b.entities().iter().map(|e| e.id).collect::<Vec<_>>()
        ),
        (Err(a), Err(b)) => {
            assert_eq!(a, b);
            if let Some(span) = a.source {
                assert!(
                    span.start.offset <= span.end.offset && span.end.offset as usize <= data.len()
                );
            }
        }
        _ => panic!("nondeterministic decoding"),
    }
}
