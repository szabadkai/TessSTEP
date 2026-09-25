//! Generic entity storage, reference analysis and explicit schema decoding.
#![forbid(unsafe_code)]

pub mod decode;

use std::{collections::BTreeMap, io::BufRead, ops::Range};
use tessstep_part21::*;

/// Dense insertion-order records indexed by sparse physical IDs. Lookup is
/// O(log n); large IDs never allocate a correspondingly large array.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EntityDatabase {
    entities: Vec<EntityInstance>,
    by_id: BTreeMap<EntityId, usize>,
}
impl EntityDatabase {
    pub fn get(&self, id: EntityId) -> Option<&EntityInstance> {
        self.by_id.get(&id).map(|&slot| &self.entities[slot])
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &EntityInstance> {
        self.entities.iter()
    }
    pub fn len(&self) -> usize {
        self.entities.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
    /// Each complex component contributes one count; totals can exceed entity count.
    pub fn counts_by_type(&self) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for entity in &self.entities {
            for record in entity.kind.records() {
                *counts.entry(record.name.as_ref()).or_default() += 1;
            }
        }
        counts
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct DataSection {
    pub parameters: Vec<StepValue>,
    /// Range into `Document::entities().iter()` in declaration order.
    pub entities: Range<usize>,
    pub source: SourceSpan,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    headers: Vec<Record>,
    entities: EntityDatabase,
    sections: Vec<DataSection>,
    anchors: Vec<Anchor>,
    externals: BTreeMap<Occurrence, ExternalReference>,
    signatures: Vec<Signature>,
}
impl Document {
    pub fn headers(&self) -> &[Record] {
        &self.headers
    }
    pub fn header(&self, name: &str) -> Option<&Record> {
        self.headers.iter().find(|r| r.name.as_ref() == name)
    }
    pub fn entities(&self) -> &EntityDatabase {
        &self.entities
    }
    pub fn data_sections(&self) -> &[DataSection] {
        &self.sections
    }
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }
    pub fn external_references(&self) -> impl Iterator<Item = &ExternalReference> {
        self.externals.values()
    }
    pub fn signatures(&self) -> &[Signature] {
        &self.signatures
    }
    /// Separate, nonrecursive analysis. Cycles and forward references are valid
    /// here: this phase tests existence without interpreting schema constraints.
    pub fn references(&self) -> Vec<ReferenceUse> {
        let mut output = Vec::new();
        let mut collect = |owner, value: &StepValue| {
            let mut stack = vec![value];
            while let Some(value) = stack.pop() {
                let target = match &value.kind {
                    ValueKind::Reference(id) => Some(Occurrence::Entity(*id)),
                    ValueKind::ValueReference(id) => Some(Occurrence::Value(*id)),
                    ValueKind::Aggregate(values) => {
                        stack.extend(values.iter().rev());
                        None
                    }
                    ValueKind::Typed { value, .. } => {
                        stack.push(value);
                        None
                    }
                    _ => None,
                };
                if let Some(target) = target {
                    let status = if self.externals.contains_key(&target) {
                        ReferenceStatus::External
                    } else if matches!(target, Occurrence::Entity(id) if self.entities.get(id).is_some())
                    {
                        ReferenceStatus::Local
                    } else {
                        ReferenceStatus::Missing
                    };
                    output.push(ReferenceUse {
                        owner,
                        target,
                        source: value.source,
                        status,
                    });
                }
            }
        };
        for header in &self.headers {
            for value in &header.parameters {
                collect(None, value);
            }
        }
        for anchor in &self.anchors {
            collect(None, &anchor.value);
            for (_, value) in &anchor.tags {
                collect(None, value);
            }
        }
        for section in &self.sections {
            for value in &section.parameters {
                collect(None, value);
            }
        }
        for entity in &self.entities.entities {
            for record in entity.kind.records() {
                for value in &record.parameters {
                    collect(Some(entity.id), value);
                }
            }
        }
        output.sort_by_key(|usage| usage.source.start.offset);
        output
    }
    /// No external resources are fetched and no signatures are trusted.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut output: Vec<_> = self
            .references()
            .into_iter()
            .filter(|r| r.status == ReferenceStatus::Missing)
            .map(|r| Diagnostic {
                entity: r.owner,
                ..Diagnostic::error(
                    DiagnosticCode::MissingReference,
                    format!("unresolved reference {}", r.target),
                    r.source,
                )
            })
            .collect();
        output.extend(self.externals.values().map(|external| {
            Diagnostic::warning(
                DiagnosticCode::ExternalReference,
                format!(
                    "{} is external; resource resolution has not been performed",
                    external.id
                ),
                external.source,
            )
        }));
        output.extend(self.signatures.iter().map(|signature| {
            Diagnostic::warning(
                DiagnosticCode::SignatureUnverified,
                "signature payload retained; cryptographic verification has not been performed",
                signature.source,
            )
        }));
        output.sort_by_key(|diagnostic| diagnostic.source_span.start.offset);
        output
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceStatus {
    Local,
    External,
    Missing,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceUse {
    pub owner: Option<EntityId>,
    pub target: Occurrence,
    pub source: SourceSpan,
    pub status: ReferenceStatus,
}

/// Collect a syntactically complete physical file. Duplicate declarations are
/// fatal; dangling references are available separately from `diagnostics()`.
pub fn parse(reader: impl BufRead, limits: ParseLimits) -> Result<Document, Diagnostic> {
    let parser = Parser::new(reader, limits)?;
    let mut document = Document {
        headers: Vec::new(),
        entities: EntityDatabase::default(),
        sections: Vec::new(),
        anchors: Vec::new(),
        externals: BTreeMap::new(),
        signatures: Vec::new(),
    };
    let mut ids = BTreeMap::new();
    let mut anchors = BTreeMap::new();
    for event in parser {
        match event? {
            Event::Header(record) => document.headers.push(record),
            Event::Entity(entity) => {
                unique_id(&mut ids, entity.id.get(), entity.source)?;
                document
                    .entities
                    .by_id
                    .insert(entity.id, document.entities.entities.len());
                document.entities.entities.push(entity);
            }
            Event::Anchor(anchor) => {
                if anchors.insert(anchor.name.clone(), anchor.source).is_some() {
                    return Err(Diagnostic::error(
                        DiagnosticCode::DuplicateAnchor,
                        "duplicate anchor name",
                        anchor.source,
                    ));
                }
                document.anchors.push(anchor);
            }
            Event::ExternalReference(reference) => {
                unique_id(&mut ids, reference.id.number(), reference.source)?;
                document.externals.insert(reference.id, reference);
            }
            Event::StartData { parameters, source } => {
                let start = document.entities.len();
                document.sections.push(DataSection {
                    parameters,
                    entities: start..start,
                    source,
                });
            }
            Event::EndData(source) => {
                if let Some(section) = document.sections.last_mut() {
                    section.entities.end = document.entities.len();
                    section.source.end = source.end;
                }
            }
            Event::Signature(signature) => document.signatures.push(signature),
            Event::EndExchange(_) => (),
        }
    }
    Ok(document)
}
fn unique_id(
    ids: &mut BTreeMap<u64, SourceSpan>,
    id: u64,
    source: SourceSpan,
) -> Result<(), Diagnostic> {
    if let Some(previous) = ids.insert(id, source) {
        let mut diagnostic = Diagnostic::error(
            DiagnosticCode::DuplicateId,
            format!(
                "duplicate occurrence {id}; first declared at {}:{}",
                previous.start.line, previous.start.column
            ),
            source,
        );
        diagnostic.entity = EntityId::new(id);
        Err(diagnostic)
    } else {
        Ok(())
    }
}
