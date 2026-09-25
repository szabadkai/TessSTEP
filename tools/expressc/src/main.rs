//! Application boundary for explicitly supplied EXPRESS sources.
#![forbid(unsafe_code)]
use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    process::ExitCode,
};
use tessstep_express::{Compilation, IrKind, Limits, Severity, Source, Span, compile};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("expressc: {e}");
            ExitCode::from(2)
        }
    }
}
fn run() -> Result<u8, String> {
    let mut json = false;
    let mut strict = false;
    let mut ast = false;
    let mut rust = false;
    let mut validator = false;
    let mut generation_limits = tessstep_codegen::Limits::default();
    let mut paths = Vec::new();
    let mut limits = Limits::default();
    let mut args = env::args().skip(1);
    let mut positional = false;
    while let Some(arg) = args.next() {
        if positional {
            paths.push(arg);
            continue;
        }
        match arg.as_str() {
            "--" => positional = true,
            "--help" | "-h" => {
                println!(
                    "Usage: expressc [--json | --ast | --rust | --validator] [--strict] [--max-bytes N] [--max-tokens N] [--max-work N] [--max-nesting N] [--max-output-bytes N] [--] FILE.exp ...\nCompile explicitly supplied schemas. Expressions remain opaque. --rust emits bindings and reflection to stdout. --validator emits a standalone schema validator source. --strict rejects unsupported semantics.\nExit: 0 structural success; 1 syntax/semantic/generation/strict failure; 2 usage or I/O failure."
                );
                return Ok(0);
            }
            "--json" => json = true,
            "--strict" => strict = true,
            "--ast" => ast = true,
            "--rust" => rust = true,
            "--validator" => validator = true,
            "--max-bytes" | "--max-tokens" | "--max-work" | "--max-nesting"
            | "--max-output-bytes" => {
                let n = args
                    .next()
                    .ok_or_else(|| format!("{arg} requires a nonnegative integer"))?
                    .parse::<usize>()
                    .map_err(|_| format!("invalid value for {arg}"))?;
                match arg.as_str() {
                    "--max-bytes" => limits.max_input_bytes = n,
                    "--max-tokens" => limits.max_tokens = n,
                    "--max-work" => {
                        limits.max_work = n;
                        generation_limits.max_work = n;
                    }
                    "--max-output-bytes" => generation_limits.max_output_bytes = n,
                    _ => limits.max_nesting = n,
                }
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}")),
            _ => paths.push(arg),
        }
    }
    if paths.is_empty() {
        return Err("supply one or more EXPRESS files; see --help".into());
    }
    if u8::from(json) + u8::from(ast) + u8::from(rust) + u8::from(validator) > 1 {
        return Err("--json, --ast, --rust and --validator are mutually exclusive".into());
    }
    if paths.len() > limits.max_schemas {
        return Err("source count exceeds schema limit".into());
    }
    let mut texts = Vec::new();
    let mut remaining = limits.max_input_bytes;
    for path in &paths {
        let mut text = String::new();
        File::open(path)
            .map_err(|e| format!("{path}: {e}"))?
            .take((remaining as u64).saturating_add(1))
            .read_to_string(&mut text)
            .map_err(|e| format!("{path}: {e}"))?;
        if text.len() > remaining {
            return Err("source set exceeds input byte limit".into());
        }
        remaining -= text.len();
        texts.push(text);
    }
    let sources: Vec<_> = paths
        .iter()
        .zip(&texts)
        .map(|(name, text)| Source { name, text })
        .collect();
    let result = compile(&sources, limits);
    let success = result.ir.is_some() && (!strict || result.diagnostics.is_empty());
    let output = if rust || validator {
        if success {
            let generate = if validator {
                tessstep_codegen::generate_validator
            } else {
                tessstep_codegen::generate
            };
            match generate(&result, generation_limits) {
                Ok(source) => source,
                Err(e) => {
                    eprintln!("expressc: {e}");
                    return Ok(1);
                }
            }
        } else {
            String::new()
        }
    } else if json {
        json_output(&result, success)
    } else if ast {
        format!("{:#?}\n", result.schemas)
    } else {
        let declarations: usize = result.schemas.iter().map(|s| s.declarations.len()).sum();
        format!(
            "{}: {} schemas, {} declarations, {} unsupported diagnostics\n",
            if success {
                "structural checks passed"
            } else {
                "compilation failed"
            },
            result.schemas.len(),
            declarations,
            result
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Unsupported)
                .count()
        )
    };
    if !json {
        for d in &result.diagnostics {
            let path = result
                .sources
                .get(d.span.source)
                .map_or("<input>", String::as_str);
            eprintln!(
                "{path}:{}:{}: {} {:?}: {}",
                d.span.line, d.span.column, d.code, d.severity, d.message
            );
        }
    }
    io::stdout()
        .lock()
        .write_all(output.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(if success { 0 } else { 1 })
}
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn span(s: Span) -> String {
    format!(
        "{{\"source\":{},\"start\":{},\"end\":{},\"line\":{},\"column\":{}}}",
        s.source, s.start, s.end, s.line, s.column
    )
}
fn json_output(c: &Compilation, success: bool) -> String {
    let diagnostics = c
        .diagnostics
        .iter()
        .map(|d| {
            format!(
                "{{\"code\":{},\"severity\":{},\"span\":{},\"message\":{}}}",
                quote(d.code),
                quote(if d.severity == Severity::Error {
                    "error"
                } else {
                    "unsupported"
                }),
                span(d.span),
                quote(&d.message)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let schemas = c.ir.as_ref().map(|ir| ir.schemas.iter().map(|s| {
        let symbols = s.symbols.iter().map(|(name,id)| format!("{}:{}", quote(name), id.0)).collect::<Vec<_>>().join(",");
        let exports = s.exports.iter().map(|(name,id)| format!("{}:{}", quote(name), id.0)).collect::<Vec<_>>().join(",");
        format!("{{\"name\":{},\"span\":{},\"dependencies\":{:?},\"symbols\":{{{symbols}}},\"exports\":{{{exports}}}}}", quote(&s.name), span(s.span), s.dependencies)
    }).collect::<Vec<_>>().join(",")).unwrap_or_default();
    let declarations =
        c.ir.as_ref()
            .map(|ir| {
                ir.declarations
                    .iter()
                    .enumerate()
                    .map(|(id, d)| {
                        format!(
                            "{{\"id\":{id},\"schema\":{},\"name\":{},\"span\":{},\"kind\":{}}}",
                            d.schema,
                            quote(&d.name),
                            span(d.span),
                            quote(match d.kind {
                                IrKind::Entity { .. } => "entity",
                                IrKind::Type(_) => "type",
                                IrKind::Constant(_) => "constant",
                                IrKind::Unsupported => "unsupported",
                            })
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
    format!(
        "{{\"format_version\":1,\"success\":{success},\"structural_valid\":{},\"expression_semantics\":\"not_implemented\",\"sources\":[{}],\"schemas\":[{schemas}],\"declarations\":[{declarations}],\"diagnostics\":[{diagnostics}]}}\n",
        c.ir.is_some(),
        c.sources
            .iter()
            .map(|s| quote(s))
            .collect::<Vec<_>>()
            .join(",")
    )
}
