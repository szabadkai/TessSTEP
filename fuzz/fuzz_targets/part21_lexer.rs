#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "../support/lexer.rs"]
mod support;
fuzz_target!(|data: &[u8]| support::exercise(data));
