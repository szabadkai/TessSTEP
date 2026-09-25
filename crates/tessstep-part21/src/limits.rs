/// Finite budgets apply to the entire parse, including discarded streaming events.
/// Limits bound logical allocations, not exact allocator overhead or process RSS.
#[derive(Debug, Clone, Copy)]
pub struct ParseLimits {
    pub max_input_bytes: u64,
    pub max_token_bytes: usize,
    pub max_string_bytes: usize,
    pub max_entities: usize,
    pub max_nesting_depth: usize,
    pub max_aggregate_elements: usize,
    pub max_total_values: usize,
    pub max_symbols: usize,
    pub max_records: usize,
    pub max_sections: usize,
}
impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 256 * 1024 * 1024,
            max_token_bytes: 1024 * 1024,
            max_string_bytes: 1024 * 1024,
            max_entities: 1_000_000,
            max_nesting_depth: 64,
            max_aggregate_elements: 100_000,
            max_total_values: 4_000_000,
            max_symbols: 100_000,
            max_records: 2_000_000,
            max_sections: 1024,
        }
    }
}
