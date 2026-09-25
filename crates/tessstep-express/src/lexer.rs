use crate::{Diagnostic, Limits, Span, limit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Word,
    Number,
    String,
    EncodedString,
    Binary,
    Symbol,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    /// Words normalized to uppercase; literals and punctuation kept verbatim.
    pub text: String,
    pub span: Span,
}

/// Tokenize a whole UTF-8 source with byte offsets and one-based byte columns.
pub fn lex(source: usize, input: &str, limits: Limits) -> Result<Vec<Token>, Diagnostic> {
    let mut l = Lexer {
        bytes: input.as_bytes(),
        source,
        pos: 0,
        line: 1,
        column: 1,
    };
    if input.len() > limits.max_input_bytes {
        return Err(limit(l.span(), "input bytes"));
    }
    let mut tokens = Vec::new();
    while l.pos < input.len() {
        if l.bytes[l.pos].is_ascii_whitespace() {
            l.bump();
            continue;
        }
        if l.at("--") {
            while l.pos < input.len() && l.bytes[l.pos] != b'\n' {
                l.bump();
            }
            continue;
        }
        if l.at("(*") {
            let start = l.span();
            let mut depth = 1;
            l.bump();
            l.bump();
            if depth > limits.max_nesting.min(128) {
                return Err(limit(start, "comment nesting"));
            }
            while depth != 0 {
                if l.pos == input.len() {
                    return Err(Diagnostic::error("EX1002", start, "unterminated comment"));
                }
                if l.at("(*") {
                    depth += 1;
                    l.bump();
                    l.bump();
                } else if l.at("*)") {
                    depth -= 1;
                    l.bump();
                    l.bump();
                } else {
                    l.bump();
                }
                if depth > limits.max_nesting.min(128) {
                    return Err(limit(start, "comment nesting"));
                }
            }
            continue;
        }
        let mut span = l.span();
        if tokens.len() >= limits.max_tokens {
            return Err(limit(span, "tokens"));
        }
        let c = l.bytes[l.pos];
        let kind;
        if c.is_ascii_alphabetic() {
            kind = TokenKind::Word;
            while l.pos < input.len()
                && (l.bytes[l.pos].is_ascii_alphanumeric() || l.bytes[l.pos] == b'_')
            {
                l.bump();
            }
        } else if c.is_ascii_digit() {
            kind = TokenKind::Number;
            while l.pos < input.len() && l.bytes[l.pos].is_ascii_digit() {
                l.bump();
            }
            if l.at(".") {
                l.bump();
                while l.pos < input.len() && l.bytes[l.pos].is_ascii_digit() {
                    l.bump();
                }
            }
            if l.at("e") || l.at("E") {
                l.bump();
                if l.at("+") || l.at("-") {
                    l.bump();
                }
                let start = l.pos;
                while l.pos < input.len() && l.bytes[l.pos].is_ascii_digit() {
                    l.bump();
                }
                if l.pos == start {
                    return Err(Diagnostic::error("EX1002", span, "missing exponent digits"));
                }
            }
        } else if c == b'\'' || c == b'"' {
            kind = if c == b'\'' {
                TokenKind::String
            } else {
                TokenKind::EncodedString
            };
            l.bump();
            let start = l.pos;
            loop {
                if l.pos == input.len() {
                    return Err(Diagnostic::error("EX1002", span, "unterminated string"));
                }
                if l.bytes[l.pos] == c {
                    if c == b'\'' && l.bytes.get(l.pos + 1) == Some(&c) {
                        l.bump();
                        l.bump();
                    } else {
                        break;
                    }
                } else {
                    if c == b'"' && !l.bytes[l.pos].is_ascii_hexdigit() {
                        return Err(Diagnostic::error(
                            "EX1002",
                            l.span(),
                            "encoded string requires hexadecimal digits",
                        ));
                    }
                    l.bump();
                }
            }
            if c == b'"' && (l.pos - start) % 8 != 0 {
                return Err(Diagnostic::error(
                    "EX1002",
                    span,
                    "encoded string requires groups of eight hexadecimal digits",
                ));
            }
            l.bump();
        } else if c == b'%' {
            kind = TokenKind::Binary;
            l.bump();
            let start = l.pos;
            while l.at("0") || l.at("1") {
                l.bump();
            }
            if l.pos == start {
                return Err(Diagnostic::error("EX1002", span, "empty binary literal"));
            }
        } else {
            kind = TokenKind::Symbol;
            let operator = [":=:", ":<>:", "**", ":=", "<=", ">=", "<>", "||", "<*"]
                .into_iter()
                .find(|s| l.at(s));
            if let Some(op) = operator {
                for _ in op.bytes() {
                    l.bump();
                }
            } else if b";:,().[]=+-*/<>\\?{}|".contains(&c) {
                l.bump();
            } else {
                return Err(Diagnostic::error(
                    "EX1002",
                    span,
                    "invalid EXPRESS character",
                ));
            }
        }
        span.end = l.pos;
        if span.end - span.start > limits.max_token_bytes {
            return Err(limit(span, "token bytes"));
        }
        let raw = &input[span.start..span.end];
        tokens.push(Token {
            kind,
            text: if kind == TokenKind::Word {
                raw.to_ascii_uppercase()
            } else {
                raw.into()
            },
            span,
        });
    }
    Ok(tokens)
}
struct Lexer<'a> {
    bytes: &'a [u8],
    source: usize,
    pos: usize,
    line: usize,
    column: usize,
}
impl Lexer<'_> {
    fn at(&self, value: &str) -> bool {
        self.bytes[self.pos..].starts_with(value.as_bytes())
    }
    fn bump(&mut self) {
        if self.bytes[self.pos] == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        self.pos += 1;
    }
    fn span(&self) -> Span {
        Span {
            source: self.source,
            start: self.pos,
            end: self.pos,
            line: self.line,
            column: self.column,
        }
    }
}
