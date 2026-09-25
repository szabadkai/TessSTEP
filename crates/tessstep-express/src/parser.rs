use crate::*;

/// Parse declarations. Expression bodies remain explicitly opaque source fragments.
pub fn parse(source: usize, input: &str, limits: Limits) -> Result<Vec<Schema>, Diagnostic> {
    parse_counted(source, input, limits).map(|(schemas, _, _)| schemas)
}
pub(crate) fn parse_counted(
    source: usize,
    input: &str,
    limits: Limits,
) -> Result<(Vec<Schema>, usize, usize), Diagnostic> {
    let tokens = lex(source, input, limits)?;
    let token_count = tokens.len();
    let eof = Span {
        source,
        start: input.len(),
        end: input.len(),
        line: input.bytes().filter(|b| *b == b'\n').count() + 1,
        column: input.rsplit('\n').next().unwrap_or("").len() + 1,
    };
    let mut p = Parser {
        input,
        tokens,
        pos: 0,
        eof,
        limits,
        declarations: 0,
        clone_work: 0,
    };
    let mut schemas = Vec::new();
    while p.pos < p.tokens.len() {
        if schemas.len() >= limits.max_schemas {
            return Err(limit(p.span(), "schemas"));
        }
        schemas.push(p.schema()?);
    }
    if schemas.is_empty() {
        return Err(p.error("expected SCHEMA"));
    }
    Ok((schemas, token_count, p.clone_work))
}
struct Parser<'a> {
    input: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    eof: Span,
    limits: Limits,
    declarations: usize,
    clone_work: usize,
}
impl Parser<'_> {
    fn span(&self) -> Span {
        self.tokens.get(self.pos).map_or(self.eof, |t| t.span)
    }
    fn at(&self, s: &str) -> bool {
        self.tokens
            .get(self.pos)
            .is_some_and(|t| t.text == s && matches!(t.kind, TokenKind::Word | TokenKind::Symbol))
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<(), Diagnostic> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(self.error(format!("expected {s}")))
        }
    }
    fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error("EX1003", self.span(), message)
    }
    fn finish_span(&self, mut start: Span) -> Span {
        start.end = self.tokens[self.pos - 1].span.end;
        start
    }
    fn name(&mut self) -> Result<Name, Diagnostic> {
        let Some(t) = self.tokens.get(self.pos) else {
            return Err(self.error("expected identifier"));
        };
        if t.kind != TokenKind::Word || reserved(&t.text) {
            return Err(self.error("expected non-reserved identifier"));
        }
        let name = Name {
            text: t.text.clone(),
            span: t.span,
        };
        self.pos += 1;
        Ok(name)
    }
    fn names(&mut self) -> Result<Vec<Name>, Diagnostic> {
        self.expect("(")?;
        let mut names = vec![self.name()?];
        while self.eat(",") {
            names.push(self.name()?);
        }
        self.expect(")")?;
        Ok(names)
    }
    fn schema(&mut self) -> Result<Schema, Diagnostic> {
        let start = self.span();
        self.expect("SCHEMA")?;
        let name = self.name()?;
        self.expect(";")?;
        let mut imports = Vec::new();
        let mut declarations = Vec::new();
        while self.at("USE") || self.at("REFERENCE") {
            imports.push(self.import()?);
        }
        while !self.at("END_SCHEMA") {
            if self.eat("CONSTANT") {
                let mut count = 0;
                while !self.at("END_CONSTANT") {
                    let start = self.span();
                    let name = self.name()?;
                    self.expect(":")?;
                    let ty = self.ty(0)?;
                    self.expect(":=")?;
                    let value = self.expression(&[";"])?;
                    self.expect(";")?;
                    self.add_decl()?;
                    count += 1;
                    declarations.push(Declaration {
                        name,
                        span: self.finish_span(start),
                        kind: DeclarationKind::Constant { ty, value },
                    });
                }
                if count == 0 {
                    return Err(self.error("empty CONSTANT section"));
                }
                self.expect("END_CONSTANT")?;
                self.expect(";")?;
            } else {
                self.add_decl()?;
                declarations.push(self.declaration()?);
            }
        }
        self.expect("END_SCHEMA")?;
        self.expect(";")?;
        Ok(Schema {
            name,
            span: self.finish_span(start),
            imports,
            declarations,
        })
    }
    fn add_decl(&mut self) -> Result<(), Diagnostic> {
        if self.declarations >= self.limits.max_declarations {
            return Err(limit(self.span(), "declarations"));
        }
        self.declarations += 1;
        Ok(())
    }
    fn import(&mut self) -> Result<Import, Diagnostic> {
        let start = self.span();
        let kind = if self.eat("USE") {
            ImportKind::Use
        } else {
            self.expect("REFERENCE")?;
            ImportKind::Reference
        };
        self.expect("FROM")?;
        let schema = self.name()?;
        let items = if self.eat("(") {
            let mut items = Vec::new();
            loop {
                let name = self.name()?;
                let alias = if self.eat("AS") {
                    Some(self.name()?)
                } else {
                    None
                };
                items.push(ImportItem { name, alias });
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(")")?;
            Some(items)
        } else {
            None
        };
        self.expect(";")?;
        Ok(Import {
            kind,
            schema,
            items,
            span: self.finish_span(start),
        })
    }
    fn declaration(&mut self) -> Result<Declaration, Diagnostic> {
        let start = self.span();
        let name;
        let kind = if self.eat("ENTITY") {
            name = self.name()?;
            let abstract_entity = self.eat("ABSTRACT");
            let supertype = if self.eat("SUPERTYPE") {
                if self.eat("OF") {
                    self.expect("(")?;
                    let e = self.expression(&[")"])?;
                    self.expect(")")?;
                    Some(e)
                } else {
                    Some(Expression {
                        text: String::new(),
                        span: self.tokens[self.pos - 1].span,
                    })
                }
            } else {
                None
            };
            let supertypes = if self.eat("SUBTYPE") {
                self.expect("OF")?;
                self.names()?
            } else {
                Vec::new()
            };
            self.expect(";")?;
            let mut attributes = Vec::new();
            while !self.at("DERIVE")
                && !self.at("INVERSE")
                && !self.at("UNIQUE")
                && !self.at("WHERE")
                && !self.at("END_ENTITY")
            {
                attributes.extend(self.attributes(0)?);
            }
            if self.eat("DERIVE") {
                let before = attributes.len();
                while !self.at("INVERSE")
                    && !self.at("UNIQUE")
                    && !self.at("WHERE")
                    && !self.at("END_ENTITY")
                {
                    attributes.extend(self.attributes(1)?);
                }
                if before == attributes.len() {
                    return Err(self.error("empty DERIVE section"));
                }
            }
            if self.eat("INVERSE") {
                let before = attributes.len();
                while !self.at("UNIQUE") && !self.at("WHERE") && !self.at("END_ENTITY") {
                    attributes.extend(self.attributes(2)?);
                }
                if before == attributes.len() {
                    return Err(self.error("empty INVERSE section"));
                }
            }
            let unique = if self.eat("UNIQUE") {
                self.rules(&["WHERE", "END_ENTITY"])?
            } else {
                Vec::new()
            };
            let where_rules = if self.eat("WHERE") {
                self.rules(&["END_ENTITY"])?
            } else {
                Vec::new()
            };
            self.expect("END_ENTITY")?;
            self.expect(";")?;
            DeclarationKind::Entity(Entity {
                abstract_entity,
                supertype,
                supertypes,
                attributes,
                unique,
                where_rules,
            })
        } else if self.eat("TYPE") {
            name = self.name()?;
            self.expect("=")?;
            let underlying = self.ty(0)?;
            self.expect(";")?;
            let where_rules = if self.eat("WHERE") {
                self.rules(&["END_TYPE"])?
            } else {
                Vec::new()
            };
            self.expect("END_TYPE")?;
            self.expect(";")?;
            DeclarationKind::Type {
                underlying,
                where_rules,
            }
        } else if ["FUNCTION", "PROCEDURE", "RULE", "SUBTYPE_CONSTRAINT"]
            .iter()
            .any(|k| self.at(k))
        {
            let keyword = self.tokens[self.pos].text.clone();
            self.pos += 1;
            name = self.name()?;
            let end = format!("END_{keyword}");
            // These declarations are preserved, not parsed. Match nested declarations of the same kind.
            let mut depth = 1;
            while depth > 0 {
                if self.pos == self.tokens.len() || self.at("END_SCHEMA") {
                    return Err(self.error(format!("expected {end}")));
                }
                if self.at(&keyword) {
                    depth += 1;
                    if depth > self.limits.max_nesting.min(128) {
                        return Err(limit(self.span(), "declaration nesting"));
                    }
                }
                if self.at(&end) {
                    depth -= 1;
                }
                self.pos += 1;
            }
            self.expect(";")?;
            let span = self.finish_span(start);
            DeclarationKind::Unsupported {
                keyword,
                body: Expression {
                    text: self.input[span.start..span.end].into(),
                    span,
                },
            }
        } else {
            return Err(
                self.error("expected ENTITY, TYPE, CONSTANT, or a supported opaque declaration")
            );
        };
        Ok(Declaration {
            name,
            span: self.finish_span(start),
            kind,
        })
    }
    fn attributes(&mut self, section: u8) -> Result<Vec<Attribute>, Diagnostic> {
        let start = self.span();
        let mut names = vec![self.name()?];
        if section == 0 {
            while self.eat(",") {
                names.push(self.name()?);
            }
        }
        self.expect(":")?;
        let optional = section == 0 && self.eat("OPTIONAL");
        let ty = self.ty(0)?;
        let kind = match section {
            1 => {
                self.expect(":=")?;
                AttributeKind::Derived(self.expression(&[";"])?)
            }
            2 => {
                self.expect("FOR")?;
                let first = self.name()?;
                let (entity, attribute) = if self.eat(".") {
                    (Some(first), self.name()?)
                } else {
                    (None, first)
                };
                AttributeKind::Inverse { entity, attribute }
            }
            _ => AttributeKind::Explicit,
        };
        self.expect(";")?;
        let span = self.finish_span(start);
        let cost = (span.end - span.start)
            .checked_mul(names.len())
            .ok_or_else(|| limit(span, "AST expansion"))?;
        self.clone_work = self
            .clone_work
            .checked_add(cost)
            .ok_or_else(|| limit(span, "AST expansion"))?;
        if self.clone_work > self.limits.max_work {
            return Err(limit(span, "AST expansion"));
        }
        Ok(names
            .into_iter()
            .map(|name| Attribute {
                name,
                ty: ty.clone(),
                optional,
                kind: kind.clone(),
                span,
            })
            .collect())
    }
    fn rules(&mut self, stop: &[&str]) -> Result<Vec<Rule>, Diagnostic> {
        let mut rules = Vec::new();
        while !stop.iter().any(|s| self.at(s)) {
            let label = if self.tokens.get(self.pos + 1).is_some_and(|t| t.text == ":") {
                let n = self.name()?;
                self.expect(":")?;
                Some(n)
            } else {
                None
            };
            let expression = self.expression(&[";"])?;
            self.expect(";")?;
            rules.push(Rule { label, expression });
        }
        if rules.is_empty() {
            return Err(self.error("empty rule section"));
        }
        Ok(rules)
    }
    fn ty(&mut self, depth: usize) -> Result<TypeExpr, Diagnostic> {
        if depth >= self.limits.max_nesting.min(128) {
            return Err(limit(self.span(), "type nesting"));
        }
        let start = self.span();
        let kind = if ["ARRAY", "BAG", "LIST", "SET"].iter().any(|k| self.at(k)) {
            let aggregate = match self.tokens[self.pos].text.as_str() {
                "ARRAY" => AggregateKind::Array,
                "BAG" => AggregateKind::Bag,
                "LIST" => AggregateKind::List,
                _ => AggregateKind::Set,
            };
            self.pos += 1;
            let bounds = if self.eat("[") {
                let lower = self.expression(&[":"])?;
                self.expect(":")?;
                let upper = self.expression(&["]"])?;
                self.expect("]")?;
                Some((lower, upper))
            } else {
                None
            };
            if aggregate == AggregateKind::Array && bounds.is_none() {
                return Err(self.error("ARRAY requires bounds"));
            }
            self.expect("OF")?;
            let optional = aggregate == AggregateKind::Array && self.eat("OPTIONAL");
            let unique = matches!(aggregate, AggregateKind::Array | AggregateKind::List)
                && self.eat("UNIQUE");
            let element = Box::new(self.ty(depth + 1)?);
            TypeKind::Aggregate {
                kind: aggregate,
                bounds,
                optional,
                unique,
                element,
            }
        } else if self.eat("ENUMERATION") {
            self.expect("OF")?;
            TypeKind::Enumeration(self.names()?)
        } else if self.eat("SELECT") {
            TypeKind::Select(self.names()?)
        } else if [
            "BINARY", "BOOLEAN", "INTEGER", "LOGICAL", "NUMBER", "REAL", "STRING",
        ]
        .iter()
        .any(|k| self.at(k))
        {
            let name = self.tokens[self.pos].text.clone();
            self.pos += 1;
            let width = if matches!(name.as_str(), "BINARY" | "REAL" | "STRING") && self.eat("(") {
                let e = self.expression(&[")"])?;
                self.expect(")")?;
                Some(e)
            } else {
                None
            };
            let fixed = matches!(name.as_str(), "BINARY" | "STRING") && self.eat("FIXED");
            if fixed && width.is_none() {
                return Err(self.error("FIXED requires a width"));
            }
            TypeKind::Builtin { name, width, fixed }
        } else {
            TypeKind::Named(self.name()?)
        };
        Ok(TypeExpr {
            kind,
            span: self.finish_span(start),
        })
    }
    fn expression(&mut self, stops: &[&str]) -> Result<Expression, Diagnostic> {
        let start = self.span();
        let begin = self.pos;
        let mut stack = Vec::new();
        while self.pos < self.tokens.len() {
            if stack.is_empty() && stops.iter().any(|s| self.at(s)) {
                break;
            }
            let token = &self.tokens[self.pos];
            if token.kind == TokenKind::Word
                && (token.text.starts_with("END_")
                    || matches!(
                        token.text.as_str(),
                        "SCHEMA" | "ENTITY" | "TYPE" | "DERIVE" | "INVERSE" | "UNIQUE" | "WHERE"
                    ))
            {
                return Err(
                    self.error("unterminated expression before declaration/section boundary")
                );
            }
            if token.kind == TokenKind::Symbol {
                match token.text.as_str() {
                    "(" => stack.push(")"),
                    "[" => stack.push("]"),
                    "{" => stack.push("}"),
                    ")" | "]" | "}" if stack.pop() != Some(token.text.as_str()) => {
                        return Err(self.error("unbalanced expression delimiter"));
                    }
                    ";" => return Err(self.error("unexpected semicolon in expression")),
                    _ => {}
                }
            }
            if stack.len() > self.limits.max_nesting.min(128) {
                return Err(limit(token.span, "expression nesting"));
            }
            self.pos += 1;
        }
        if self.pos == self.tokens.len() || !stack.is_empty() {
            return Err(self.error("unterminated expression"));
        }
        if self.pos == begin {
            return Err(self.error("empty expression"));
        }
        let span = self.finish_span(start);
        Ok(Expression {
            text: self.input[span.start..span.end].into(),
            span,
        })
    }
}
fn reserved(name: &str) -> bool {
    // Keywords and standard built-ins occupy the EXPRESS reserved namespace.
    const WORDS: &str = " ABS ABSTRACT ACOS AGGREGATE ALIAS AND ANDOR ARRAY AS ASIN ATAN BAG BASED_ON BEGIN BINARY BLENGTH BOOLEAN BY CASE CONSTANT CONST_E COS DERIVE DIV ELSE END END_ALIAS END_CASE END_CONSTANT END_ENTITY END_FUNCTION END_IF END_LOCAL END_PROCEDURE END_REPEAT END_RULE END_SCHEMA END_SUBTYPE_CONSTRAINT END_TYPE ENTITY ENUMERATION ESCAPE EXISTS EXP EXTENSIBLE FALSE FIXED FOR FORMAT FROM FUNCTION GENERIC GENERIC_ENTITY HIBOUND HIINDEX IF IN INSERT INTEGER INVERSE LENGTH LIKE LIST LOBOUND LOCAL LOG LOG2 LOG10 LOGICAL LOINDEX MOD NOT NUMBER NVL ODD OF ONEOF OPTIONAL OR OTHERWISE PI PROCEDURE QUERY REAL REFERENCE REMOVE RENAMED REPEAT RETURN ROLESOF RULE SCHEMA SELECT SELF SET SIN SIZEOF SKIP SQRT STRING SUBTYPE SUBTYPE_CONSTRAINT SUPERTYPE TAN THEN TO TOTAL_OVER TRUE TYPE TYPEOF UNIQUE UNKNOWN UNTIL USE USEDIN VALUE VALUE_IN VALUE_UNIQUE VAR WHERE WHILE WITH XOR ";
    WORDS.split_ascii_whitespace().any(|w| w == name)
}
