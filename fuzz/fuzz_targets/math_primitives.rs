#![no_main]
#[path = "../../crates/tessstep-math/tests/support/exercise.rs"]
mod exercise;
libfuzzer_sys::fuzz_target!(|data: &[u8]| exercise::exercise(data));
