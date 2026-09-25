use std::io::{self, BufReader, Cursor, Read};
use tessstep_part21::*;

fn file(body: &str) -> String {
    format!(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION(('test'),'2;1');FILE_NAME('','',('a'),('o'),'','','');FILE_SCHEMA(('TEST'));ENDSEC;DATA;{body}ENDSEC;END-ISO-10303-21;"
    )
}
fn events(text: &str) -> Result<Vec<Event>, Diagnostic> {
    Parser::new(text.as_bytes(), ParseLimits::default())?.collect()
}
fn values(text: &str) -> Vec<StepValue> {
    events(&file(&format!("#1=A({text});")))
        .unwrap()
        .into_iter()
        .find_map(|event| {
            if let Event::Entity(EntityInstance {
                kind: EntityKind::Simple(record),
                ..
            }) = event
            {
                Some(record.parameters)
            } else {
                None
            }
        })
        .unwrap()
}
fn fail(body: &str, code: DiagnosticCode) {
    let error = events(&file(body)).unwrap_err();
    assert_eq!(error.code, code, "{body}: {error}");
}
#[test]
fn all_value_categories() {
    let values = values("$,*,-42,+0,1.,-2.5E+3,'a''b',.T.,#99,(),((1),2),LABEL(3),#C,@V,@8,\"31\"");
    assert_eq!(values.len(), 16);
    assert_eq!(values[0].kind, ValueKind::Null);
    assert_eq!(values[1].kind, ValueKind::Omitted);
    assert_eq!(values[2].kind, ValueKind::Integer(-42));
    assert_eq!(values[4].kind, ValueKind::Real(1.0));
    assert_eq!(values[5].kind, ValueKind::Real(-2500.0));
    assert_eq!(values[6].kind, ValueKind::String("a'b".into()));
    assert_eq!(
        values[8].kind,
        ValueKind::Reference(EntityId::new(99).unwrap())
    );
    assert!(matches!(values[11].kind, ValueKind::Typed { .. }));
    assert_eq!(
        values[15].kind,
        ValueKind::Binary(BinaryValue {
            bytes: vec![128],
            bit_len: 1
        })
    );
}
#[test]
fn numeric_boundaries_and_underflow() {
    let parsed =
        values("-9223372036854775808,9223372036854775807,-0.,4.9406564584124654E-324,0.E99999");
    assert_eq!(parsed[0].kind, ValueKind::Integer(i64::MIN));
    assert_eq!(parsed[1].kind, ValueKind::Integer(i64::MAX));
    if let ValueKind::Real(number) = parsed[2].kind {
        assert!(number.is_sign_negative());
    }
    for literal in [
        "9223372036854775808",
        "-9223372036854775809",
        "1.E309",
        "1.E-9999",
    ] {
        fail(&format!("#1=A({literal});"), DiagnosticCode::NumericRange);
    }
    for literal in [".5", "1E3", "1.0e3", "1.E+", "+", "NaN", "INF", "1.2.3"] {
        assert!(
            events(&file(&format!("#1=A({literal});"))).is_err(),
            "{literal}"
        );
    }
}
#[test]
fn strings_and_page_reset() {
    let parsed = values(
        r"'Don''t','\\','\S\D','\PB\\S\!','\S\!','\X\00','\X2\03A903BB\X0\','\X4\0001F642\X0\','árvíz','\S\''",
    );
    let expected = ["Don't", "\\", "Ä", "Ą", "¡", "\0", "Ωλ", "🙂", "árvíz", "§"];
    for (value, expected) in parsed.iter().zip(expected) {
        assert_eq!(value.kind, ValueKind::String(expected.into()));
    }
    let pages =
        values(r"'\PC\\S\!','\PD\\S\!','\PE\\S\*','\PF\\S\,','\PG\\S\A','\PH\\S\`','\PI\\S\P'");
    for (value, expected) in pages.iter().zip(["Ħ", "Ą", "Њ", "،", "Α", "א", "Ğ"]) {
        assert_eq!(value.kind, ValueKind::String(expected.into()));
    }
}
#[test]
fn rejects_malformed_string_directives() {
    for text in [
        r"'\Q\'",
        r"'\X2\\X0\'",
        r"'\X2\123\X0\'",
        r"'\X2\D800\X0\'",
        r"'\X4\00110000\X0\'",
        r"'\X\GG'",
        r"'\PJ\\S\A'",
        r"'\PF\\S\!'",
        r"'\X2\0041'",
        r"'\PB!'",
        r"'\N!'",
        r"'\X2\0041\Y0\'",
    ] {
        assert!(events(&file(&format!("#1=A({text});"))).is_err(), "{text}");
    }
    let input = file("#1=A('x');").replace("'x'", "'\u{fffd}'").into_bytes();
    let mut invalid = input;
    let i = invalid.iter().position(|&b| b == 0xef).unwrap();
    invalid[i] = 0xff;
    assert!(
        Parser::new(invalid.as_slice(), ParseLimits::default())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .is_err()
    );
}
#[test]
fn binaries_preserve_exact_bits() {
    let parsed = values("\"0\",\"30\",\"31\",\"23B\",\"092A\",\"0ABC\"");
    let expected = [
        (vec![], 0),
        (vec![0], 1),
        (vec![128], 1),
        (vec![236], 6),
        (vec![146, 160], 12),
        (vec![171, 192], 12),
    ];
    for (value, (bytes, bit_len)) in parsed.iter().zip(expected) {
        assert_eq!(
            value.kind,
            ValueKind::Binary(BinaryValue { bytes, bit_len })
        );
    }
    for text in ["\"\"", "\"1\"", "\"4A\"", "\"3F\"", "\"0aa\"", "\"0 A\""] {
        fail(&format!("#1=A({text});"), DiagnosticCode::InvalidBinary);
    }
}
#[test]
fn complex_instances_and_user_defined_names() {
    let parsed = events(&file("#9=(A() B($) C(#1));#1=!VENDOR(!TYPE(2));")).unwrap();
    assert!(
        matches!(&parsed[4], Event::Entity(EntityInstance { kind: EntityKind::Complex(records), .. }) if records.len() == 3)
    );
    fail("#1=();", DiagnosticCode::InvalidComplexInstance);
    fail("#1=(A() A());", DiagnosticCode::InvalidComplexInstance);
    for bad in [
        "#1=(A(),B());",
        "#1=(A()); extra",
        "#1=A(TYPE());",
        "#1=A(TYPE(1,2));",
        "#1=A(,);",
        "#1=A(1,);",
        "#1=A(1 2);",
        "#1=a();",
    ] {
        assert!(events(&file(bad)).is_err(), "{bad}");
    }
}
#[test]
fn sparse_nonzero_identifiers() {
    assert!(events(&file("#18446744073709551615=A(#00001);#1=B();")).is_ok());
    for bad in [
        "#0=A();",
        "#18446744073709551616=A();",
        "#-1=A();",
        "#=A();",
        "@1=A();",
    ] {
        assert!(events(&file(bad)).is_err(), "{bad}");
    }
}
#[test]
fn every_reader_chunk_size_has_identical_result() {
    let input = include_bytes!("../../../corpus/part21/valid/unicode.step");
    let expected = Parser::new(input.as_slice(), ParseLimits::default())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for size in 1..=64 {
        let reader = BufReader::with_capacity(size, Cursor::new(input));
        let actual = Parser::new(reader, ParseLimits::default())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(actual, expected, "chunk size {size}");
    }
}
#[test]
fn byte_spans_crlf_and_ignored_controls() {
    let mut lexer = Lexer::new(
        b" \r\n/*c*/ #12=ABC('a\nb');".as_slice(),
        ParseLimits::default(),
    );
    let token = lexer.next_token().unwrap();
    assert_eq!(
        token.source.start,
        SourcePosition {
            offset: 9,
            line: 2,
            column: 7
        }
    );
    assert_eq!(token.source.end.offset, 12);
    for _ in 0..3 {
        lexer.next_token().unwrap();
    }
    let string = lexer.next_token().unwrap();
    assert_eq!(string.kind, TokenKind::String("ab".into()));
    assert_eq!(string.source.end.line, 3);
    assert_eq!(
        values("'a\\N\\b\\F\\c',\"0A\\N\\B\"")[0].kind,
        ValueKind::String("abc".into())
    );
    assert!(events(&file("/* pre */#1=A(1\n2);\\N\\")).is_ok());
}
#[test]
fn section_order_and_trailing_bytes_are_checked() {
    let input = include_str!("../../../corpus/part21/valid/extended.step");
    assert!(events(input).is_ok());
    for bad in [
        input.replace("TWE=", "TW=="),
        input.replace("TWE=", "TWE"),
        input.replace("SIGNATURE", "SIGNATURE;"),
        format!("{input}GARBAGE"),
        input.replace("REFERENCE;", "ANCHOR;"),
        input.replace("<part>", "<123>"),
    ] {
        assert!(events(&bad).is_err(), "{bad}");
    }
    assert!(events(&file("").replace("DATA;", "DATA();")).is_err());
    assert!(events(&file("").replace("END-ISO-10303-21;", "")).is_err());
}
#[test]
fn anchor_parameters_differ_from_entity_parameters() {
    let source = include_str!("../../../corpus/part21/valid/extended.step");
    for bad in [
        source.replace("<part>=#1", "<part>=*"),
        source.replace("<part>=#1", "<part>=TYPE(1)"),
        source.replace("<part>=#1", "<part>=(*,1)"),
        source.replace("#1=NODE(@2,#3,#CONSTANT,@VALUE)", "#1=NODE(<resource>)"),
    ] {
        assert!(events(&bad).is_err());
    }
    assert!(events(&source.replace("<part>=#1", "<part>=(<a>,1,($,#1))")).is_ok());
    assert!(events(&source.replace("<units.step#unit>", "<units%2Estep#unit>")).is_ok());
    assert!(events(&source.replace("<units.step#unit>", "<units%2Gstep#unit>")).is_err());
}
#[test]
fn required_header_order_and_shape() {
    let source = file("");
    for bad in [
        source.replace("FILE_SCHEMA(('TEST'))", "FILE_SCHEMA('TEST')"),
        source.replace("FILE_NAME", "FILE_NAM"),
        source.replace(
            "FILE_DESCRIPTION(('test'),'2;1')",
            "FILE_DESCRIPTION((),'2;1')",
        ),
        source.replace("FILE_SCHEMA(('TEST'));", ""),
        source.replace("ENDSEC;DATA;", "FILE_SCHEMA(('TEST'));ENDSEC;DATA;"),
    ] {
        assert_eq!(
            events(&bad).unwrap_err().code,
            DiagnosticCode::InvalidHeader
        );
    }
}
#[test]
fn every_resource_budget_is_enforced() {
    let input = file("#1=A('abcdef',(1,2),T(3));#2=B();");
    let budgets = [
        ParseLimits {
            max_input_bytes: 10,
            ..Default::default()
        },
        ParseLimits {
            max_token_bytes: 3,
            ..Default::default()
        },
        ParseLimits {
            max_string_bytes: 3,
            ..Default::default()
        },
        ParseLimits {
            max_entities: 1,
            ..Default::default()
        },
        ParseLimits {
            max_nesting_depth: 1,
            ..Default::default()
        },
        ParseLimits {
            max_aggregate_elements: 1,
            ..Default::default()
        },
        ParseLimits {
            max_total_values: 2,
            ..Default::default()
        },
        ParseLimits {
            max_symbols: 1,
            ..Default::default()
        },
        ParseLimits {
            max_records: 1,
            ..Default::default()
        },
        ParseLimits {
            max_sections: 1,
            ..Default::default()
        },
    ];
    for limits in budgets {
        let error = Parser::new(input.as_bytes(), limits)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap_err();
        assert_eq!(
            error.code,
            DiagnosticCode::LimitExceeded,
            "{limits:?}: {error}"
        );
    }
    let limits = ParseLimits {
        max_input_bytes: input.len() as u64,
        ..Default::default()
    };
    assert!(
        Parser::new(input.as_bytes(), limits)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .is_ok()
    );
    assert!(
        Parser::new(
            input.as_bytes(),
            ParseLimits {
                max_nesting_depth: usize::MAX,
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn deep_aggregates_and_typed_parameters_fail_without_stack_exhaustion() {
    for (open, close) in [("(", ")"), ("T(", ")")] {
        let text = file(&format!(
            "#1=A({}1{});",
            open.repeat(5000),
            close.repeat(5000)
        ));
        assert_eq!(
            events(&text).unwrap_err().code,
            DiagnosticCode::LimitExceeded
        );
    }
}
#[test]
fn parser_and_lexer_fuse_after_failure() {
    let mut parser = Parser::new(b"bad".as_slice(), ParseLimits::default()).unwrap();
    assert!(parser.next().unwrap().is_err());
    assert!(parser.next().is_none());
    assert!(parser.next().is_none());
    let mut lexer = Lexer::new(b"?".as_slice(), ParseLimits::default());
    assert!(lexer.next_token().is_err());
    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Eof);
    assert_eq!(
        events(&file("#1=&SCOPE")).unwrap_err().code,
        DiagnosticCode::UnsupportedFeature
    );
}
#[derive(Debug)]
struct Faulty {
    bytes: Cursor<Vec<u8>>,
    fail_at: usize,
    interrupted: bool,
}
impl Read for Faulty {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.bytes.position() as usize >= self.fail_at {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let len = out.len().min(self.fail_at - self.bytes.position() as usize);
        self.bytes.read(&mut out[..len])
    }
}
#[test]
fn interrupted_reads_retry_and_io_failure_is_structured() {
    let input = file("#1=A();");
    let reader = Faulty {
        bytes: Cursor::new(input.into_bytes()),
        fail_at: 140,
        interrupted: false,
    };
    let error = Parser::new(BufReader::with_capacity(1, reader), ParseLimits::default())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap_err();
    assert_eq!(error.code, DiagnosticCode::Io);
}
#[test]
fn every_truncation_is_rejected_except_trailing_whitespace() {
    let input = file("#1=A('hello',T((1,2))); ");
    for end in 0..input.len() {
        assert!(
            events(&input[..end]).is_err(),
            "accepted truncation at {end}"
        );
    }
    assert!(events(&input).is_ok());
}

#[test]
fn shared_names_are_interned_and_header_extensions_are_retained() {
    use std::sync::Arc;
    let text =
        file("#1=A(.T.);#2=A(.T.);").replace("ENDSEC;DATA;", "VENDOR_HEADER('kept');ENDSEC;DATA;");
    let result = events(&text).unwrap();
    assert!(
        result
            .iter()
            .any(|e| matches!(e, Event::Header(r) if r.name.as_ref() == "VENDOR_HEADER"))
    );
    let records: Vec<_> = result
        .iter()
        .filter_map(|e| match e {
            Event::Entity(entity) => Some(&entity.kind.records()[0]),
            _ => None,
        })
        .collect();
    assert!(Arc::ptr_eq(&records[0].name, &records[1].name));
}
#[test]
fn first_event_does_not_read_the_whole_file() {
    use std::{cell::Cell, rc::Rc};
    struct Counted {
        input: Cursor<Vec<u8>>,
        read: Rc<Cell<usize>>,
    }
    impl Read for Counted {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let n = self.input.read(out)?;
            self.read.set(self.read.get() + n);
            Ok(n)
        }
    }
    let text = file(&"#1=A();".repeat(10000));
    let read = Rc::new(Cell::new(0));
    let source = Counted {
        input: Cursor::new(text.into_bytes()),
        read: read.clone(),
    };
    let mut parser =
        Parser::new(BufReader::with_capacity(32, source), ParseLimits::default()).unwrap();
    assert!(matches!(parser.next().unwrap().unwrap(), Event::Header(_)));
    assert!(
        read.get() < 128,
        "read {} bytes before first event",
        read.get()
    );
}
#[test]
fn exact_string_and_token_limits_and_maximum_safe_depth() {
    let mut lexer = Lexer::new(
        "'é'".as_bytes(),
        ParseLimits {
            max_string_bytes: 2,
            max_token_bytes: 4,
            ..Default::default()
        },
    );
    assert_eq!(
        lexer.next_token().unwrap().kind,
        TokenKind::String("é".into())
    );
    let mut lexer = Lexer::new(
        "'é'".as_bytes(),
        ParseLimits {
            max_string_bytes: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        lexer.next_token().unwrap_err().code,
        DiagnosticCode::LimitExceeded
    );
    let mut lexer = Lexer::new(
        b"'a'".as_slice(),
        ParseLimits {
            max_token_bytes: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        lexer.next_token().unwrap_err().code,
        DiagnosticCode::LimitExceeded
    );
    let input = file(&format!("#1=A({}1{});", "(".repeat(127), ")".repeat(127)));
    assert!(
        Parser::new(
            input.as_bytes(),
            ParseLimits {
                max_nesting_depth: 128,
                ..Default::default()
            }
        )
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .is_ok()
    );
}
#[test]
fn all_single_nibble_binary_encodings() {
    for unused in 0..4 {
        for nibble in 0..16u8 {
            let text = format!("\"{unused}{nibble:X}\"");
            let token = Lexer::new(text.as_bytes(), ParseLimits::default()).next_token();
            if nibble >> (4 - unused) != 0 {
                assert!(token.is_err());
            } else {
                assert_eq!(
                    token.unwrap().kind,
                    TokenKind::Binary(BinaryValue {
                        bit_len: 4 - unused,
                        bytes: vec![nibble << (4 + unused)]
                    })
                );
            }
        }
    }
}
