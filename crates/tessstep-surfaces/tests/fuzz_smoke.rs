#[path = "support/exercise.rs"]
mod exercise;
#[test]
fn surface_bounded_arbitrary_float_smoke() {
    let mut state = 0x9b05688c2b3e6c1f_u64;
    let mut bytes = [0; 128];
    for _ in 0..5000 {
        for part in bytes.chunks_exact_mut(8) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            part.copy_from_slice(&state.to_le_bytes());
        }
        exercise::exercise(&bytes);
    }
    let seed: [f64; 16] = [
        0., 0., 0., 0., 1., 0., 1., 0., 0., 1., 1., 1., 1., 2., 3., 4.,
    ];
    for (part, v) in bytes.chunks_exact_mut(8).zip(seed) {
        part.copy_from_slice(&v.to_le_bytes());
    }
    for i in 0..bytes.len() {
        for byte in [0, 1, 127, 255] {
            let mut changed = bytes;
            changed[i] = byte;
            exercise::exercise(&changed);
        }
    }
    for len in 0..=bytes.len() {
        exercise::exercise(&bytes[..len]);
    }
}
