#[path = "support/pipeline_exercise.rs"]
mod pipeline;
#[test]
fn topology_trim_sampling_bounded_mutation_smoke() {
    let mut state = 0xd875_4673_8246_1109u64;
    for _ in 0..3000 {
        let mut data = [0u8; 96];
        for b in &mut data {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *b = state as u8;
        }
        pipeline::exercise(&data);
    }
    for kind in 0..4 {
        let mut data = [0u8; 96];
        data[0] = kind;
        for n in 0..=96 {
            pipeline::exercise(&data[..n]);
        }
    }
}
