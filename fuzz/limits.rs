use tessstep_part21::ParseLimits;
pub fn limits() -> ParseLimits {
    ParseLimits {
        max_input_bytes: 64 * 1024,
        max_token_bytes: 4096,
        max_string_bytes: 4096,
        max_entities: 256,
        max_nesting_depth: 32,
        max_aggregate_elements: 128,
        max_total_values: 4096,
        max_symbols: 256,
        max_records: 512,
        max_sections: 32,
    }
}
