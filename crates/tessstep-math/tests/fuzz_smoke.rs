#[path = "support/exercise.rs"]
mod exercise;
#[test]
fn math_arbitrary_float_fuzz_smoke() {
    let mut state = 0xbb67ae8584caa73b_u64;
    let mut bytes = [0; 144];
    for _ in 0..10_000 {
        for part in bytes.chunks_exact_mut(8) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            part.copy_from_slice(&state.to_le_bytes());
        }
        exercise::exercise(&bytes);
    }
    for bits in [
        0_u64,
        1,
        u64::MAX,
        f64::MAX.to_bits(),
        f64::INFINITY.to_bits(),
        (-0_f64).to_bits(),
    ] {
        for part in bytes.chunks_exact_mut(8) {
            part.copy_from_slice(&bits.to_le_bytes());
        }
        for len in 0..=bytes.len() {
            exercise::exercise(&bytes[..len]);
        }
    }
}
