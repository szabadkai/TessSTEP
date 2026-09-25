use crate::EntityId;
use std::fmt;

/// Byte offsets are zero-based; lines and byte columns are one-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub offset: u64,
    pub line: u64,
    pub column: u64,
}

impl Default for SourcePosition {
    fn default() -> Self {
        Self {
            offset: 0,
            line: 1,
            column: 1,
        }
    }
}

/// Half-open range in the original byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    Io,
    UnexpectedToken,
    InvalidCharacter,
    UnterminatedComment,
    UnterminatedString,
    InvalidEscape,
    InvalidNumber,
    NumericRange,
    InvalidBinary,
    InvalidIdentifier,
    InvalidHeader,
    InvalidComplexInstance,
    DuplicateId,
    DuplicateAnchor,
    MissingReference,
    ExternalReference,
    SignatureUnverified,
    InvalidSignature,
    LimitExceeded,
    UnsupportedFeature,
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Io => "TS1001",
            Self::UnexpectedToken => "TS1002",
            Self::InvalidCharacter => "TS1003",
            Self::UnterminatedComment => "TS1004",
            Self::UnterminatedString => "TS1005",
            Self::InvalidEscape => "TS1006",
            Self::InvalidNumber => "TS1007",
            Self::NumericRange => "TS1008",
            Self::InvalidBinary => "TS1009",
            Self::InvalidIdentifier => "TS1010",
            Self::InvalidHeader => "TS1011",
            Self::InvalidComplexInstance => "TS1012",
            Self::DuplicateId => "TS1101",
            Self::DuplicateAnchor => "TS1102",
            Self::MissingReference => "TS1103",
            Self::ExternalReference => "TS1104",
            Self::SignatureUnverified => "TS1105",
            Self::InvalidSignature => "TS1013",
            Self::LimitExceeded => "TS1201",
            Self::UnsupportedFeature => "TS1301",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub entity: Option<EntityId>,
    pub source_span: SourceSpan,
}

impl Diagnostic {
    pub fn error(
        code: DiagnosticCode,
        message: impl Into<String>,
        source_span: SourceSpan,
    ) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            entity: None,
            source_span,
        }
    }
    pub fn warning(
        code: DiagnosticCode,
        message: impl Into<String>,
        source_span: SourceSpan,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(code, message, source_span)
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:?} at {}:{}: {}",
            self.code.as_str(),
            self.severity,
            self.source_span.start.line,
            self.source_span.start.column,
            self.message
        )
    }
}
impl std::error::Error for Diagnostic {}
