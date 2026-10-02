//! Part 21 lexer under the shared fuzz budgets: tokens until the first error or end.
#[path = "../limits.rs"]
mod budgets;
pub fn exercise(data: &[u8]) {
    let mut lexer = tessstep_part21::Lexer::new(data, budgets::limits());
    while let Ok(token) = lexer.next_token() {
        if token.kind == tessstep_part21::TokenKind::Eof {
            break;
        }
    }
}
