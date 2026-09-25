#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "../limits.rs"]
mod budgets;
fuzz_target!(|data: &[u8]| {
    let mut lexer = tessstep_part21::Lexer::new(data, budgets::limits());
    while let Ok(token) = lexer.next_token() {
        if token.kind == tessstep_part21::TokenKind::Eof {
            break;
        }
    }
});
