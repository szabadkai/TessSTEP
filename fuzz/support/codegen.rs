use tessstep_codegen::{Error, Limits, generate};
use tessstep_express::{Source, compile};
pub fn exercise(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let compilation = compile(
        &[Source {
            name: "fuzz.exp",
            text,
        }],
        tessstep_express::Limits {
            max_input_bytes: 8192,
            max_tokens: 2048,
            max_token_bytes: 1024,
            max_nesting: 16,
            max_schemas: 8,
            max_declarations: 128,
            max_work: 8192,
        },
    );
    let limits = Limits {
        max_output_bytes: 256 * 1024,
        max_work: 65536,
    };
    let result = generate(&compilation, limits);
    assert_eq!(result, generate(&compilation, limits));
    if compilation.ir.is_none() {
        assert_eq!(result, Err(Error::InvalidCompilation));
    } else {
        assert!(!matches!(
            result,
            Err(Error::InvalidCompilation | Error::NestingLimit)
        ));
    }
    if let Ok(source) = result {
        assert!(source.len() <= limits.max_output_bytes);
    }
}
