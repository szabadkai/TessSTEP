use tessstep_express::{Limits, Source, compile, lex};
pub fn exercise(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let limits = Limits {
        max_input_bytes: 8192,
        max_tokens: 2048,
        max_token_bytes: 1024,
        max_nesting: 16,
        max_schemas: 8,
        max_declarations: 128,
        max_work: 8192,
    };
    let _ = lex(0, text, limits);
    let sources = [Source {
        name: "fuzz.exp",
        text,
    }];
    let result = compile(&sources, limits);
    assert_eq!(result, compile(&sources, limits));
    for d in &result.diagnostics {
        assert!(d.span.start <= d.span.end && d.span.end <= text.len());
        assert_eq!(d.span.source, 0);
    }
    if let Some(ir) = result.ir {
        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity == tessstep_express::Severity::Unsupported)
        );
        for schema in ir.schemas {
            for id in schema.symbols.values() {
                assert!(id.0 < ir.declarations.len());
            }
        }
    }
}
