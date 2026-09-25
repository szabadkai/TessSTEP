#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "../../crates/tessstep-tessellate/tests/support/pipeline_exercise.rs"]
mod pipeline;
fuzz_target!(|data: &[u8]| pipeline::exercise(data));
