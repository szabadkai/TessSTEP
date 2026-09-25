#[path = "../../../fuzz/support/decoder.rs"]
mod decoder;
#[test]
fn schema_decoder_fuzz_smoke() {
    let seeds: &[&[u8]] = &[
        include_bytes!("../../../corpus/schema/simple.step"),
        include_bytes!("../../../corpus/schema/complex.step"),
        include_bytes!("../../../corpus/schema/duplicate.step"),
        include_bytes!("../../../corpus/schema/bad-select.step"),
    ];
    let mut cases = 0;
    for seed in seeds {
        for end in 0..=seed.len() {
            decoder::exercise(&seed[..end]);
            cases += 1;
        }
        for index in 0..seed.len() {
            for byte in [b'0', b'\'', b'$', b'*', b'(', b')', b';', b'X', 255] {
                let mut data = seed.to_vec();
                data[index] = byte;
                decoder::exercise(&data);
                cases += 1;
            }
        }
    }
    println!("schema decoder fuzz smoke: {cases} bounded cases");
}
