//! Schema selection: the application protocol a document declares.
use super::*;

/// An application protocol recognized from a declared FILE_SCHEMA name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// `CONFIG_CONTROL_DESIGN` (ISO 10303-203 edition 1).
    Ap203,
    /// The AP203 edition 2 MIM long form.
    Ap203e2,
    /// `AUTOMOTIVE_DESIGN` (ISO 10303-214).
    Ap214,
    /// The AP242 MIM long form.
    Ap242,
}

/// The first FILE_SCHEMA entry of a document and the protocol it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredSchema<'a> {
    /// The entry as written, including any object identifier.
    pub declared: &'a str,
    /// The name without its object identifier, in upper case.
    pub name: String,
    /// None for every other name, including the `AUTOMOTIVE_DESIGN_CC*` conformance
    /// class schemas: names are matched exactly, never fuzzily.
    pub protocol: Option<Protocol>,
    /// FILE_SCHEMA entries in the header.
    pub count: usize,
}

/// The table the corpus schema stage uses (`corpus/ap-schema-map.json`).
const PROTOCOLS: [(&str, Protocol); 4] = [
    ("CONFIG_CONTROL_DESIGN", Protocol::Ap203),
    (
        "AP203_CONFIGURATION_CONTROLLED_3D_DESIGN_OF_MECHANICAL_PARTS_AND_ASSEMBLIES_MIM_LF",
        Protocol::Ap203e2,
    ),
    ("AUTOMOTIVE_DESIGN", Protocol::Ap214),
    (
        "AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF",
        Protocol::Ap242,
    ),
];

/// Read the document's FILE_SCHEMA header. None when the header has no schema
/// entry. Selecting a protocol is not validation against it: TessSTEP imports
/// through bounded original profiles, and no ISO schema is bundled.
pub fn declared_schema(document: &Document) -> Option<DeclaredSchema<'_>> {
    let record = document
        .headers()
        .iter()
        .find(|r| r.name.as_ref().eq_ignore_ascii_case("FILE_SCHEMA"))?;
    let ValueKind::Aggregate(entries) = &record.parameters.first()?.kind else {
        return None;
    };
    let ValueKind::String(declared) = &entries.first()?.kind else {
        return None;
    };
    let name = declared
        .trim()
        .split(|c: char| c.is_whitespace() || c == '{')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    let protocol = PROTOCOLS.iter().find(|(n, _)| *n == name).map(|&(_, p)| p);
    Some(DeclaredSchema {
        declared: declared.as_ref(),
        name,
        protocol,
        count: entries.len(),
    })
}
