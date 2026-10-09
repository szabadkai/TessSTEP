//! The ABI 1 export record (abi/exports.txt) against the bridge sources and the
//! public header. The 1.0 set is pinned: changing it means editing both the record
//! and this test, deliberately. 1.x releases may only add entries.
use std::{collections::BTreeSet, fs, path::Path};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
/// Size and FNV-1a 64 of the sorted, newline-joined 1.0 symbols.
const ABI_1_0: (usize, u64) = (95, 0x2540_677a_c982_7496);

fn read(path: &str) -> String {
    fs::read_to_string(Path::new(ROOT).join(path)).unwrap()
}
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}
/// The record's entries by version, checked for order and form.
fn record() -> Vec<(String, String)> {
    let entries: Vec<(String, String)> = read("abi/exports.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let (version, symbol) = l.split_once(' ').expect("<version> <symbol>");
            assert!(version.starts_with("1."), "ABI 1 records 1.x versions: {l}");
            assert!(symbol.starts_with("ts_") && !symbol.contains(' '), "{l}");
            (version.to_owned(), symbol.to_owned())
        })
        .collect();
    let symbols: Vec<&String> = entries.iter().map(|(_, s)| s).collect();
    let mut sorted = symbols.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        symbols, sorted,
        "the record is sorted by symbol, without duplicates"
    );
    entries
}
/// Functions the bridge exports unmangled.
fn exported() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(Path::new(ROOT).join("crates/tessstep-capi/src")).unwrap() {
        let source = fs::read_to_string(entry.unwrap().path()).unwrap();
        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim() != "#[unsafe(no_mangle)]" {
                continue;
            }
            let item = lines[i + 1..]
                .iter()
                .find(|l| !l.trim_start().starts_with("#["))
                .unwrap();
            let name = item
                .split("extern \"C\" fn ")
                .nth(1)
                .and_then(|rest| rest.split('(').next())
                .unwrap_or_else(|| panic!("unmangled item is not a C function: {item}"));
            found.insert(name.to_owned());
        }
    }
    found
}
/// Functions the public header declares.
fn declared() -> BTreeSet<String> {
    read("include/tessstep/tessstep.h")
        .split("TS_CALL ")
        .skip(1)
        .filter_map(|rest| rest.split('(').next())
        .filter(|name| name.starts_with("ts_"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn c_abi_export_record_matches_sources_and_header() {
    let entries = record();
    let recorded: BTreeSet<String> = entries.iter().map(|(_, s)| s.clone()).collect();
    let exported = exported();
    assert_eq!(
        exported.difference(&recorded).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "exported but not in abi/exports.txt: add them deliberately, as a 1.x addition"
    );
    assert_eq!(
        recorded.difference(&exported).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "recorded but not exported: ABI 1 never removes a symbol"
    );
    assert_eq!(
        declared(),
        recorded,
        "tessstep.h declares exactly the record"
    );
}

#[test]
fn c_abi_1_0_export_set_is_frozen() {
    let frozen: Vec<String> = record()
        .into_iter()
        .filter(|(v, _)| v == "1.0")
        .map(|(_, s)| s)
        .collect();
    assert_eq!(
        (frozen.len(), fnv1a(&frozen.join("\n"))),
        ABI_1_0,
        "the ABI 1.0 export set changed; it is frozen"
    );
}
