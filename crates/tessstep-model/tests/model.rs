use std::{fs, io::BufReader, path::Path};
use tessstep_model::*;
use tessstep_part21::*;
fn file(body: &str) -> String {
    include_str!("../../../corpus/part21/valid/empty.step")
        .replace("DATA;", &format!("DATA;{body}"))
}
fn parse_text(text: &str) -> Result<Document, Diagnostic> {
    parse(text.as_bytes(), ParseLimits::default())
}
#[test]
fn resolves_forward_backward_self_and_cyclic_references() {
    let doc = parse_text(&file("#5=A(#2,#5);#2=B(#5);")).unwrap();
    assert_eq!(
        doc.entities()
            .iter()
            .map(|e| e.id.get())
            .collect::<Vec<_>>(),
        [5, 2]
    );
    assert!(
        doc.references()
            .iter()
            .all(|r| r.status == ReferenceStatus::Local)
    );
    assert!(doc.diagnostics().is_empty());
    assert_eq!(doc.entities().counts_by_type()["A"], 1);
}
#[test]
fn unresolved_uses_retain_owner_and_exact_parameter_span() {
    let input = file("#7=A((#900,TYPE(#901)),#900);");
    let doc = parse_text(&input).unwrap();
    let references = doc.references();
    assert_eq!(references.len(), 3);
    for usage in &references {
        assert_eq!(usage.owner, EntityId::new(7));
        assert_eq!(usage.status, ReferenceStatus::Missing);
        assert_eq!(
            &input[usage.source.start.offset as usize..usage.source.end.offset as usize],
            usage.target.to_string()
        );
    }
    assert_eq!(doc.diagnostics().len(), 3);
}
#[test]
fn duplicate_ids_never_overwrite_even_across_sections_or_namespaces() {
    assert_eq!(
        parse_text(&file("#1=A();#01=B();")).unwrap_err().code,
        DiagnosticCode::DuplicateId
    );
    let extended = include_str!("../../../corpus/part21/valid/extended.step");
    assert_eq!(
        parse_text(&extended.replace("#1=NODE", "#2=NODE"))
            .unwrap_err()
            .code,
        DiagnosticCode::DuplicateId
    );
    assert_eq!(
        parse_text(&extended.replace("#3=<", "#2=<"))
            .unwrap_err()
            .code,
        DiagnosticCode::DuplicateId
    );
    assert_eq!(
        parse_text(&extended.replace("<unit>=@2", "<part>=@2"))
            .unwrap_err()
            .code,
        DiagnosticCode::DuplicateAnchor
    );
    let multiple = include_str!("../../../corpus/part21/valid/multiple.step");
    assert_eq!(
        parse_text(&multiple.replace("#1=B", "#2=B"))
            .unwrap_err()
            .code,
        DiagnosticCode::DuplicateId
    );
}
#[test]
fn external_references_and_signatures_are_not_trusted() {
    let doc = parse_text(include_str!("../../../corpus/part21/valid/extended.step")).unwrap();
    assert_eq!(doc.signatures()[0].base64, "TWE=");
    assert_eq!(
        doc.references()
            .iter()
            .filter(|r| r.status == ReferenceStatus::External)
            .count(),
        3
    );
    assert_eq!(
        doc.diagnostics()
            .iter()
            .filter(|d| d.code == DiagnosticCode::SignatureUnverified)
            .count(),
        1
    );
    assert_eq!(
        doc.diagnostics()
            .iter()
            .filter(|d| d.code == DiagnosticCode::ExternalReference)
            .count(),
        2
    );
    assert!(
        doc.diagnostics()
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );
}
#[test]
fn multiple_sections_and_complex_type_counts() {
    let doc = parse_text(include_str!("../../../corpus/part21/valid/multiple.step")).unwrap();
    assert_eq!(doc.data_sections()[0].entities, 0..1);
    assert_eq!(doc.data_sections()[1].entities, 1..2);
    assert!(doc.diagnostics().is_empty());
    let doc = parse_text(include_str!("../../../corpus/part21/valid/complex.step")).unwrap();
    assert_eq!(doc.entities().len(), 2);
    assert_eq!(
        doc.entities()
            .counts_by_type()
            .into_iter()
            .collect::<Vec<_>>(),
        [("A", 2), ("B", 1), ("C", 1)]
    );
}
#[test]
fn corpus_regression_and_determinism() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/part21");
    for entry in fs::read_dir(root.join("valid")).unwrap() {
        let path = entry.unwrap().path();
        let data = fs::read(&path).unwrap();
        let first = parse(data.as_slice(), ParseLimits::default())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let second = parse(
            BufReader::with_capacity(1, data.as_slice()),
            ParseLimits::default(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.diagnostics(), second.diagnostics());
    }
    for entry in fs::read_dir(root.join("invalid")).unwrap() {
        let path = entry.unwrap().path();
        let data = fs::read(&path).unwrap();
        assert!(
            parse(data.as_slice(), ParseLimits::default()).is_err(),
            "{}",
            path.display()
        );
    }
}
#[test]
fn huge_ids_remain_sparse() {
    let doc = parse_text(&file("#18446744073709551615=A();")).unwrap();
    assert_eq!(doc.entities().len(), 1);
    assert!(
        doc.entities()
            .get(EntityId::new(u64::MAX).unwrap())
            .is_some()
    );
    assert!(doc.entities().get(EntityId::new(1).unwrap()).is_none());
}
#[test]
fn generated_reference_graphs_are_deterministic_and_complete() {
    for size in [1, 2, 13, 128, 1024] {
        let mut body = String::new();
        for id in (1..=size).rev() {
            body.push_str(&format!("#{id}=NODE((#{},#{}));", id % size + 1, size + 1));
        }
        let input = file(&body);
        let doc = parse_text(&input).unwrap();
        assert_eq!(doc.entities().len(), size);
        assert_eq!(doc.diagnostics().len(), size);
        assert_eq!(doc.references().len(), 2 * size);
        assert_eq!(doc, parse_text(&input).unwrap());
    }
}
