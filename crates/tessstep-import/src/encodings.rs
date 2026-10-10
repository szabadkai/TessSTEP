//! Exporter encodings of units, measures and complex instances that the tolerant
//! default accepts in the context and product profiles, and their counts.
use super::*;

/// Nonconformant exporter encodings accepted without changing what a record means,
/// counted over one decoded unit or product closure. [`ImportOptions::strict`]
/// rejects every one of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToleratedEncodings {
    /// SI units whose DIMENSIONS slot holds DIMENSIONAL_EXPONENTS instead of the
    /// derived `*`; the exponents are checked against the unit role.
    pub explicit_si_dimensions: usize,
    /// SI units whose DIMENSIONS slot is `$` instead of `*`.
    pub unset_si_dimensions: usize,
    /// Conversion-based units whose DIMENSIONS slot is `*` instead of explicit
    /// exponents; the dimensions follow from the unit role.
    pub omitted_conversion_dimensions: usize,
    /// Measure values written as a bare number where the measure SELECT requires a
    /// typed value such as `LENGTH_MEASURE(0.01)`; the unit gives the dimension.
    pub untyped_measures: usize,
    /// Complex instances whose components are not in canonical (alphabetical) order.
    pub unordered_complex: usize,
    /// Item defined transformations (product records only) whose transform_item_1
    /// belongs to rep_2 and transform_item_2 to rep_1, as HOOPS Exchange and the CATIA V4
    /// interface write them; the items are exchanged so that each belongs to its
    /// representation. Not part of the frozen C record `ts_tolerated_encodings`.
    pub reversed_transform_items: usize,
}
impl ToleratedEncodings {
    pub fn total(&self) -> usize {
        self.explicit_si_dimensions
            + self.unset_si_dimensions
            + self.omitted_conversion_dimensions
            + self.untyped_measures
            + self.unordered_complex
            + self.reversed_transform_items
    }
    /// Count the encodings in a closure decoded under [`policy`]. The measure value
    /// is the only SELECT of these profiles with defined-type alternatives, so every
    /// untyped SELECT value the decoder admitted is an untyped measure.
    pub(crate) fn of(decoded: &DecodedDocument<'_>) -> Self {
        let retained = decoded.retained();
        let mut counts = Self {
            untyped_measures: retained.untyped_select,
            unordered_complex: retained.unordered_complex,
            ..Self::default()
        };
        let is = |v: &EntityView<'_>, name: &str| {
            v.types.iter().any(|id| {
                decoded
                    .schemas()
                    .declaration(*id)
                    .is_some_and(|d| d.name == name)
            })
        };
        for v in decoded.entities() {
            let Some(dimensions) = v
                .attributes
                .iter()
                .find(|a| a.declaration.name == "DIMENSIONS")
                .map(|a| &a.value.kind)
            else {
                continue;
            };
            if is(v, "SI_UNIT") {
                match dimensions {
                    ValueKind::Reference(_) => counts.explicit_si_dimensions += 1,
                    ValueKind::Null => counts.unset_si_dimensions += 1,
                    _ => (),
                }
            } else if is(v, "CONVERSION_BASED_UNIT") && matches!(dimensions, ValueKind::Omitted) {
                counts.omitted_conversion_dimensions += 1;
            }
        }
        counts
    }
}

/// The decoder policy of the context and product profiles: unordered complex
/// components and untyped measure values are admitted (and counted) unless strict.
pub(crate) fn policy(options: ImportOptions) -> decode::Policy {
    decode::Policy {
        accept_unordered_complex: !options.strict,
        accept_untyped_select: !options.strict,
        ..decode::Policy::default()
    }
}
