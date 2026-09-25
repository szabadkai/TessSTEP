#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "../limits.rs"]
mod budgets;
fuzz_target!(|data: &[u8]| {
    for event in tessstep_part21::Parser::new(data, budgets::limits()).unwrap() {
        if event.is_err() {
            break;
        }
    }
});
