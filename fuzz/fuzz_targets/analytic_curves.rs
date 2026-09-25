#![no_main]
#[path = "../../crates/tessstep-curves/tests/support/exercise.rs"]
mod exercise;
libfuzzer_sys::fuzz_target!(|data: &[u8]| exercise::exercise(data));
