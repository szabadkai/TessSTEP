//! Inspect physical STEP data without making schema or geometry claims.
#![forbid(unsafe_code)]
mod output;
use std::{
    env,
    fs::File,
    io::{self, BufReader, Write},
    process::ExitCode,
};
use tessstep_part21::{Diagnostic, ParseLimits, Severity};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stepdump: {error}");
            ExitCode::from(2)
        }
    }
}
fn run() -> io::Result<u8> {
    let mut path = None;
    let mut json = false;
    for arg in env::args_os().skip(1) {
        if arg == "--json" {
            json = true;
        } else if arg == "--help" || arg == "-h" {
            println!(
                "Usage: stepdump [--json] <model.step|->\nInspects physical syntax, headers, entity counts and references.\nExit codes: 0 clean, 1 parse/reference errors, 2 usage/I/O errors.\nNo schema, product, unit, or geometry interpretation is performed."
            );
            return Ok(0);
        } else if path.is_none() && (arg == "-" || !arg.to_string_lossy().starts_with('-')) {
            path = Some(arg);
        } else {
            eprintln!("Usage: stepdump [--json] <model.step|->");
            return Ok(2);
        }
    }
    let Some(path) = path else {
        eprintln!("Usage: stepdump [--json] <model.step|->");
        return Ok(2);
    };
    let reader: Box<dyn io::BufRead> = if path == "-" {
        Box::new(BufReader::new(io::stdin()))
    } else {
        Box::new(BufReader::new(File::open(path)?))
    };
    let parsed = tessstep_model::parse(reader, ParseLimits::default());
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let diagnostics: Vec<Diagnostic> = match &parsed {
        Ok(doc) => doc.diagnostics(),
        Err(error) => vec![error.clone()],
    };
    if json {
        output::json(&mut stdout, parsed.as_ref().ok(), &diagnostics)?;
    } else {
        output::text(&mut stdout, parsed.as_ref().ok(), &diagnostics)?;
    }
    stdout.flush()?;
    Ok(u8::from(
        diagnostics.iter().any(|d| d.severity == Severity::Error),
    ))
}
