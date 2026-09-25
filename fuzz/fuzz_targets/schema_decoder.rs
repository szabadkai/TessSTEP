#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "../support/decoder.rs"]
mod decoder;
fuzz_target!(|data: &[u8]| decoder::exercise(data));
