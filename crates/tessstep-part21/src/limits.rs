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
impl ParseLimits {
    /// Fewest input bytes a value takes with its separator, as in `$,`. Bounds both
    /// the total values and the elements of one aggregate.
    pub const BYTES_PER_VALUE: u64 = 2;
    /// Fewest input bytes a record takes, as in `A()`.
    pub const BYTES_PER_RECORD: u64 = 3;
    /// Fewest input bytes an entity instance takes, as in `#1=A();`.
    pub const BYTES_PER_ENTITY: u64 = 7;
    /// Budgets for an input of `input_bytes` bytes, analogous to
    /// `ImportOptions::for_document`: the defaults, with the input byte budget raised
    /// to `input_bytes` and the entity, record, total value and aggregate element
    /// budgets raised to the most of each that `input_bytes` of syntax can hold
    /// (`BYTES_PER_*`). No budget falls below its default, so no input the defaults
    /// accept is rejected, and no input of at most `input_bytes` bytes exceeds those
    /// four. The token, string, nesting, symbol and section budgets keep their
    /// defaults: they do not grow with the model. Each budget stays explicit in the returned limits
    /// and is named by the diagnostic that reports it. Logical allocation then grows
    /// with the input: the parsed document takes about ten times the input bytes on
    /// exporter files, and a hostile input can take several times more, so callers
    /// that cannot afford that keep the defaults or set lower budgets.
    pub fn for_input(input_bytes: u64) -> Self {
        let defaults = Self::default();
        let most = |default: usize, bytes_per_item: u64| {
            default.max(usize::try_from(input_bytes / bytes_per_item).unwrap_or(usize::MAX))
        };
        Self {
            max_input_bytes: defaults.max_input_bytes.max(input_bytes),
            max_entities: most(defaults.max_entities, Self::BYTES_PER_ENTITY),
            max_records: most(defaults.max_records, Self::BYTES_PER_RECORD),
            max_total_values: most(defaults.max_total_values, Self::BYTES_PER_VALUE),
            max_aggregate_elements: most(defaults.max_aggregate_elements, Self::BYTES_PER_VALUE),
            ..defaults
        }
    }
}
