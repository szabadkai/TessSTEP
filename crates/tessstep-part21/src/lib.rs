//! Streaming, schema-independent STEP physical-file syntax.
#![forbid(unsafe_code)]

mod deviations;
mod diagnostic;
mod lexer;
mod limits;
mod parser;
mod strings;
mod value;

pub use deviations::{Conformance, Deviation, Deviations};
pub use diagnostic::*;
pub use lexer::{Lexer, Token, TokenKind};
pub use limits::ParseLimits;
pub use parser::{Event, Parser};
pub use value::*;
