//! Usage: cargo run --release -p tessstep-import --example session -- [FILE...]
//! Runs the session benchmark (`benches/session.rs`) once outside `cargo bench`: the
//! per-stage time and peak memory of a session import and of independent per-root
//! imports of every selected root of each FILE, or without files the generated
//! scaling series and the external corpus inputs it records nightly.
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "../benches/session.rs"]
mod bench;

fn main() {
    bench::run(std::env::args().skip(1).collect());
}
