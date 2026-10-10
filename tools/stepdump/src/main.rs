//! Inspect physical STEP data without making schema or geometry claims.
#![forbid(unsafe_code)]
mod output;
use std::{
    env,
    fs::File,
    io::{self, BufReader, Write},
    process::ExitCode,
};
use tessstep_part21::{Conformance, Diagnostic, ParseLimits, Severity};

const USAGE: &str =
    "Usage: stepdump [--json] [--strict] [--default-budgets] [--max-<budget>=N ...] <model.step|->";

/// Every parse budget by its flag name, as `--max-<name>=N`.
const BUDGETS: [&str; 10] = [
    "input-bytes",
    "token-bytes",
    "string-bytes",
    "entities",
    "nesting-depth",
    "aggregate-elements",
    "total-values",
    "symbols",
    "records",
    "sections",
];

fn budget_help(l: &ParseLimits) -> String {
    let values = [
        l.max_input_bytes,
        l.max_token_bytes as u64,
        l.max_string_bytes as u64,
        l.max_entities as u64,
        l.max_nesting_depth as u64,
        l.max_aggregate_elements as u64,
        l.max_total_values as u64,
        l.max_symbols as u64,
        l.max_records as u64,
        l.max_sections as u64,
    ];
    BUDGETS
        .iter()
        .zip(values)
        .map(|(name, value)| format!("--max-{name}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Applies `<name>=<value>`; false for an unknown name or an unparsable value.
fn set_budget(limits: &mut ParseLimits, budget: &str) -> bool {
    let Some((name, value)) = budget.split_once('=') else {
        return false;
    };
    let Ok(value) = value.parse::<u64>() else {
        return false;
    };
    let Ok(count) = usize::try_from(value) else {
        return false;
    };
    match name {
        "input-bytes" => limits.max_input_bytes = value,
        "token-bytes" => limits.max_token_bytes = count,
        "string-bytes" => limits.max_string_bytes = count,
        "entities" => limits.max_entities = count,
        "nesting-depth" => limits.max_nesting_depth = count,
        "aggregate-elements" => limits.max_aggregate_elements = count,
        "total-values" => limits.max_total_values = count,
        "symbols" => limits.max_symbols = count,
        "records" => limits.max_records = count,
        "sections" => limits.max_sections = count,
        _ => return false,
    }
    true
}

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
    let mut conformance = Conformance::Tolerant;
    let mut sized = true;
    let mut budgets = Vec::new();
    for arg in env::args_os().skip(1) {
        if arg == "--json" {
            json = true;
        } else if arg == "--strict" {
            conformance = Conformance::Strict;
        } else if arg == "--default-budgets" {
            sized = false;
        } else if arg == "--help" || arg == "-h" {
            println!(
                "{USAGE}\nInspects physical syntax, headers, entity counts and references.\nExit codes: 0 clean, 1 parse/reference errors, 2 usage/I/O errors.\nNo schema, product, unit, or geometry interpretation is performed.\nThe parse accepts counted exporter slips (string backslashes that start no\ndirective, `$` header strings) and reports them as warnings; --strict rejects them.\nThe budgets of a file are sized from its length (ParseLimits::for_input); standard\ninput and --default-budgets use the defaults. --max-<budget>=N then sets one budget.\nBudgets (default): {}",
                budget_help(&ParseLimits::default())
            );
            return Ok(0);
        } else if let Some(budget) = arg.to_str().and_then(|a| a.strip_prefix("--max-")) {
            if !set_budget(&mut ParseLimits::default(), budget) {
                eprintln!("stepdump: unknown or invalid budget --max-{budget}; see --help");
                return Ok(2);
            }
            budgets.push(budget.to_owned());
        } else if path.is_none() && (arg == "-" || !arg.to_string_lossy().starts_with('-')) {
            path = Some(arg);
        } else {
            eprintln!("{USAGE}");
            return Ok(2);
        }
    }
    let Some(path) = path else {
        eprintln!("{USAGE}");
        return Ok(2);
    };
    let mut limits = ParseLimits::default();
    let reader: Box<dyn io::BufRead> = if path == "-" {
        Box::new(BufReader::new(io::stdin()))
    } else {
        let file = File::open(path)?;
        if sized {
            limits = ParseLimits::for_input(file.metadata()?.len());
        }
        Box::new(BufReader::new(file))
    };
    for budget in &budgets {
        set_budget(&mut limits, budget);
    }
    let parsed = tessstep_model::parse_with(reader, limits, conformance);
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
