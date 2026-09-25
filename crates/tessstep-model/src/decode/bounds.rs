use super::*;
use std::collections::BTreeSet;
impl Context<'_> {
    /// Frontend diagnostics preserve all expressions as opaque. Recognize only
    /// width/bound expressions actually checked by this decoder, by source span.
    pub(super) fn check_diagnostics(&mut self) -> Result<(), Error> {
        if self.schemas.unsupported.is_empty() {
            return Ok(());
        }
        let mut supported = BTreeSet::new();
        for declaration in self.schemas.declarations {
            self.tick()?;
            let mut stack = Vec::new();
            match declaration.kind {
                DeclarationKind::Entity { attributes, .. } => {
                    for attr in attributes {
                        self.tick()?;
                        stack.push((attr.domain, 0));
                    }
                }
                DeclarationKind::Type { domain, .. } => stack.push((domain, 0)),
                _ => (),
            }
            while let Some((domain, depth)) = stack.pop() {
                self.tick()?;
                if depth >= self.limits.max_depth.min(128) {
                    return Err(self.error(
                        ErrorKind::ResourceLimit,
                        "schema expression traversal depth budget",
                    ));
                }
                let expressions = match domain {
                    Domain::Builtin {
                        kind: Builtin::String | Builtin::Binary,
                        width: Some(width),
                        ..
                    } => vec![(width, false)],
                    Domain::Aggregate {
                        bounds, element, ..
                    } => {
                        stack.push((*element, depth + 1));
                        bounds.map_or_else(Vec::new, |(lower, upper)| {
                            vec![(lower, false), (upper, true)]
                        })
                    }
                    _ => Vec::new(),
                };
                for (expression, unbounded) in expressions {
                    if !unbounded || expression.text.trim() != "?" {
                        self.bound(expression)?;
                    }
                    let span = expression.span;
                    supported.insert((span.source, span.start, span.end));
                }
            }
        }
        for diagnostic in self.schemas.unsupported {
            self.tick()?;
            let span = diagnostic.span;
            if diagnostic.code != "EX2001"
                || !supported.contains(&(span.source, span.start, span.end))
            {
                return Err(self.error(
                    ErrorKind::Unsupported,
                    "schema set contains unsupported EXPRESS semantics",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn bound(&mut self, expression: Expression) -> Result<i64, Error> {
        let mut parser = BoundParser {
            bytes: expression.text.as_bytes(),
            offset: 0,
            cx: self,
        };
        let result = parser.expression(0, 0)?;
        parser.space()?;
        if parser.offset != parser.bytes.len() {
            return Err(parser.cx.error(
                ErrorKind::Unsupported,
                "unsupported bound expression syntax",
            ));
        }
        result.try_into().map_err(|_| {
            parser
                .cx
                .error(ErrorKind::InvalidMetadata, "bound is outside i64 range")
        })
    }
}
struct BoundParser<'a, 'b, 'c> {
    bytes: &'a [u8],
    offset: usize,
    cx: &'b mut Context<'c>,
}
impl BoundParser<'_, '_, '_> {
    fn space(&mut self) -> Result<(), Error> {
        loop {
            while self
                .bytes
                .get(self.offset)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.cx.tick()?;
                self.offset += 1;
            }
            if self.bytes.get(self.offset..self.offset + 2) == Some(b"--") {
                while self.bytes.get(self.offset).is_some_and(|c| *c != b'\n') {
                    self.cx.tick()?;
                    self.offset += 1;
                }
            } else if self.bytes.get(self.offset..self.offset + 2) == Some(b"(*") {
                self.offset += 2;
                let mut nesting = 1usize;
                while nesting > 0 {
                    self.cx.tick()?;
                    if self.offset >= self.bytes.len() {
                        return Err(self
                            .cx
                            .error(ErrorKind::InvalidMetadata, "unterminated bound comment"));
                    }
                    match self.bytes.get(self.offset..self.offset + 2) {
                        Some(b"(*") => {
                            nesting += 1;
                            self.offset += 2;
                        }
                        Some(b"*)") => {
                            nesting -= 1;
                            self.offset += 2;
                        }
                        _ => self.offset += 1,
                    }
                }
            } else {
                break;
            }
        }
        Ok(())
    }
    fn expression(&mut self, min_precedence: u8, depth: usize) -> Result<i128, Error> {
        self.cx.tick()?;
        if depth >= self.cx.limits.max_depth.min(128) {
            return Err(self
                .cx
                .error(ErrorKind::ResourceLimit, "bound expression depth budget"));
        }
        self.space()?;
        let mut value = match self.bytes.get(self.offset).copied() {
            Some(b'+' | b'-') => {
                let negative = self.bytes[self.offset] == b'-';
                self.offset += 1;
                let v = self.expression(3, depth + 1)?;
                if negative {
                    v.checked_neg().ok_or_else(|| {
                        self.cx
                            .error(ErrorKind::InvalidMetadata, "bound arithmetic overflow")
                    })?
                } else {
                    v
                }
            }
            Some(b'(') => {
                self.offset += 1;
                let v = self.expression(0, depth + 1)?;
                self.space()?;
                if self.bytes.get(self.offset) != Some(&b')') {
                    return Err(self
                        .cx
                        .error(ErrorKind::Unsupported, "unclosed bound expression"));
                }
                self.offset += 1;
                v
            }
            Some(b'0'..=b'9') => {
                let mut v = 0i128;
                while let Some(c @ b'0'..=b'9') = self.bytes.get(self.offset) {
                    self.cx.tick()?;
                    v = v
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(i128::from(*c - b'0')))
                        .ok_or_else(|| {
                            self.cx
                                .error(ErrorKind::InvalidMetadata, "bound arithmetic overflow")
                        })?;
                    self.offset += 1;
                }
                v
            }
            _ => {
                return Err(self.cx.error(
                    ErrorKind::Unsupported,
                    "bound requires integer arithmetic without variables or functions",
                ));
            }
        };
        loop {
            self.space()?;
            let (op, precedence, len) = match self.bytes.get(self.offset).copied() {
                Some(b'+') => (b'+', 1, 1),
                Some(b'-') => (b'-', 1, 1),
                Some(b'*') => (b'*', 2, 1),
                Some(b'D' | b'd')
                    if self
                        .bytes
                        .get(self.offset..self.offset + 3)
                        .is_some_and(|s| s.eq_ignore_ascii_case(b"DIV")) =>
                {
                    (b'/', 2, 3)
                }
                Some(b'M' | b'm')
                    if self
                        .bytes
                        .get(self.offset..self.offset + 3)
                        .is_some_and(|s| s.eq_ignore_ascii_case(b"MOD")) =>
                {
                    (b'%', 2, 3)
                }
                _ => break,
            };
            if precedence < min_precedence {
                break;
            }
            if len == 3
                && self
                    .bytes
                    .get(self.offset + len)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
            {
                return Err(self
                    .cx
                    .error(ErrorKind::Unsupported, "bound contains an identifier"));
            }
            self.offset += len;
            let rhs = self.expression(precedence + 1, depth + 1)?;
            if matches!(op, b'/' | b'%') && (value < 0 || rhs < 0) {
                return Err(self
                    .cx
                    .error(ErrorKind::Unsupported, "DIV/MOD with negative operands"));
            }
            value = match op {
                b'+' => value.checked_add(rhs),
                b'-' => value.checked_sub(rhs),
                b'*' => value.checked_mul(rhs),
                b'/' => value.checked_div(rhs),
                b'%' => value.checked_rem(rhs),
                _ => unreachable!(),
            }
            .ok_or_else(|| {
                self.cx.error(
                    ErrorKind::InvalidMetadata,
                    "bound overflow or division by zero",
                )
            })?;
        }
        Ok(value)
    }
}
