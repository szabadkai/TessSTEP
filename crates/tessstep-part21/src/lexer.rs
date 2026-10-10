use crate::{
    BinaryValue, Conformance, Deviation, Diagnostic, DiagnosticCode as Code, EntityId, Occurrence,
    ParseLimits, Signature, SourcePosition, SourceSpan, ValueId,
};
use std::io::{self, BufRead};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Word(String),
    Integer(i64),
    Real(f64),
    String(String),
    Binary(BinaryValue),
    Enumeration(String),
    Occurrence(Occurrence),
    ConstantEntity(String),
    ConstantValue(String),
    Resource(String),
    LeftParen,
    RightParen,
    Comma,
    Semicolon,
    Equals,
    Null,
    Omitted,
    LeftBrace,
    RightBrace,
    Colon,
    Eof,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub source: SourceSpan,
}

/// Pull lexer over `BufRead`. Retains only the current token and the reader buffer.
/// Once an error occurs the lexer is fused: subsequent calls return EOF.
#[derive(Debug)]
pub struct Lexer<R> {
    reader: R,
    limits: ParseLimits,
    position: SourcePosition,
    previous_cr: bool,
    failed: bool,
    token_start: SourcePosition,
    conformance: Conformance,
    literal_backslashes: Deviation,
}
impl<R: BufRead> Lexer<R> {
    /// A tolerant lexer ([`Conformance::Tolerant`]).
    pub fn new(reader: R, limits: ParseLimits) -> Self {
        Self::with_conformance(reader, limits, Conformance::default())
    }
    pub fn with_conformance(reader: R, limits: ParseLimits, conformance: Conformance) -> Self {
        Self {
            reader,
            limits,
            position: SourcePosition {
                offset: 0,
                line: 1,
                column: 1,
            },
            previous_cr: false,
            failed: false,
            token_start: SourcePosition::default(),
            conformance,
            literal_backslashes: Deviation::default(),
        }
    }
    /// String backslashes kept literally so far ([`crate::Deviations::literal_backslashes`]).
    pub fn literal_backslashes(&self) -> Deviation {
        self.literal_backslashes
    }
    pub fn position(&self) -> SourcePosition {
        self.position
    }
    pub(crate) fn span(&self, start: SourcePosition) -> SourceSpan {
        SourceSpan {
            start,
            end: self.position,
        }
    }
    pub(crate) fn error(&self, code: Code, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, message, self.span(self.token_start))
    }
    fn raw_peek(&mut self) -> Result<Option<u8>, Diagnostic> {
        loop {
            match self.reader.fill_buf() {
                Ok(bytes) => {
                    if !bytes.is_empty() && self.position.offset >= self.limits.max_input_bytes {
                        return Err(self.error(
                            Code::LimitExceeded,
                            format!(
                                "input byte budget exceeded (limit {} bytes)",
                                self.limits.max_input_bytes
                            ),
                        ));
                    }
                    return Ok(bytes.first().copied());
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(self.error(Code::Io, err.to_string())),
            }
        }
    }
    fn raw_bump(&mut self, byte: u8) {
        self.reader.consume(1);
        self.position.offset += 1;
        if byte == b'\r' || (byte == b'\n' && !self.previous_cr) {
            self.position.line += 1;
            self.position.column = 1;
        } else if byte != b'\n' {
            self.position.column += 1;
        }
        self.previous_cr = byte == b'\r';
    }
    /// Physical control octets are ignored even within tokens, while source
    /// locations continue to describe the unmodified input stream.
    fn peek(&mut self) -> Result<Option<u8>, Diagnostic> {
        loop {
            match self.raw_peek()? {
                Some(byte) if byte < 0x20 || byte == 0x7f => self.raw_bump(byte),
                byte => return Ok(byte),
            }
        }
    }
    fn bump(&mut self) -> Result<Option<u8>, Diagnostic> {
        let byte = self.peek()?;
        if let Some(byte) = byte {
            self.raw_bump(byte);
        }
        if self.position.offset.saturating_sub(self.token_start.offset)
            > self.limits.max_token_bytes as u64
        {
            return Err(self.error(
                Code::LimitExceeded,
                format!(
                    "token byte budget exceeded (limit {} bytes)",
                    self.limits.max_token_bytes
                ),
            ));
        }
        Ok(byte)
    }
    fn expect_byte(&mut self, expected: u8, code: Code) -> Result<(), Diagnostic> {
        if self.bump()? == Some(expected) {
            Ok(())
        } else {
            Err(self.error(code, format!("expected byte {:?}", char::from(expected))))
        }
    }
    fn skip_separators(&mut self) -> Result<(), Diagnostic> {
        loop {
            self.token_start = self.position;
            match self.peek()? {
                Some(b' ') => {
                    self.raw_bump(b' ');
                }
                Some(b'/') => {
                    self.bump()?;
                    self.expect_byte(b'*', Code::InvalidCharacter)?;
                    let mut star = false;
                    loop {
                        let Some(byte) = self.peek()? else {
                            return Err(
                                self.error(Code::UnterminatedComment, "unterminated comment")
                            );
                        };
                        self.raw_bump(byte);
                        if star && byte == b'/' {
                            break;
                        }
                        star = byte == b'*';
                    }
                }
                Some(b'\\') => {
                    self.bump()?;
                    if !matches!(self.bump()?, Some(b'N' | b'F')) {
                        return Err(
                            self.error(Code::InvalidEscape, "expected print control directive")
                        );
                    }
                    self.expect_byte(b'\\', Code::InvalidEscape)?;
                }
                _ => return Ok(()),
            }
        }
    }
    fn word(&mut self) -> Result<String, Diagnostic> {
        let mut bytes = Vec::new();
        while let Some(byte) = self.peek()? {
            if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' {
                bytes.push(byte);
                self.bump()?;
            } else {
                break;
            }
        }
        // All bytes are ASCII by construction.
        Ok(bytes.into_iter().map(char::from).collect())
    }
    fn digits(&mut self, bytes: &mut String) -> Result<usize, Diagnostic> {
        let before = bytes.len();
        while let Some(byte @ b'0'..=b'9') = self.peek()? {
            bytes.push(char::from(byte));
            self.bump()?;
        }
        Ok(bytes.len() - before)
    }
    fn number(&mut self) -> Result<TokenKind, Diagnostic> {
        let mut text = String::new();
        if let Some(sign @ (b'+' | b'-')) = self.peek()? {
            text.push(char::from(sign));
            self.bump()?;
        }
        if self.digits(&mut text)? == 0 {
            return Err(self.error(Code::InvalidNumber, "number requires digits"));
        }
        if self.peek()? != Some(b'.') {
            return text
                .parse()
                .map(TokenKind::Integer)
                .map_err(|_| self.error(Code::NumericRange, "integer is outside i64 range"));
        }
        self.bump()?;
        text.push('.');
        self.digits(&mut text)?;
        if self.peek()? == Some(b'E') {
            self.bump()?;
            text.push('E');
            if let Some(sign @ (b'+' | b'-')) = self.peek()? {
                text.push(char::from(sign));
                self.bump()?;
            }
            if self.digits(&mut text)? == 0 {
                return Err(self.error(Code::InvalidNumber, "exponent requires digits"));
            }
        }
        let value: f64 = text
            .parse()
            .map_err(|_| self.error(Code::InvalidNumber, "invalid real"))?;
        let mantissa = text.split('E').next().unwrap_or("");
        if !value.is_finite()
            || (value == 0.0 && mantissa.bytes().any(|b| matches!(b, b'1'..=b'9')))
        {
            return Err(self.error(Code::NumericRange, "real overflow or underflow to zero"));
        }
        Ok(TokenKind::Real(value))
    }
    fn string(&mut self) -> Result<TokenKind, Diagnostic> {
        self.bump()?;
        let mut raw = Vec::new();
        loop {
            let Some(byte) = self.bump()? else {
                return Err(self.error(Code::UnterminatedString, "unterminated string"));
            };
            if byte == b'\'' {
                if self.peek()? == Some(b'\'') {
                    raw.extend_from_slice(b"''");
                    self.bump()?;
                } else {
                    break;
                }
            } else if byte == b'\\' {
                if self.conformance == Conformance::Strict {
                    self.strict_directive(&mut raw)?;
                } else {
                    self.tolerant_directive(&mut raw)?;
                }
            } else {
                raw.push(byte);
            }
        }
        crate::strings::decode(&raw, self.limits.max_string_bytes)
            .map(TokenKind::String)
            .map_err(|(code, message)| {
                if code == Code::LimitExceeded {
                    self.error(
                        code,
                        format!("{message} (limit {} bytes)", self.limits.max_string_bytes),
                    )
                } else {
                    self.error(code, message)
                }
            })
    }
    /// Reads the directive after a string backslash into `raw`, rejecting any that is
    /// not well formed.
    fn strict_directive(&mut self, raw: &mut Vec<u8>) -> Result<(), Diagnostic> {
        raw.push(b'\\');
        let next = self
            .bump()?
            .ok_or_else(|| self.error(Code::UnterminatedString, "unterminated escape"))?;
        raw.push(next);
        let tail = match next {
            b'\\' => 0,
            b'S' | b'P' => 2,
            b'N' | b'F' => 1,
            b'X' => {
                let mode = self
                    .bump()?
                    .ok_or_else(|| self.error(Code::InvalidEscape, "missing X mode"))?;
                raw.push(mode);
                match mode {
                    b'\\' => 2,
                    b'2' | b'4' => {
                        self.expect_byte(b'\\', Code::InvalidEscape)?;
                        raw.push(b'\\');
                        return self.extended_run(raw);
                    }
                    _ => return Err(self.error(Code::InvalidEscape, "invalid X directive")),
                }
            }
            _ => return Err(self.error(Code::InvalidEscape, "unknown string directive")),
        };
        self.directive_payload(raw, tail)
    }
    /// Reads the directive after a string backslash into `raw`. A backslash whose
    /// following characters complete no directive opening (`\\`, `\N\`, `\F\`,
    /// `\PA\` to `\PI\`, `\S\`, `\X\`, `\X2\`, `\X4\`) is a literal backslash,
    /// written to `raw` as `\\` and counted. The character that broke the opening is
    /// left unread, so an apostrophe after the backslash still ends the string. A
    /// complete opening with a malformed payload is rejected as in strict mode.
    fn tolerant_directive(&mut self, raw: &mut Vec<u8>) -> Result<(), Diagnostic> {
        let end = self.position;
        let start = SourcePosition {
            offset: end.offset - 1,
            column: end.column - 1,
            ..end
        };
        let mut read = Vec::new();
        let tail = match self.peek()? {
            None => return Err(self.error(Code::UnterminatedString, "unterminated escape")),
            Some(b'\\') => {
                self.bump()?;
                raw.extend_from_slice(b"\\\\");
                return Ok(());
            }
            Some(letter @ (b'N' | b'F')) => {
                self.bump()?;
                read.push(letter);
                self.opening_byte(b'\\', &mut read)?.then_some(0)
            }
            Some(b'P') => {
                self.bump()?;
                read.push(b'P');
                match self.peek()? {
                    Some(alphabet @ b'A'..=b'I') => {
                        self.bump()?;
                        read.push(alphabet);
                        self.opening_byte(b'\\', &mut read)?.then_some(0)
                    }
                    _ => None,
                }
            }
            Some(b'S') => {
                self.bump()?;
                read.push(b'S');
                self.opening_byte(b'\\', &mut read)?.then_some(1)
            }
            Some(b'X') => {
                self.bump()?;
                read.push(b'X');
                match self.peek()? {
                    Some(b'\\') => {
                        self.bump()?;
                        read.push(b'\\');
                        Some(2)
                    }
                    Some(mode @ (b'2' | b'4')) => {
                        self.bump()?;
                        read.push(mode);
                        if self.opening_byte(b'\\', &mut read)? {
                            raw.push(b'\\');
                            raw.extend_from_slice(&read);
                            return self.extended_run(raw);
                        }
                        None
                    }
                    _ => None,
                }
            }
            Some(_) => None,
        };
        match tail {
            Some(tail) => {
                raw.push(b'\\');
                raw.extend_from_slice(&read);
                self.directive_payload(raw, tail)
            }
            None => {
                self.literal_backslashes.record(SourceSpan { start, end });
                raw.extend_from_slice(b"\\\\");
                raw.extend_from_slice(&read);
                Ok(())
            }
        }
    }
    /// Consumes the next byte when it is `expected`, the next byte of an opening.
    fn opening_byte(&mut self, expected: u8, read: &mut Vec<u8>) -> Result<bool, Diagnostic> {
        if self.peek()? == Some(expected) {
            self.bump()?;
            read.push(expected);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    /// The hexadecimal groups of an X2 or X4 directive through their `\X0\`.
    fn extended_run(&mut self, raw: &mut Vec<u8>) -> Result<(), Diagnostic> {
        loop {
            let digit = self
                .bump()?
                .ok_or_else(|| self.error(Code::InvalidEscape, "unterminated Unicode escape"))?;
            raw.push(digit);
            if digit == b'\\' {
                break;
            }
            if hex(digit).is_none() {
                return Err(self.error(Code::InvalidEscape, "expected Unicode hex digit"));
            }
        }
        self.directive_payload(raw, 3)
    }
    /// The remaining `count` bytes of a directive, checked when decoded.
    fn directive_payload(&mut self, raw: &mut Vec<u8>, count: usize) -> Result<(), Diagnostic> {
        for _ in 0..count {
            raw.push(
                self.bump()?
                    .ok_or_else(|| self.error(Code::InvalidEscape, "incomplete directive"))?,
            );
        }
        Ok(())
    }
    fn binary(&mut self) -> Result<TokenKind, Diagnostic> {
        self.bump()?;
        let unused = match self.bump()? {
            Some(byte @ b'0'..=b'3') => usize::from(byte - b'0'),
            _ => {
                return Err(self.error(
                    Code::InvalidBinary,
                    "binary requires leading padding count 0..3",
                ));
            }
        };
        let mut nibbles = Vec::new();
        loop {
            match self.bump()? {
                Some(b'"') => break,
                Some(b'\\') => {
                    if !matches!(self.bump()?, Some(b'N' | b'F')) {
                        return Err(
                            self.error(Code::InvalidBinary, "invalid binary print directive")
                        );
                    }
                    self.expect_byte(b'\\', Code::InvalidBinary)?;
                }
                Some(byte) => match hex(byte) {
                    Some(nibble) => nibbles.push(nibble),
                    None => {
                        return Err(self.error(
                            Code::InvalidBinary,
                            "binary requires uppercase hexadecimal digits",
                        ));
                    }
                },
                None => return Err(self.error(Code::InvalidBinary, "unterminated binary")),
            }
        }
        if (nibbles.is_empty() && unused != 0)
            || nibbles.first().is_some_and(|&v| v >> (4 - unused) != 0)
        {
            return Err(self.error(Code::InvalidBinary, "invalid leading zero padding"));
        }
        let bit_len = nibbles.len() * 4 - unused;
        let mut bytes = vec![0; bit_len.div_ceil(8)];
        for bit in 0..bit_len {
            let source_bit = bit + unused;
            let value = (nibbles[source_bit / 4] >> (3 - source_bit % 4)) & 1;
            bytes[bit / 8] |= value << (7 - bit % 8);
        }
        Ok(TokenKind::Binary(BinaryValue { bytes, bit_len }))
    }
    fn occurrence(&mut self) -> Result<TokenKind, Diagnostic> {
        let entity = self.bump()? == Some(b'#');
        if self.peek()?.is_some_and(upper) {
            let word = self.word()?;
            if !keyword(&word) {
                return Err(self.error(Code::InvalidIdentifier, "invalid constant name"));
            }
            return Ok(if entity {
                TokenKind::ConstantEntity(word)
            } else {
                TokenKind::ConstantValue(word)
            });
        }
        let mut number = String::new();
        self.digits(&mut number)?;
        let id: u64 = number.parse().map_err(|_| {
            self.error(Code::InvalidIdentifier, "identifier requires a nonzero u64")
        })?;
        let id = if entity {
            Occurrence::Entity(
                EntityId::new(id)
                    .ok_or_else(|| self.error(Code::InvalidIdentifier, "entity zero is invalid"))?,
            )
        } else {
            Occurrence::Value(
                ValueId::new(id)
                    .ok_or_else(|| self.error(Code::InvalidIdentifier, "value zero is invalid"))?,
            )
        };
        Ok(TokenKind::Occurrence(id))
    }
    fn token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_separators()?;
        self.token_start = self.position;
        let kind = match self.peek()? {
            None => TokenKind::Eof,
            Some(b'0'..=b'9' | b'+' | b'-') => self.number()?,
            Some(b'\'') => self.string()?,
            Some(b'"') => self.binary()?,
            Some(b'#' | b'@') => self.occurrence()?,
            Some(b'.') => {
                self.bump()?;
                let name = self.word()?;
                if !keyword(&name) {
                    return Err(self.error(Code::InvalidIdentifier, "invalid enumeration"));
                }
                self.expect_byte(b'.', Code::InvalidIdentifier)?;
                TokenKind::Enumeration(name)
            }
            Some(b'<') => {
                self.bump()?;
                let mut bytes = Vec::new();
                loop {
                    match self.bump()? {
                        Some(b'>') => break,
                        Some(byte) if uri_byte(byte) => bytes.push(byte),
                        _ => {
                            return Err(self.error(
                                Code::InvalidIdentifier,
                                "invalid or unterminated URI resource",
                            ));
                        }
                    }
                }
                let resource: String = bytes.into_iter().map(char::from).collect();
                if !valid_percent_encoding(&resource) {
                    return Err(self.error(Code::InvalidIdentifier, "invalid URI percent escape"));
                }
                TokenKind::Resource(resource)
            }
            Some(b'!') => {
                self.bump()?;
                let word = self.word()?;
                if !keyword(&word) {
                    return Err(self.error(Code::InvalidIdentifier, "invalid user-defined keyword"));
                }
                TokenKind::Word(format!("!{word}"))
            }
            Some(byte) if byte.is_ascii_alphabetic() || byte == b'_' => {
                TokenKind::Word(self.word()?)
            }
            Some(byte) => {
                self.bump()?;
                match byte {
                    b'(' => TokenKind::LeftParen,
                    b')' => TokenKind::RightParen,
                    b',' => TokenKind::Comma,
                    b';' => TokenKind::Semicolon,
                    b'=' => TokenKind::Equals,
                    b'$' => TokenKind::Null,
                    b'*' => TokenKind::Omitted,
                    b'{' => TokenKind::LeftBrace,
                    b'}' => TokenKind::RightBrace,
                    b':' => TokenKind::Colon,
                    b'&' => {
                        return Err(self.error(
                            Code::UnsupportedFeature,
                            "legacy scope encoding is unsupported",
                        ));
                    }
                    _ => {
                        return Err(self.error(
                            Code::InvalidCharacter,
                            format!("unexpected byte 0x{byte:02X}"),
                        ));
                    }
                }
            }
        };
        Ok(Token {
            kind,
            source: self.span(self.token_start),
        })
    }
    pub fn next_token(&mut self) -> Result<Token, Diagnostic> {
        if self.failed {
            return Ok(Token {
                kind: TokenKind::Eof,
                source: self.span(self.position),
            });
        }
        let result = self.token();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(crate) fn next_signature(&mut self) -> Result<Option<Signature>, Diagnostic> {
        self.skip_separators()?;
        self.token_start = self.position;
        if self.peek()?.is_none() {
            return Ok(None);
        }
        let start = self.position;
        for byte in b"SIGNATURE" {
            self.expect_byte(*byte, Code::InvalidSignature)?;
        }
        self.signature(start).map(Some)
    }
    /// Called directly after consuming SIGNATURE; the payload is not STEP tokens.
    pub(crate) fn signature(&mut self, start: SourcePosition) -> Result<Signature, Diagnostic> {
        self.token_start = start;
        let mut payload = String::new();
        loop {
            match self.bump()? {
                Some(b';') => break,
                Some(b' ') => (),
                Some(byte)
                    if byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=') =>
                {
                    payload.push(char::from(byte))
                }
                _ => {
                    return Err(self.error(
                        Code::InvalidSignature,
                        "invalid or truncated signature section",
                    ));
                }
            }
        }
        let Some(content) = payload.strip_suffix("ENDSEC") else {
            return Err(self.error(Code::InvalidSignature, "missing signature ENDSEC"));
        };
        let data = content.trim_end_matches('=');
        let padding = content.len() - data.len();
        if content.is_empty() || content.len() % 4 != 0 || padding > 2 || data.contains('=') {
            return Err(self.error(
                Code::InvalidSignature,
                "invalid base64 signature length or padding",
            ));
        }
        let last = data.bytes().last().and_then(base64_digit).unwrap_or(0);
        if (padding == 1 && last & 3 != 0) || (padding == 2 && last & 15 != 0) {
            return Err(self.error(Code::InvalidSignature, "nonzero base64 padding bits"));
        }
        Ok(Signature {
            base64: content.to_owned(),
            source: self.span(start),
        })
    }
}
fn base64_digit(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
pub(crate) fn upper(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte == b'_'
}
pub(crate) fn keyword(word: &str) -> bool {
    let word = word.strip_prefix('!').unwrap_or(word);
    word.bytes().next().is_some_and(upper) && word.bytes().all(|b| upper(b) || b.is_ascii_digit())
}
pub(crate) fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
fn uri_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&byte)
}
fn valid_percent_encoding(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().all(|(i, &b)| {
        b != b'%'
            || (bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
    })
}
