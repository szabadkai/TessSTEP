use crate::{DiagnosticCode as Code, lexer::hex};

// ISO-8859 character mappings are Unicode data, not EXPRESS schema definitions.
mod pages {
    include!("string_pages.rs");
}

pub(crate) fn decode(raw: &[u8], max_bytes: usize) -> Result<String, (Code, &'static str)> {
    let mut output = String::new();
    let mut cursor = 0;
    let mut page = 0;
    while cursor < raw.len() {
        let character = match raw[cursor] {
            b'\'' => {
                if raw.get(cursor + 1) != Some(&b'\'') {
                    return Err((Code::InvalidEscape, "unpaired apostrophe"));
                }
                cursor += 2;
                Some('\'')
            }
            b'\\' => {
                cursor += 1;
                let directive = take(raw, &mut cursor)?;
                match directive {
                    b'\\' => Some('\\'),
                    b'N' | b'F' => {
                        expect(raw, &mut cursor, b'\\')?;
                        None
                    }
                    b'P' => {
                        let alphabet = take(raw, &mut cursor)?;
                        if !(b'A'..=b'I').contains(&alphabet) {
                            return Err((Code::InvalidEscape, "unsupported ISO-8859 alphabet"));
                        }
                        page = usize::from(alphabet - b'A');
                        expect(raw, &mut cursor, b'\\')?;
                        None
                    }
                    b'S' => {
                        expect(raw, &mut cursor, b'\\')?;
                        let byte = take(raw, &mut cursor)?;
                        if !(0x20..=0x7e).contains(&byte) {
                            return Err((
                                Code::InvalidEscape,
                                "page payload must be a basic character",
                            ));
                        }
                        let scalar = pages::PAGES[page][usize::from(byte - 0x20)];
                        if scalar == 0 {
                            return Err((Code::InvalidEscape, "undefined ISO-8859 character"));
                        }
                        char::from_u32(scalar)
                    }
                    b'X' => {
                        let mode = take(raw, &mut cursor)?;
                        match mode {
                            b'\\' => Some(scalar(raw, &mut cursor, 2)?),
                            b'2' | b'4' => {
                                expect(raw, &mut cursor, b'\\')?;
                                let mut count = 0;
                                while raw.get(cursor) != Some(&b'\\') {
                                    let c =
                                        scalar(raw, &mut cursor, if mode == b'2' { 4 } else { 8 })?;
                                    push(&mut output, c, max_bytes)?;
                                    count += 1;
                                }
                                if count == 0 {
                                    return Err((
                                        Code::InvalidEscape,
                                        "empty extended string directive",
                                    ));
                                }
                                for byte in b"\\X0\\" {
                                    expect(raw, &mut cursor, *byte)?;
                                }
                                None
                            }
                            _ => return Err((Code::InvalidEscape, "unknown X string directive")),
                        }
                    }
                    _ => return Err((Code::InvalidEscape, "unknown string directive")),
                }
            }
            _ => {
                let begin = cursor;
                while cursor < raw.len() && !matches!(raw[cursor], b'\'' | b'\\') {
                    cursor += 1;
                }
                let text = std::str::from_utf8(&raw[begin..cursor])
                    .map_err(|_| (Code::InvalidEscape, "invalid UTF-8 string"))?;
                if output.len().saturating_add(text.len()) > max_bytes {
                    return Err((Code::LimitExceeded, "decoded string byte budget exceeded"));
                }
                output.push_str(text);
                None
            }
        };
        if let Some(c) = character {
            push(&mut output, c, max_bytes)?;
        }
    }
    Ok(output)
}
fn take(raw: &[u8], cursor: &mut usize) -> Result<u8, (Code, &'static str)> {
    let byte = raw
        .get(*cursor)
        .copied()
        .ok_or((Code::InvalidEscape, "incomplete string directive"))?;
    *cursor += 1;
    Ok(byte)
}
fn expect(raw: &[u8], cursor: &mut usize, byte: u8) -> Result<(), (Code, &'static str)> {
    if take(raw, cursor)? == byte {
        Ok(())
    } else {
        Err((Code::InvalidEscape, "malformed string directive"))
    }
}
fn scalar(raw: &[u8], cursor: &mut usize, count: usize) -> Result<char, (Code, &'static str)> {
    let mut value = 0u32;
    for _ in 0..count {
        value = value * 16
            + u32::from(
                hex(take(raw, cursor)?)
                    .ok_or((Code::InvalidEscape, "invalid hexadecimal digit"))?,
            );
    }
    char::from_u32(value).ok_or((
        Code::InvalidEscape,
        "invalid Unicode scalar (surrogates are not UCS characters)",
    ))
}
fn push(output: &mut String, c: char, max_bytes: usize) -> Result<(), (Code, &'static str)> {
    if output.len().saturating_add(c.len_utf8()) > max_bytes {
        return Err((Code::LimitExceeded, "decoded string byte budget exceeded"));
    }
    output.push(c);
    Ok(())
}
