#[path = "../../../fuzz/support/express.rs"]
mod support;
#[test]
fn express_fuzz_smoke() {
    let seeds: &[&[u8]] = &[
        include_bytes!("../../../corpus/express/valid/base.exp"),
        include_bytes!("../../../corpus/express/valid/imports.exp"),
        include_bytes!("../../../corpus/express/valid/opaque.exp"),
        include_bytes!("../../../corpus/express/invalid/truncated.exp"),
        include_bytes!("../../../corpus/express/invalid/unbalanced.exp"),
        include_bytes!("../../../corpus/express/invalid/semantic.exp"),
    ];
    let mut cases = 0;
    for seed in seeds {
        for end in 0..=seed.len() {
            support::exercise(&seed[..end]);
            cases += 1;
        }
        for index in 0..seed.len() {
            for byte in [b';', b'(', b'\'', b'\n', 0, 255] {
                let mut data = seed.to_vec();
                data[index] = byte;
                support::exercise(&data);
                cases += 1;
            }
        }
    }
    let mut state = 0x7d56ab23u64;
    for len in 0..512 {
        let data: Vec<_> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state & 127) as u8
            })
            .collect();
        support::exercise(&data);
        cases += 1;
    }
    println!("EXPRESS deterministic fuzz smoke: {cases} cases");
}
