use std::io::{self, Write};
use tessstep_model::{Document, ReferenceStatus};
use tessstep_part21::{Diagnostic, StepValue, ValueKind};

pub fn text(
    out: &mut impl Write,
    doc: Option<&Document>,
    diagnostics: &[Diagnostic],
) -> io::Result<()> {
    if let Some(doc) = doc {
        for name in ["FILE_DESCRIPTION", "FILE_NAME", "FILE_SCHEMA"] {
            if let Some(record) = doc.header(name) {
                write!(out, "{name}: ")?;
                values(out, &record.parameters)?;
                writeln!(out)?;
            }
        }
        writeln!(
            out,
            "Entities: {}\nData sections: {}",
            doc.entities().len(),
            doc.data_sections().len()
        )?;
        writeln!(
            out,
            "Entity counts by type (complex components counted individually):"
        )?;
        for (name, count) in doc.entities().counts_by_type() {
            writeln!(out, "  {name}: {count}")?;
        }
        let references = doc.references();
        writeln!(
            out,
            "Unresolved references: {}",
            references
                .iter()
                .filter(|r| r.status == ReferenceStatus::Missing)
                .count()
        )?;
        for usage in references
            .iter()
            .filter(|r| r.status == ReferenceStatus::Missing)
        {
            writeln!(
                out,
                "  {} at {}:{}",
                usage.target, usage.source.start.line, usage.source.start.column
            )?;
        }
    }
    writeln!(out, "Diagnostics: {}", diagnostics.len())?;
    for diagnostic in diagnostics {
        writeln!(out, "  {diagnostic}")?;
    }
    writeln!(
        out,
        "Scope: physical syntax and reference existence only; schema/geometry validation not performed."
    )
}
pub fn json(
    out: &mut impl Write,
    doc: Option<&Document>,
    diagnostics: &[Diagnostic],
) -> io::Result<()> {
    write!(
        out,
        "{{\"format_version\":1,\"scope\":\"physical-syntax\",\"document\":"
    )?;
    if let Some(doc) = doc {
        write!(out, "{{\"headers\":[")?;
        for (i, record) in doc.headers().iter().enumerate() {
            if i != 0 {
                write!(out, ",")?;
            }
            write!(out, "{{\"name\":")?;
            quoted(out, &record.name)?;
            write!(out, ",\"parameters\":")?;
            values(out, &record.parameters)?;
            write!(out, "}}")?;
        }
        write!(
            out,
            "],\"entity_count\":{},\"data_sections\":{},\"entity_counts\":{{",
            doc.entities().len(),
            doc.data_sections().len()
        )?;
        for (i, (name, count)) in doc.entities().counts_by_type().iter().enumerate() {
            if i != 0 {
                write!(out, ",")?;
            }
            quoted(out, name)?;
            write!(out, ":{count}")?;
        }
        write!(out, "}},\"unresolved_references\":[")?;
        for (i, usage) in doc
            .references()
            .iter()
            .filter(|r| r.status == ReferenceStatus::Missing)
            .enumerate()
        {
            if i != 0 {
                write!(out, ",")?;
            }
            write!(out, "{{\"target\":")?;
            quoted(out, &usage.target.to_string())?;
            write!(out, ",\"owner\":")?;
            if let Some(id) = usage.owner {
                quoted(out, &id.to_string())?;
            } else {
                write!(out, "null")?;
            }
            write!(out, ",\"offset\":{}}}", usage.source.start.offset)?;
        }
        write!(out, "]}}")?;
    } else {
        write!(out, "null")?;
    }
    write!(out, ",\"diagnostics\":[")?;
    for (i, diagnostic) in diagnostics.iter().enumerate() {
        if i != 0 {
            write!(out, ",")?;
        }
        write!(out, "{{\"code\":")?;
        quoted(out, diagnostic.code.as_str())?;
        write!(out, ",\"severity\":")?;
        quoted(out, &format!("{:?}", diagnostic.severity).to_lowercase())?;
        write!(out, ",\"message\":")?;
        quoted(out, &diagnostic.message)?;
        write!(out, ",\"entity\":")?;
        if let Some(id) = diagnostic.entity {
            quoted(out, &id.to_string())?;
        } else {
            write!(out, "null")?;
        }
        let span = diagnostic.source_span;
        write!(
            out,
            ",\"span\":{{\"start\":{},\"end\":{},\"line\":{},\"column\":{}}}}}",
            span.start.offset, span.end.offset, span.start.line, span.start.column
        )?;
    }
    writeln!(out, "]}}")
}
fn values(out: &mut impl Write, values: &[StepValue]) -> io::Result<()> {
    write!(out, "[")?;
    for (i, item) in values.iter().enumerate() {
        if i != 0 {
            write!(out, ",")?;
        }
        value(out, item)?;
    }
    write!(out, "]")
}
fn value(out: &mut impl Write, item: &StepValue) -> io::Result<()> {
    match &item.kind {
        ValueKind::String(s) | ValueKind::Resource(s) => quoted(out, s),
        ValueKind::Enumeration(s) | ValueKind::ConstantEntity(s) | ValueKind::ConstantValue(s) => {
            quoted(out, s)
        }
        ValueKind::Null => write!(out, "null"),
        ValueKind::Omitted => quoted(out, "*"),
        ValueKind::Integer(n) => write!(out, "{n}"),
        ValueKind::Real(n) => write!(out, "{n}"),
        ValueKind::Reference(id) => quoted(out, &id.to_string()),
        ValueKind::ValueReference(id) => quoted(out, &format!("@{}", id.get())),
        ValueKind::Aggregate(items) => values(out, items),
        ValueKind::Typed {
            type_name,
            value: item,
        } => {
            write!(out, "{{")?;
            quoted(out, type_name)?;
            write!(out, ":")?;
            value(out, item)?;
            write!(out, "}}")
        }
        ValueKind::Binary(bits) => {
            write!(out, "{{\"bit_len\":{},\"bytes\":[", bits.bit_len)?;
            for (i, byte) in bits.bytes.iter().enumerate() {
                if i != 0 {
                    write!(out, ",")?;
                }
                write!(out, "{byte}")?;
            }
            write!(out, "]}}")
        }
    }
}
fn quoted(out: &mut impl Write, text: &str) -> io::Result<()> {
    write!(out, "\"")?;
    for c in text.chars() {
        match c {
            '"' => write!(out, "\\\"")?,
            '\\' => write!(out, "\\\\")?,
            c if c <= '\u{1f}' => write!(out, "\\u{:04x}", u32::from(c))?,
            c => write!(out, "{c}")?,
        }
    }
    write!(out, "\"")
}
