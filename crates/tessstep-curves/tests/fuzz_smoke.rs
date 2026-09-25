#[path = "support/exercise.rs"]
mod exercise;
#[test]
fn analytic_curve_arbitrary_float_smoke() {
    let mut state = 0x3c6ef372fe94f82b_u64;
    let mut bytes = [0; 128];
    for _ in 0..10_000 {
        for part in bytes.chunks_exact_mut(8) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            part.copy_from_slice(&state.to_le_bytes());
        }
        exercise::exercise(&bytes);
    }
    for value in [
        0_f64,
        f64::from_bits(1),
        1.,
        f64::MAX,
        f64::INFINITY,
        f64::NAN,
    ] {
        for part in bytes.chunks_exact_mut(8) {
            part.copy_from_slice(&value.to_le_bytes());
        }
        for len in 0..=bytes.len() {
            exercise::exercise(&bytes[..len]);
        }
    }
}
