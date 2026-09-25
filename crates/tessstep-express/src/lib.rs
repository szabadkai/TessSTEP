//! Bounded EXPRESS syntax and basic schema validation, independent of STEP instances.
//!
//! Expressions are retained verbatim, not evaluated or type checked. Successful
//! compilation establishes only the structural checks documented in `docs/EXPRESS.md`.
//! ```
//! use tessstep_express::{compile, Limits, Source};
//! let result = compile(&[Source { name: "example.exp", text:
//!     "SCHEMA example; ENTITY point; x : REAL; END_ENTITY; END_SCHEMA;" }], Limits::default());
//! assert!(result.ir.is_some());
//! assert!(result.diagnostics.is_empty());
//! ```
#![forbid(unsafe_code)]

mod ast;
mod lexer;
mod parser;
mod semantic;
pub use ast::*;
pub use lexer::{Token, TokenKind, lex};
pub use parser::parse;
pub use semantic::*;

/// Source identifiers are indices into the source array passed to `compile`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub source: usize,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Default for Span {
    fn default() -> Self {
        Self {
            source: 0,
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub span: Span,
    pub message: String,
}
impl Diagnostic {
    pub(crate) fn error(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            span,
            message: message.into(),
        }
    }
    pub(crate) fn unsupported(span: Span, message: impl Into<String>) -> Self {
        Self {
            code: "EX2001",
            severity: Severity::Unsupported,
            span,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {:?} at {}:{}:{}: {}",
            self.code,
            self.severity,
            self.span.source,
            self.span.line,
            self.span.column,
            self.message
        )
    }
}
impl std::error::Error for Diagnostic {}

/// Logical budgets, not an RSS cap. Nesting is also hard-capped at 128.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_tokens: usize,
    pub max_token_bytes: usize,
    pub max_nesting: usize,
    pub max_schemas: usize,
    pub max_declarations: usize,
    pub max_work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 16 * 1024 * 1024,
            max_tokens: 1_000_000,
            max_token_bytes: 1024 * 1024,
            max_nesting: 64,
            max_schemas: 256,
            max_declarations: 100_000,
            max_work: 5_000_000,
        }
    }
}
pub(crate) fn limit(span: Span, what: &str) -> Diagnostic {
    Diagnostic::error("EX1001", span, format!("resource limit: {what}"))
}
