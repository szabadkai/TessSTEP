//! Shared bounded harness used by deterministic replay and libFuzzer.
use tessstep_part21::{Lexer, Parser, TokenKind};
#[path = "limits.rs"]
mod budgets;
use budgets::limits;
pub fn exercise(data: &[u8]) {
    let mut lexer = Lexer::new(data, limits());
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
    }
    for result in Parser::new(data, limits()).expect("fixed safe limits") {
        if result.is_err() {
            break;
        }
    }
    if let Ok(doc) = tessstep_model::parse(data, limits()) {
        std::hint::black_box(doc.diagnostics());
    }
}
