use crate::SourceSpan;

/// Whether a parse accepts the exporter slips counted in [`Deviations`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Conformance {
    /// Accept each slip of [`Deviations`] without changing what the file says, and
    /// count it (the default).
    #[default]
    Tolerant,
    /// Reject every slip, as ISO 10303-21 requires.
    Strict,
}

/// How often one kind of slip occurred, and where first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Deviation {
    pub count: u64,
    pub first: Option<SourceSpan>,
}
impl Deviation {
    pub(crate) fn record(&mut self, source: SourceSpan) {
        self.count += 1;
        self.first.get_or_insert(source);
    }
}

/// Exporter slips a tolerant parse accepted. A strict parse rejects each of them, so
/// its deviations are always empty. Kinds may be added within a major version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Deviations {
    /// Backslashes in strings whose following characters complete no directive
    /// (as in the Windows path `'C:\temp'`), kept as literal backslashes.
    pub literal_backslashes: Deviation,
    /// `$` in place of a string of FILE_DESCRIPTION or FILE_NAME (as an unset
    /// authorisation), or of an element of their string lists. The value stays `$`.
    pub null_header_strings: Deviation,
}
impl Deviations {
    pub fn total(&self) -> u64 {
        self.literal_backslashes.count + self.null_header_strings.count
    }
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}
