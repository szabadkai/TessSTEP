#[path = "../../../fuzz/harness.rs"]
mod harness;
use std::{fs, path::Path};

#[test]
fn fuzz_corpus_smoke() {
    let mut inputs = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/part21");
    for category in ["valid", "invalid"] {
        let mut files: Vec<_> = fs::read_dir(root.join(category))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        for file in files {
            inputs.push(fs::read(file).unwrap());
        }
    }
    let mut cases = 0;
    for seed in &inputs {
        harness::exercise(seed);
        cases += 1;
        for end in 0..seed.len() {
            harness::exercise(&seed[..end]);
            cases += 1;
        }
        for position in 0..seed.len() {
            for byte in [0, b'(', b')', b'\'', b'\\', b';', 0xff] {
                let mut data = seed.clone();
                data[position] = byte;
                harness::exercise(&data);
                cases += 1;
            }
        }
    }
    let mut random = 0x7465737373746570u64;
    for length in 0..2048 {
        let data: Vec<u8> = (0..length % 256)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                random as u8
            })
            .collect();
        harness::exercise(&data);
        cases += 1;
    }
    println!(
        "fuzz smoke: {cases} deterministic seed, truncation, mutation, and arbitrary-byte cases"
    );
}
