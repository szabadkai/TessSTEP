use tessstep_express::*;
const BASE: &str = include_str!("../../../corpus/express/valid/base.exp");
const IMPORTS: &str = include_str!("../../../corpus/express/valid/imports.exp");
fn c(text: &str) -> Compilation {
    compile(
        &[Source {
            name: "test.exp",
            text,
        }],
        Limits::default(),
    )
}
fn fail(text: &str, code: &str) {
    let result = c(text);
    assert!(result.ir.is_none(), "unexpected success: {text}");
    assert!(
        result.diagnostics.iter().any(|d| d.code == code),
        "{text}: {:?}",
        result.diagnostics
    );
}
#[test]
fn express_lexer_spans_literals_and_comments() {
    let input = "(* a (* nested *) *)\n schema Mixed; -- comment\n'it''s é' \"00000041\" %010 12.5E-2 :<>: :=: ** <* ";
    let tokens = lex(7, input, Limits::default()).unwrap();
    assert_eq!(tokens[0].text, "SCHEMA");
    assert_eq!(
        (
            tokens[0].span.source,
            tokens[0].span.line,
            tokens[0].span.column
        ),
        (7, 2, 2)
    );
    assert_eq!(tokens[3].text, "'it''s é'");
    assert_eq!(tokens[4].kind, TokenKind::EncodedString);
    assert_eq!(tokens[5].kind, TokenKind::Binary);
    for t in tokens {
        assert!(t.span.start < t.span.end);
        assert_eq!(
            input[t.span.start..t.span.end].to_ascii_uppercase(),
            t.text.to_ascii_uppercase()
        );
    }
    for bad in [
        "(*",
        "'abc",
        "\"123\"",
        "\"00000GGG\"",
        "1e+",
        "%",
        "@",
        "é",
        "_name",
    ] {
        assert_eq!(lex(0, bad, Limits::default()).unwrap_err().code, "EX1002");
    }
}
#[test]
fn express_ast_covers_declarations_and_retains_expressions() {
    let ast = parse(3, BASE, Limits::default()).unwrap();
    assert_eq!(ast[0].declarations.len(), 7);
    let DeclarationKind::Entity(item) = &ast[0].declarations[2].kind else {
        panic!()
    };
    assert!(item.abstract_entity);
    let expr = &item.where_rules[0].expression;
    assert_eq!(expr.text, "LENGTH(name) > 0");
    assert_eq!(&BASE[expr.span.start..expr.span.end], expr.text);
    assert_eq!(expr.span.source, 3);
    assert!(matches!(
        item.attributes[1].kind,
        AttributeKind::Inverse { .. }
    ));
    let DeclarationKind::Entity(part) = &ast[0].declarations[3].kind else {
        panic!()
    };
    assert_eq!(part.supertypes[0].text, "ITEM");
    assert!(matches!(
        part.attributes[0].ty.kind,
        TypeKind::Aggregate {
            kind: AggregateKind::Array,
            optional: true,
            unique: true,
            ..
        }
    ));
    assert!(part.attributes[1].optional);
    assert!(matches!(part.attributes[2].kind, AttributeKind::Derived(_)));
    let result = c(BASE);
    assert!(result.ir.is_some(), "{:?}", result.diagnostics);
    assert!(!result.diagnostics.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Unsupported)
    );
}
#[test]
fn express_import_resolution_and_reexports() {
    let sources = [
        Source {
            name: "consumer.exp",
            text: IMPORTS,
        },
        Source {
            name: "base.exp",
            text: BASE,
        },
    ];
    let result = compile(&sources, Limits::default());
    let ir = result
        .ir
        .as_ref()
        .unwrap_or_else(|| panic!("{:?}", result.diagnostics));
    assert_eq!(ir.schemas[0].dependencies, vec![2]);
    assert_eq!(
        ir.schemas[0].symbols["COMPONENT"],
        ir.schemas[2].symbols["PART"]
    );
    assert!(ir.schemas[0].symbols.contains_key("FACTOR"));
    assert!(!ir.schemas[0].exports.contains_key("FACTOR"));
    assert_eq!(
        ir.schemas[1].symbols["COMPONENT"],
        ir.schemas[2].symbols["PART"]
    );
    assert_eq!(result, compile(&sources, Limits::default()));
    let reversed = compile(&[sources[1], sources[0]], Limits::default());
    assert!(reversed.ir.is_some());
    fail(
        "SCHEMA a; END_SCHEMA; SCHEMA b; USE FROM a (missing); END_SCHEMA;",
        "EX2003",
    );
    fail("SCHEMA a; USE FROM absent; END_SCHEMA;", "EX2003");
    fail("SCHEMA a; USE FROM a; END_SCHEMA;", "EX2003");
    fail(
        "SCHEMA a; CONSTANT x:REAL:=1; END_CONSTANT; END_SCHEMA; SCHEMA b; USE FROM a (x); END_SCHEMA;",
        "EX2003",
    );
    fail(
        "SCHEMA a; ENTITY x; END_ENTITY; END_SCHEMA; SCHEMA b; REFERENCE FROM a; END_SCHEMA; SCHEMA c; USE FROM b (x); END_SCHEMA;",
        "EX2003",
    );
}
#[test]
fn express_import_cycles_collisions_and_visibility() {
    let result = c(
        "SCHEMA a; USE FROM b; ENTITY x; END_ENTITY; END_SCHEMA; SCHEMA b; USE FROM a; ENTITY y; child:x; END_ENTITY; END_SCHEMA;",
    );
    assert!(result.ir.is_some(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.unwrap().schemas[0].symbols.len(), 2);
    fail(
        "SCHEMA a; ENTITY x; END_ENTITY; END_SCHEMA; SCHEMA b; ENTITY x; END_ENTITY; END_SCHEMA; SCHEMA c; USE FROM a; USE FROM b; END_SCHEMA;",
        "EX2004",
    );
    fail(
        "SCHEMA a; ENTITY x; END_ENTITY; END_SCHEMA; SCHEMA b; USE FROM a; TYPE x=REAL; END_TYPE; END_SCHEMA;",
        "EX2004",
    );
    fail(
        "SCHEMA a; ENTITY x; END_ENTITY; END_SCHEMA; SCHEMA b; ENTITY y; x:x; END_ENTITY; END_SCHEMA;",
        "EX2005",
    );
    let wildcard = c(
        "SCHEMA a; ENTITY x; END_ENTITY; CONSTANT k:REAL:=1; END_CONSTANT; END_SCHEMA; SCHEMA b; USE FROM a; END_SCHEMA;",
    );
    assert_eq!(wildcard.ir.unwrap().schemas[1].symbols.len(), 1);
}
#[test]
fn express_semantic_checks_and_graph_cycles() {
    for text in [
        "SCHEMA a; END_SCHEMA; SCHEMA A; END_SCHEMA;",
        "SCHEMA a; TYPE x=REAL; END_TYPE; ENTITY X; END_ENTITY; END_SCHEMA;",
        "SCHEMA a; ENTITY x; a,b:REAL; A:INTEGER; END_ENTITY; END_SCHEMA;",
        "SCHEMA a; TYPE x=ENUMERATION OF (one,ONE); END_TYPE; END_SCHEMA;",
        "SCHEMA a; TYPE x=REAL; WHERE w:TRUE; W:FALSE; END_TYPE; END_SCHEMA;",
    ] {
        fail(text, "EX2002");
    }
    fail(
        "SCHEMA a; TYPE x=SELECT (missing); END_TYPE; END_SCHEMA;",
        "EX2005",
    );
    fail(
        "SCHEMA a; TYPE x=REAL; END_TYPE; ENTITY y SUBTYPE OF (x); END_ENTITY; END_SCHEMA;",
        "EX2005",
    );
    fail(
        "SCHEMA a; CONSTANT k:REAL:=1; END_CONSTANT; TYPE x=k; END_TYPE; END_SCHEMA;",
        "EX2005",
    );
    fail(
        "SCHEMA a; TYPE x=y; END_TYPE; TYPE y=LIST OF x; END_TYPE; END_SCHEMA;",
        "EX2007",
    );
    fail(
        "SCHEMA a; TYPE x=SELECT (x); END_TYPE; END_SCHEMA;",
        "EX2007",
    );
    fail(
        "SCHEMA a; ENTITY x SUBTYPE OF (y); END_ENTITY; ENTITY y SUBTYPE OF(x); END_ENTITY; END_SCHEMA;",
        "EX2007",
    );
    fail(
        "SCHEMA a; TYPE x=ARRAY [3:1] OF REAL; END_TYPE; END_SCHEMA;",
        "EX2006",
    );
    fail(
        "SCHEMA a; ENTITY x; a:REAL; END_ENTITY; ENTITY y SUBTYPE OF(x); a:REAL; END_ENTITY; END_SCHEMA;",
        "EX2008",
    );
    let diamond = c(
        "SCHEMA a; ENTITY x; a:REAL; END_ENTITY; ENTITY y SUBTYPE OF(x); END_ENTITY; ENTITY z SUBTYPE OF(x); END_ENTITY; ENTITY w SUBTYPE OF(y,z); END_ENTITY; END_SCHEMA;",
    );
    assert!(diamond.ir.is_some(), "{:?}", diamond.diagnostics);
    assert!(
        c("SCHEMA a; ENTITY x; children:LIST OF x; END_ENTITY; END_SCHEMA;")
            .ir
            .is_some()
    );
}
#[test]
fn express_inverse_targets_are_checked() {
    assert!(c("SCHEMA a; ENTITY x; INVERSE back:SET OF y FOR owner; END_ENTITY; ENTITY y; owner:x; END_ENTITY; END_SCHEMA;").ir.is_some());
    assert!(c("SCHEMA a; ENTITY x; INVERSE back:SET OF z FOR y.owner; END_ENTITY; ENTITY y; owner:x; END_ENTITY; ENTITY z SUBTYPE OF(y); END_ENTITY; END_SCHEMA;").ir.is_some());
    for text in [
        "SCHEMA a; ENTITY x; INVERSE back:REAL FOR absent; END_ENTITY; END_SCHEMA;",
        "SCHEMA a; ENTITY x; INVERSE back:LIST OF x FOR absent; END_ENTITY; END_SCHEMA;",
        "SCHEMA a; ENTITY x; INVERSE back:x FOR absent; END_ENTITY; END_SCHEMA;",
        "SCHEMA a; ENTITY x; DERIVE owner:x:=SELF; INVERSE back:x FOR owner; END_ENTITY; END_SCHEMA;",
        "SCHEMA a; ENTITY x; INVERSE back:x FOR y.owner; END_ENTITY; ENTITY y; owner:x; END_ENTITY; END_SCHEMA;",
    ] {
        fail(text, "EX2009");
    }
}
#[test]
fn express_opaque_semantics_are_never_silent() {
    let source = include_str!("../../../corpus/express/valid/opaque.exp");
    let result = c(source);
    assert!(result.ir.is_some(), "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics.len(), 4);
    for decl in &result.schemas[0].declarations[..4] {
        let DeclarationKind::Unsupported { body, .. } = &decl.kind else {
            panic!()
        };
        assert_eq!(body.text, &source[body.span.start..body.span.end]);
    }
    let opaque =
        c("SCHEMA a; TYPE x=REAL; WHERE unchecked: not_a_function(1); END_TYPE; END_SCHEMA;");
    assert!(opaque.ir.is_some());
    assert_eq!(opaque.diagnostics[0].severity, Severity::Unsupported);
}
#[test]
fn express_malformed_input_is_rejected() {
    for text in [
        "",
        "SCHEMA a",
        "SCHEMA a;",
        "SCHEMA a; END_SCHEMA; junk",
        "SCHEMA REAL; END_SCHEMA;",
        "SCHEMA a; ENTITY x; DERIVE END_ENTITY; END_SCHEMA;",
        "SCHEMA a; TYPE x=SELECT (); END_TYPE; END_SCHEMA;",
        "SCHEMA a; TYPE x=ARRAY OF REAL; END_TYPE; END_SCHEMA;",
        "SCHEMA a; TYPE x=SET OF UNIQUE REAL; END_TYPE; END_SCHEMA;",
        "SCHEMA a; TYPE x=STRING FIXED; END_TYPE; END_SCHEMA;",
        "SCHEMA a; CONSTANT x:REAL:=; END_CONSTANT; END_SCHEMA;",
        "SCHEMA a; CONSTANT x:REAL:=(1+2]; END_CONSTANT; END_SCHEMA;",
        "SCHEMA a; CONSTANT x:REAL:=1 END_CONSTANT; END_SCHEMA;",
        "SCHEMA a; TYPE x=REAL; WHERE END_TYPE; END_SCHEMA;",
        "SCHEMA a; FUNCTION x; END_SCHEMA;",
        "SCHEMA a; TYPE x=EXTENSIBLE SELECT (foo); END_TYPE; END_SCHEMA;",
        "SCHEMA a; USE FROM b (); END_SCHEMA;",
        "SCHEMA a; ENTITY x; SELF\\parent.x:REAL; END_ENTITY; END_SCHEMA;",
    ] {
        fail(text, "EX1003");
    }
    fail(
        include_str!("../../../corpus/express/invalid/truncated.exp"),
        "EX1003",
    );
    fail(
        include_str!("../../../corpus/express/invalid/unbalanced.exp"),
        "EX1003",
    );
    fail(
        include_str!("../../../corpus/express/invalid/semantic.exp"),
        "EX2007",
    );
}
#[test]
fn express_resource_limits_cover_parsing_and_resolution() {
    let text =
        "SCHEMA sample; ENTITY node; payload : LIST OF LIST OF REAL; END_ENTITY; END_SCHEMA;";
    let limits = Limits::default();
    for l in [
        Limits {
            max_input_bytes: 1,
            ..limits
        },
        Limits {
            max_tokens: 1,
            ..limits
        },
        Limits {
            max_token_bytes: 1,
            ..limits
        },
        Limits {
            max_nesting: 1,
            ..limits
        },
        Limits {
            max_schemas: 0,
            ..limits
        },
        Limits {
            max_declarations: 0,
            ..limits
        },
        Limits {
            max_work: 0,
            ..limits
        },
    ] {
        let result = compile(&[Source { name: "a", text }], l);
        assert!(result.ir.is_none());
        assert!(
            result.diagnostics.iter().any(|d| d.code == "EX1001"),
            "{:?}",
            result.diagnostics
        );
    }
    assert_eq!(
        lex(
            0,
            "(* (* nested *) *)",
            Limits {
                max_nesting: 1,
                ..limits
            }
        )
        .unwrap_err()
        .code,
        "EX1001"
    );
    let deep = format!(
        "SCHEMA a; TYPE x={}REAL; END_TYPE; END_SCHEMA;",
        "LIST OF ".repeat(1000)
    );
    assert_eq!(
        parse(
            0,
            &deep,
            Limits {
                max_nesting: usize::MAX,
                ..limits
            }
        )
        .unwrap_err()
        .code,
        "EX1001"
    );
    let deep_expr = format!(
        "SCHEMA a; CONSTANT x:REAL:={}1{}; END_CONSTANT; END_SCHEMA;",
        "(".repeat(1000),
        ")".repeat(1000)
    );
    assert_eq!(parse(0, &deep_expr, limits).unwrap_err().code, "EX1001");
    let one = "SCHEMA a; END_SCHEMA;";
    let two = "SCHEMA b; END_SCHEMA;";
    let result = compile(
        &[
            Source {
                name: "a",
                text: one,
            },
            Source {
                name: "b",
                text: two,
            },
        ],
        Limits {
            max_tokens: 6,
            ..limits
        },
    );
    assert!(result.ir.is_none());
    assert_eq!(result.diagnostics[0].code, "EX1001");
    let mut chain = String::from("SCHEMA a; ENTITY n0; END_ENTITY;");
    for n in 1..100 {
        chain.push_str(&format!("ENTITY n{n} SUBTYPE OF(n{}); END_ENTITY;", n - 1));
    }
    chain.push_str("END_SCHEMA;");
    let result = compile(
        &[Source {
            name: "chain",
            text: &chain,
        }],
        Limits {
            max_work: 1000,
            ..limits
        },
    );
    assert!(result.ir.is_none());
    assert!(result.diagnostics.iter().any(|d| d.code == "EX1001"));
}

#[test]
fn express_ir_resolves_attribute_domains_and_preserves_rule_source() {
    let text = "SCHEMA a; TYPE scalar=REAL; END_TYPE; ENTITY node; left,right:OPTIONAL scalar; DERIVE label:STRING:='END_ENTITY;(' || (* trivia *) 'é'; END_ENTITY; END_SCHEMA;";
    let c = c(text);
    let ir = c.ir.unwrap();
    let scalar = ir.schemas[0].symbols["SCALAR"];
    let IrKind::Entity { attributes, .. } = &ir.declarations[1].kind else {
        panic!()
    };
    assert_eq!(attributes.len(), 3);
    assert!(attributes[0].optional && attributes[1].optional);
    assert_eq!(attributes[0].ty, IrType::Named(scalar));
    let AttributeKind::Derived(expr) = &attributes[2].kind else {
        panic!()
    };
    assert_eq!(expr.text, "'END_ENTITY;(' || (* trivia *) 'é'");
    assert_eq!(&text[expr.span.start..expr.span.end], expr.text);
    fail(
        "SCHEMA a; TYPE collection=LIST OF x; END_TYPE; ENTITY x; owner:x; INVERSE back:collection FOR owner; END_ENTITY; END_SCHEMA;",
        "EX2009",
    );
    fail(
        "SCHEMA a; TYPE collection=SET OF x; END_TYPE; ENTITY x; owner:x; INVERSE back:SET OF collection FOR owner; END_ENTITY; END_SCHEMA;",
        "EX2009",
    );
}
#[test]
fn express_expansion_budgets_are_enforced_before_copying() {
    let text = format!(
        "SCHEMA a; ENTITY x; {} : STRING(123456789); END_ENTITY; END_SCHEMA;",
        (0..100)
            .map(|n| format!("a{n}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let result = compile(
        &[Source {
            name: "a",
            text: &text,
        }],
        Limits {
            max_work: 1000,
            ..Limits::default()
        },
    );
    assert!(result.ir.is_none());
    assert!(result.diagnostics[0].message.contains("AST expansion"));
    let name = "a".repeat(500);
    let text = format!(
        "SCHEMA a; ENTITY {name}; END_ENTITY; END_SCHEMA; SCHEMA b; USE FROM a; END_SCHEMA;"
    );
    let result = compile(
        &[Source {
            name: "a",
            text: &text,
        }],
        Limits {
            max_work: 1000,
            ..Limits::default()
        },
    );
    assert!(result.ir.is_none());
    assert!(result.diagnostics.iter().any(|d| d.code == "EX1001"));
}
