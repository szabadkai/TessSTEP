use crate::{lexer::keyword, *};
use std::{collections::BTreeSet, io::BufRead, sync::Arc};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Header(Record),
    Anchor(Anchor),
    ExternalReference(ExternalReference),
    StartData {
        parameters: Vec<StepValue>,
        source: SourceSpan,
    },
    Entity(EntityInstance),
    EndData(SourceSpan),
    Signature(Signature),
    EndExchange(SourceSpan),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Start,
    Header,
    Sections,
    Anchors,
    References,
    Data,
    Signatures,
    Done,
}

/// Pull events without retaining previous records. Entity identity checks and
/// reference resolution belong to `tessstep-model`, not this syntax parser.
#[derive(Debug)]
pub struct Parser<R> {
    lexer: Lexer<R>,
    lookahead: Option<Token>,
    limits: ParseLimits,
    state: State,
    last_end: SourcePosition,
    symbols: BTreeSet<Symbol>,
    values: usize,
    records: usize,
    entities: usize,
    sections: usize,
    headers: usize,
    phase: u8,
    current_entity: Option<EntityId>,
}
impl<R: BufRead> Parser<R> {
    pub fn new(reader: R, limits: ParseLimits) -> Result<Self, Diagnostic> {
        // A configurable depth must not permit callers to disable stack protection.
        if limits.max_nesting_depth > 128 {
            return Err(Diagnostic::error(
                DiagnosticCode::LimitExceeded,
                "max_nesting_depth must not exceed the hard safety cap of 128",
                SourceSpan::default(),
            ));
        }
        Ok(Self {
            lexer: Lexer::new(reader, limits),
            lookahead: None,
            limits,
            state: State::Start,
            last_end: SourcePosition::default(),
            symbols: BTreeSet::new(),
            values: 0,
            records: 0,
            entities: 0,
            sections: 0,
            headers: 0,
            phase: 0,
            current_entity: None,
        })
    }
    fn peek(&mut self) -> Result<&Token, Diagnostic> {
        if self.lookahead.is_none() {
            self.lookahead = Some(self.lexer.next_token()?);
        }
        Ok(self
            .lookahead
            .as_ref()
            .expect("lookahead initialized above"))
    }
    fn take(&mut self) -> Result<Token, Diagnostic> {
        let token = match self.lookahead.take() {
            Some(t) => t,
            None => self.lexer.next_token()?,
        };
        self.last_end = token.source.end;
        Ok(token)
    }
    fn is_word(&mut self, word: &str) -> Result<bool, Diagnostic> {
        Ok(matches!(&self.peek()?.kind, TokenKind::Word(w) if w == word))
    }
    fn expect_word(&mut self, word: &str) -> Result<Token, Diagnostic> {
        let token = self.take()?;
        if matches!(&token.kind, TokenKind::Word(w) if w == word) {
            Ok(token)
        } else {
            Err(self.unexpected(&token, word))
        }
    }
    fn expect(&mut self, kind: TokenKind) -> Result<Token, Diagnostic> {
        let token = self.take()?;
        if token.kind == kind {
            Ok(token)
        } else {
            Err(self.unexpected(&token, &format!("{kind:?}")))
        }
    }
    fn unexpected(&self, token: &Token, expected: &str) -> Diagnostic {
        Diagnostic::error(
            DiagnosticCode::UnexpectedToken,
            format!("expected {expected}; found {}", token_label(&token.kind)),
            token.source,
        )
    }
    fn span(&self, start: SourcePosition) -> SourceSpan {
        SourceSpan {
            start,
            end: self.last_end,
        }
    }
    fn limit(&mut self, count: usize, maximum: usize, name: &str) -> Result<(), Diagnostic> {
        if count >= maximum {
            let span = self.peek()?.source;
            Err(Diagnostic::error(
                DiagnosticCode::LimitExceeded,
                format!("{name} budget exceeded"),
                span,
            ))
        } else {
            Ok(())
        }
    }
    fn intern(&mut self, text: String, span: SourceSpan) -> Result<Symbol, Diagnostic> {
        if let Some(symbol) = self.symbols.get(text.as_str()) {
            return Ok(symbol.clone());
        }
        if self.symbols.len() >= self.limits.max_symbols {
            return Err(Diagnostic::error(
                DiagnosticCode::LimitExceeded,
                "symbol budget exceeded",
                span,
            ));
        }
        let symbol: Symbol = Arc::from(text);
        self.symbols.insert(symbol.clone());
        Ok(symbol)
    }
    fn name(&mut self) -> Result<(Symbol, SourcePosition), Diagnostic> {
        let token = self.take()?;
        match token.kind {
            TokenKind::Word(name) if keyword(&name) => {
                Ok((self.intern(name, token.source)?, token.source.start))
            }
            _ => Err(self.unexpected(&token, "uppercase STEP keyword")),
        }
    }
    fn value(&mut self, depth: usize, anchor: bool) -> Result<StepValue, Diagnostic> {
        self.limit(depth, self.limits.max_nesting_depth, "nesting depth")?;
        self.limit(self.values, self.limits.max_total_values, "total value")?;
        self.values += 1;
        let token = self.take()?;
        let source = token.source;
        let kind = match token.kind {
            TokenKind::Null => ValueKind::Null,
            TokenKind::Omitted if !anchor => ValueKind::Omitted,
            TokenKind::Integer(n) => ValueKind::Integer(n),
            TokenKind::Real(n) => ValueKind::Real(n),
            TokenKind::String(text) => ValueKind::String(text),
            TokenKind::Binary(bits) => ValueKind::Binary(bits),
            TokenKind::Enumeration(text) => ValueKind::Enumeration(self.intern(text, source)?),
            TokenKind::Occurrence(Occurrence::Entity(id)) => ValueKind::Reference(id),
            TokenKind::Occurrence(Occurrence::Value(id)) => ValueKind::ValueReference(id),
            TokenKind::ConstantEntity(text) => {
                ValueKind::ConstantEntity(self.intern(text, source)?)
            }
            TokenKind::ConstantValue(text) => ValueKind::ConstantValue(self.intern(text, source)?),
            TokenKind::Resource(uri) if anchor => ValueKind::Resource(uri),
            TokenKind::LeftParen => ValueKind::Aggregate(self.list(depth + 1, anchor)?),
            TokenKind::Word(name) if !anchor && keyword(&name) => {
                let type_name = self.intern(name, source)?;
                self.expect(TokenKind::LeftParen)?;
                let value = Box::new(self.value(depth + 1, false)?);
                self.expect(TokenKind::RightParen)?;
                ValueKind::Typed { type_name, value }
            }
            _ => return Err(self.unexpected(&token, "parameter")),
        };
        Ok(StepValue {
            kind,
            source: self.span(source.start),
        })
    }
    /// Opening parenthesis has already been consumed.
    fn list(&mut self, depth: usize, anchor: bool) -> Result<Vec<StepValue>, Diagnostic> {
        let mut values = Vec::new();
        if self.peek()?.kind == TokenKind::RightParen {
            self.take()?;
            return Ok(values);
        }
        loop {
            self.limit(
                values.len(),
                self.limits.max_aggregate_elements,
                "aggregate element",
            )?;
            values.push(self.value(depth, anchor)?);
            let separator = self.take()?;
            match separator.kind {
                TokenKind::RightParen => return Ok(values),
                TokenKind::Comma => (),
                _ => return Err(self.unexpected(&separator, "comma or closing parenthesis")),
            }
        }
    }
    fn record(&mut self) -> Result<Record, Diagnostic> {
        self.limit(self.records, self.limits.max_records, "record")?;
        self.records += 1;
        let (name, start) = self.name()?;
        self.expect(TokenKind::LeftParen)?;
        let parameters = self.list(0, false)?;
        Ok(Record {
            name,
            parameters,
            source: self.span(start),
        })
    }
    fn entity(&mut self) -> Result<EntityInstance, Diagnostic> {
        self.limit(self.entities, self.limits.max_entities, "entity")?;
        self.entities += 1;
        let token = self.take()?;
        let TokenKind::Occurrence(Occurrence::Entity(id)) = token.kind else {
            return Err(self.unexpected(&token, "entity instance identifier"));
        };
        self.current_entity = Some(id);
        self.expect(TokenKind::Equals)?;
        let kind = if self.peek()?.kind == TokenKind::LeftParen {
            self.take()?;
            let mut records: Vec<Record> = Vec::new();
            let mut names = BTreeSet::new();
            while self.peek()?.kind != TokenKind::RightParen {
                self.limit(
                    records.len(),
                    self.limits.max_aggregate_elements,
                    "complex component",
                )?;
                let record = self.record()?;
                if !names.insert(record.name.clone()) {
                    return Err(Diagnostic::error(
                        DiagnosticCode::InvalidComplexInstance,
                        "duplicate complex component",
                        record.source,
                    ));
                }
                records.push(record);
            }
            self.take()?;
            if records.is_empty() {
                return Err(Diagnostic::error(
                    DiagnosticCode::InvalidComplexInstance,
                    "complex instance must contain a record",
                    self.span(token.source.start),
                ));
            }
            EntityKind::Complex(records)
        } else {
            EntityKind::Simple(self.record()?)
        };
        self.expect(TokenKind::Semicolon)?;
        let entity = EntityInstance {
            id,
            kind,
            source: self.span(token.source.start),
        };
        self.current_entity = None;
        Ok(entity)
    }
    fn anchor(&mut self) -> Result<Anchor, Diagnostic> {
        self.limit(self.records, self.limits.max_records, "record")?;
        self.records += 1;
        let token = self.take()?;
        let TokenKind::Resource(name) = token.kind else {
            return Err(self.unexpected(&token, "anchor name"));
        };
        if name.is_empty()
            || name.bytes().all(|b| b.is_ascii_digit())
            || name.contains(['#', '[', ']'])
        {
            return Err(Diagnostic::error(
                DiagnosticCode::InvalidIdentifier,
                "invalid anchor fragment identifier",
                token.source,
            ));
        }
        self.expect(TokenKind::Equals)?;
        let value = self.value(0, true)?;
        let mut tags = Vec::new();
        while self.peek()?.kind == TokenKind::LeftBrace {
            self.limit(tags.len(), self.limits.max_aggregate_elements, "anchor tag")?;
            self.take()?;
            let tag = self.take()?;
            let TokenKind::Word(name) = tag.kind else {
                return Err(self.unexpected(&tag, "tag name"));
            };
            if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(Diagnostic::error(
                    DiagnosticCode::InvalidIdentifier,
                    "invalid tag name",
                    tag.source,
                ));
            }
            self.expect(TokenKind::Colon)?;
            let value = self.value(0, true)?;
            self.expect(TokenKind::RightBrace)?;
            tags.push((name, value));
        }
        self.expect(TokenKind::Semicolon)?;
        Ok(Anchor {
            name,
            value,
            tags,
            source: self.span(token.source.start),
        })
    }
    fn external_reference(&mut self) -> Result<ExternalReference, Diagnostic> {
        self.limit(self.records, self.limits.max_records, "record")?;
        self.records += 1;
        let token = self.take()?;
        let TokenKind::Occurrence(id) = token.kind else {
            return Err(self.unexpected(&token, "occurrence identifier"));
        };
        self.expect(TokenKind::Equals)?;
        let resource = self.take()?;
        let TokenKind::Resource(resource) = resource.kind else {
            return Err(self.unexpected(&resource, "resource URI"));
        };
        self.expect(TokenKind::Semicolon)?;
        Ok(ExternalReference {
            id,
            resource,
            source: self.span(token.source.start),
        })
    }
    fn event(&mut self) -> Result<Option<Event>, Diagnostic> {
        loop {
            match self.state {
                State::Start => {
                    self.expect_word("ISO-10303-21")?;
                    self.expect(TokenKind::Semicolon)?;
                    self.expect_word("HEADER")?;
                    self.expect(TokenKind::Semicolon)?;
                    self.section_budget()?;
                    self.state = State::Header;
                }
                State::Header => {
                    if self.is_word("ENDSEC")? {
                        let token = self.take()?;
                        self.expect(TokenKind::Semicolon)?;
                        if self.headers < 3 {
                            return Err(Diagnostic::error(
                                DiagnosticCode::InvalidHeader,
                                "three required header records are missing",
                                token.source,
                            ));
                        }
                        self.state = State::Sections;
                        continue;
                    }
                    let record = self.record()?;
                    self.expect(TokenKind::Semicolon)?;
                    validate_header(&record, self.headers)?;
                    self.headers += 1;
                    return Ok(Some(Event::Header(record)));
                }
                State::Sections => {
                    let token = self.take()?;
                    match &token.kind {
                        TokenKind::Word(w) if w == "ANCHOR" && self.phase == 0 => {
                            self.section_budget()?;
                            self.expect(TokenKind::Semicolon)?;
                            self.phase = 1;
                            self.state = State::Anchors;
                        }
                        TokenKind::Word(w) if w == "REFERENCE" && self.phase <= 1 => {
                            self.section_budget()?;
                            self.expect(TokenKind::Semicolon)?;
                            self.phase = 2;
                            self.state = State::References;
                        }
                        TokenKind::Word(w) if w == "DATA" => {
                            self.section_budget()?;
                            self.phase = 3;
                            let parameters = if self.peek()?.kind == TokenKind::LeftParen {
                                self.take()?;
                                let params = self.list(0, false)?;
                                if params.is_empty() {
                                    return Err(Diagnostic::error(
                                        DiagnosticCode::UnexpectedToken,
                                        "DATA() requires parameters",
                                        self.span(token.source.start),
                                    ));
                                }
                                params
                            } else {
                                Vec::new()
                            };
                            self.expect(TokenKind::Semicolon)?;
                            self.state = State::Data;
                            return Ok(Some(Event::StartData {
                                parameters,
                                source: self.span(token.source.start),
                            }));
                        }
                        TokenKind::Word(w) if w == "END-ISO-10303-21" => {
                            self.expect(TokenKind::Semicolon)?;
                            self.state = State::Signatures;
                            return Ok(Some(Event::EndExchange(self.span(token.source.start))));
                        }
                        _ => {
                            return Err(self.unexpected(
                                &token,
                                "ordered ANCHOR, REFERENCE, DATA, or exchange terminator",
                            ));
                        }
                    }
                }
                State::Anchors | State::References | State::Data => {
                    if self.is_word("ENDSEC")? {
                        let token = self.take()?;
                        self.expect(TokenKind::Semicolon)?;
                        let data = self.state == State::Data;
                        self.state = State::Sections;
                        if data {
                            return Ok(Some(Event::EndData(self.span(token.source.start))));
                        }
                    } else {
                        return Ok(Some(match self.state {
                            State::Anchors => Event::Anchor(self.anchor()?),
                            State::References => {
                                Event::ExternalReference(self.external_reference()?)
                            }
                            _ => Event::Entity(self.entity()?),
                        }));
                    }
                }
                State::Signatures => match self.lexer.next_signature()? {
                    None => {
                        self.state = State::Done;
                        return Ok(None);
                    }
                    Some(signature) => {
                        if self.sections >= self.limits.max_sections {
                            return Err(Diagnostic::error(
                                DiagnosticCode::LimitExceeded,
                                "section budget exceeded",
                                signature.source,
                            ));
                        }
                        self.sections += 1;
                        return Ok(Some(Event::Signature(signature)));
                    }
                },
                State::Done => return Ok(None),
            }
        }
    }
    fn section_budget(&mut self) -> Result<(), Diagnostic> {
        self.limit(self.sections, self.limits.max_sections, "section")?;
        self.sections += 1;
        Ok(())
    }
}
impl<R: BufRead> Iterator for Parser<R> {
    type Item = Result<Event, Diagnostic>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.event() {
            Ok(Some(event)) => Some(Ok(event)),
            Ok(None) => None,
            Err(mut diagnostic) => {
                self.state = State::Done;
                diagnostic.entity = self.current_entity;
                Some(Err(diagnostic))
            }
        }
    }
}
impl<R: BufRead> std::iter::FusedIterator for Parser<R> {}

fn token_label(token: &TokenKind) -> &'static str {
    match token {
        TokenKind::Word(_) => "keyword",
        TokenKind::Integer(_) => "integer",
        TokenKind::Real(_) => "real",
        TokenKind::String(_) => "string",
        TokenKind::Binary(_) => "binary",
        TokenKind::Enumeration(_) => "enumeration",
        TokenKind::Occurrence(_) => "occurrence",
        TokenKind::ConstantEntity(_) | TokenKind::ConstantValue(_) => "constant",
        TokenKind::Resource(_) => "resource",
        TokenKind::LeftParen => "(",
        TokenKind::RightParen => ")",
        TokenKind::Comma => ",",
        TokenKind::Semicolon => ";",
        TokenKind::Equals => "=",
        TokenKind::Null => "$",
        TokenKind::Omitted => "*",
        TokenKind::LeftBrace => "{",
        TokenKind::RightBrace => "}",
        TokenKind::Colon => ":",
        TokenKind::Eof => "EOF",
    }
}
fn validate_header(record: &Record, index: usize) -> Result<(), Diagnostic> {
    let expected = ["FILE_DESCRIPTION", "FILE_NAME", "FILE_SCHEMA"];
    if index >= 3 {
        if expected.contains(&record.name.as_ref()) {
            return Err(Diagnostic::error(
                DiagnosticCode::InvalidHeader,
                "duplicate mandatory header",
                record.source,
            ));
        }
        return Ok(());
    }
    let params = &record.parameters;
    let string = |value: &StepValue| matches!(value.kind, ValueKind::String(_));
    let strings = |value: &StepValue| matches!(&value.kind, ValueKind::Aggregate(v) if !v.is_empty() && v.iter().all(string));
    let valid = record.name.as_ref() == expected[index]
        && match index {
            0 => params.len() == 2 && strings(&params[0]) && string(&params[1]),
            1 => {
                params.len() == 7
                    && string(&params[0])
                    && string(&params[1])
                    && strings(&params[2])
                    && strings(&params[3])
                    && params[4..].iter().all(string)
            }
            _ => params.len() == 1 && strings(&params[0]),
        };
    if valid {
        Ok(())
    } else {
        Err(Diagnostic::error(
            DiagnosticCode::InvalidHeader,
            format!(
                "expected {} with its physical header parameter structure",
                expected[index]
            ),
            record.source,
        ))
    }
}
