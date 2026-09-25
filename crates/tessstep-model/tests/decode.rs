use tessstep_model::{
    Document,
    decode::{self, ErrorKind, Limits},
};
use tessstep_part21::{EntityId, ParseLimits};
use tessstep_schema::*;
const SPAN: SourceSpan = SourceSpan {
    source: 0,
    start: 0,
    end: 0,
    line: 1,
    column: 1,
};
const INT: Domain = Domain::Builtin {
    kind: Builtin::Integer,
    width: None,
    fixed: false,
};
const fn expr(text: &'static str) -> Expression {
    Expression { text, span: SPAN }
}
const fn attr(name: &'static str, domain: Domain, optional: bool) -> Attribute {
    Attribute {
        name,
        span: SPAN,
        domain,
        optional,
        kind: AttributeKind::Explicit,
    }
}
const fn entity(
    name: &'static str,
    parents: &'static [DeclarationId],
    attributes: &'static [Attribute],
) -> Declaration {
    Declaration {
        name,
        schema: 0,
        span: SPAN,
        kind: DeclarationKind::Entity {
            abstract_entity: false,
            supertype_constraint: None,
            supertypes: parents,
            attributes,
            unique: &[],
            where_rules: &[],
        },
    }
}
const BASE: Declaration = entity(
    "BASE",
    &[],
    &[
        attr("number", INT, false),
        attr("next", Domain::Named(DeclarationId(0)), true),
    ],
);
const CHILD: Declaration = entity(
    "CHILD",
    &[DeclarationId(0)],
    &[attr(
        "values",
        Domain::Aggregate {
            kind: AggregateKind::Array,
            bounds: Some((expr("-1"), expr("1"))),
            optional: true,
            unique: false,
            element: &INT,
        },
        false,
    )],
);
static SCHEMAS: SchemaSet = SchemaSet {
    sources: &["authored.exp"],
    schemas: &[Schema {
        name: "TEST",
        span: SPAN,
        dependencies: &[],
        exports: &[],
        symbols: &[
            Symbol {
                name: "BASE",
                declaration: DeclarationId(0),
            },
            Symbol {
                name: "CHILD",
                declaration: DeclarationId(1),
            },
            Symbol {
                name: "OTHER",
                declaration: DeclarationId(2),
            },
            Symbol {
                name: "ALIAS",
                declaration: DeclarationId(1),
            },
        ],
    }],
    declarations: &[BASE, CHILD, entity("OTHER", &[], &[])],
    unsupported: &[],
};
fn text(body: &str) -> String {
    include_str!("../../../corpus/part21/valid/empty.step")
        .replace("DATA;", &format!("DATA;{body}"))
}
fn parse(body: &str) -> Document {
    tessstep_model::parse(text(body).as_bytes(), ParseLimits::default()).unwrap()
}
fn error(body: &str) -> decode::Error {
    decode::decode(&parse(body), &SCHEMAS, "test", Limits::default()).unwrap_err()
}
fn with_domain(domain: Domain) -> SchemaSet {
    let attrs = Box::leak(vec![attr("value", domain, false)].into_boxed_slice());
    SchemaSet {
        declarations: Box::leak(vec![entity("BASE", &[], attrs)].into_boxed_slice()),
        ..SCHEMAS
    }
}
fn check(domain: Domain, value: &str) -> Result<(), decode::Error> {
    decode::decode(
        &parse(&format!("#1=BASE({value});")),
        &with_domain(domain),
        "test",
        Limits::default(),
    )
    .map(|_| ())
}
#[test]
fn schema_decode_borrows_attributes_and_checks_forward_subtype_references() {
    let doc = parse("#9=BASE(2,#1);#1=ALIAS(3,#9,(1,$,3));");
    let decoded = decode::decode(&doc, &SCHEMAS, "TeSt", Limits::default()).unwrap();
    assert!(std::ptr::eq(decoded.document(), &doc));
    assert!(std::ptr::eq(decoded.schemas(), &SCHEMAS));
    assert_eq!(decoded.entities().len(), 2);
    let child = decoded.get(EntityId::new(1).unwrap()).unwrap();
    assert_eq!(child.declaration, DeclarationId(1));
    assert_eq!(
        child
            .attributes
            .iter()
            .map(|a| a.declaration.name)
            .collect::<Vec<_>>(),
        ["number", "next", "values"]
    );
    assert!(std::ptr::eq(
        child.attributes[0].value,
        &doc.entities().get(child.id).unwrap().kind.records()[0].parameters[0]
    ));
    assert!(decoded.get(EntityId::new(42).unwrap()).is_none());
}
#[test]
fn schema_decode_rejects_invalid_instances_with_precise_context() {
    for (body, expected) in [
        ("#1=UNKNOWN();", ErrorKind::UnknownEntity),
        ("#1=BASE(1);", ErrorKind::AttributeCount),
        ("#1=BASE($,$);", ErrorKind::RequiredValue),
        ("#1=BASE(*,$);", ErrorKind::TypeMismatch),
        ("#1=BASE(1.5,$);", ErrorKind::TypeMismatch),
        ("#1=BASE(1,#8);", ErrorKind::MissingReference),
        ("#1=BASE(1,#8);#8=OTHER();", ErrorKind::ReferenceType),
        ("#1=CHILD(1,$,(1,2));", ErrorKind::Cardinality),
        ("#1=CHILD(1,$,(1,'bad',3));", ErrorKind::TypeMismatch),
    ] {
        let result = error(body);
        assert_eq!(result.kind, expected, "{body}: {result}");
        assert_eq!(result.entity, EntityId::new(1));
        assert_eq!(result, error(body));
    }
    let input = text("#1=CHILD(1,$,(1,'bad',3));");
    let result = error("#1=CHILD(1,$,(1,'bad',3));");
    let span = result.source.unwrap();
    assert_eq!(
        &input[span.start.offset as usize..span.end.offset as usize],
        "'bad'"
    );
    assert_eq!(result.attribute, Some("values"));
}
#[test]
fn schema_decode_primitives_widths_and_aggregate_bounds() {
    for (kind, good, bad) in [
        (Builtin::Boolean, ".T.", ".U."),
        (Builtin::Logical, ".U.", ".OTHER."),
        (Builtin::Integer, "-9", "1.2"),
        (Builtin::Real, "1", "'1'"),
        (Builtin::Number, "1.5", ".T."),
        (Builtin::String, "'a'", "1"),
        (Builtin::Binary, "\"0F\"", "'a'"),
    ] {
        let domain = Domain::Builtin {
            kind,
            width: None,
            fixed: false,
        };
        assert!(check(domain, good).is_ok(), "{kind:?}");
        assert_eq!(
            check(domain, bad).unwrap_err().kind,
            ErrorKind::TypeMismatch
        );
    }
    assert!(check(Domain::Enumeration(&["RED", "BLUE"]), ".RED.").is_ok());
    assert!(check(Domain::Enumeration(&["RED", "BLUE"]), ".GREEN.").is_err());
    let string = Domain::Builtin {
        kind: Builtin::String,
        width: Some(expr("2")),
        fixed: true,
    };
    assert!(check(string, "'é😀'").is_ok());
    assert_eq!(check(string, "'a'").unwrap_err().kind, ErrorKind::Width);
    let binary = Domain::Builtin {
        kind: Builtin::Binary,
        width: Some(expr("3")),
        fixed: true,
    };
    assert!(check(binary, "\"17\"").is_ok());
    assert_eq!(check(binary, "\"0F\"").unwrap_err().kind, ErrorKind::Width);
    for kind in [AggregateKind::List, AggregateKind::Bag] {
        let domain = Domain::Aggregate {
            kind,
            bounds: Some((expr("1"), expr("?"))),
            optional: false,
            unique: false,
            element: &INT,
        };
        assert!(check(domain, "(1,2,2)").is_ok());
        assert_eq!(
            check(domain, "()").unwrap_err().kind,
            ErrorKind::Cardinality
        );
        assert_eq!(
            check(domain, "($)").unwrap_err().kind,
            ErrorKind::RequiredValue
        );
        let bounded = Domain::Aggregate {
            kind,
            bounds: Some((expr("1"), expr("2"))),
            optional: false,
            unique: false,
            element: &INT,
        };
        assert_eq!(
            check(bounded, "(1,2,3)").unwrap_err().kind,
            ErrorKind::Cardinality
        );
    }
}
#[test]
fn schema_decode_unsupported_semantics_never_pass_silently() {
    assert_eq!(error("#1=(BASE(1,$)OTHER());").kind, ErrorKind::Unsupported);
    for domain in [
        Domain::Select(&[DeclarationId(0)]),
        Domain::Aggregate {
            kind: AggregateKind::Set,
            bounds: None,
            optional: false,
            unique: false,
            element: &INT,
        },
        Domain::Aggregate {
            kind: AggregateKind::List,
            bounds: Some((expr("1+1"), expr("?"))),
            optional: false,
            unique: false,
            element: &INT,
        },
    ] {
        assert_eq!(
            check(domain, "(1,2)").unwrap_err().kind,
            ErrorKind::Unsupported
        );
    }
    let mut schema = SCHEMAS;
    schema.unsupported = &[UnsupportedDiagnostic {
        code: "test",
        span: SPAN,
        message: "opaque",
    }];
    assert_eq!(
        decode::decode(&parse(""), &schema, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    schema = SCHEMAS;
    schema.declarations = Box::leak(
        vec![
            entity("BASE", &[DeclarationId(1), DeclarationId(2)], &[]),
            CHILD,
            entity("OTHER", &[], &[]),
        ]
        .into_boxed_slice(),
    );
    assert_eq!(
        decode::decode(&parse("#1=BASE();"), &schema, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    let derived = Attribute {
        kind: AttributeKind::Derived(expr("1")),
        ..attr("derived", INT, false)
    };
    let attrs = Box::leak(vec![derived].into_boxed_slice());
    schema.declarations = Box::leak(vec![entity("BASE", &[], attrs)].into_boxed_slice());
    assert_eq!(
        decode::decode(&parse("#1=BASE();"), &schema, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
}
#[test]
fn schema_decode_limits_and_malformed_metadata_are_bounded() {
    let doc = parse("#1=BASE(1,#1);");
    for limits in [
        Limits {
            max_work: 0,
            ..Limits::default()
        },
        Limits {
            max_depth: 0,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            decode::decode(&doc, &SCHEMAS, "test", limits)
                .unwrap_err()
                .kind,
            ErrorKind::ResourceLimit
        );
    }
    assert_eq!(
        decode::decode(&doc, &SCHEMAS, "absent", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::UnknownSchema
    );
    let mut schema = SCHEMAS;
    schema.declarations =
        Box::leak(vec![entity("BASE", &[DeclarationId(0)], &[])].into_boxed_slice());
    assert_eq!(
        decode::decode(&doc, &schema, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::ResourceLimit
    );
    schema.declarations =
        Box::leak(vec![entity("BASE", &[DeclarationId(usize::MAX)], &[])].into_boxed_slice());
    assert_eq!(
        decode::decode(&doc, &schema, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::InvalidMetadata
    );
    assert_eq!(
        check(Domain::Named(DeclarationId(usize::MAX)), "1")
            .unwrap_err()
            .kind,
        ErrorKind::InvalidMetadata
    );
    let mut abstract_base = BASE;
    if let DeclarationKind::Entity {
        ref mut abstract_entity,
        ..
    } = abstract_base.kind
    {
        *abstract_entity = true;
    }
    schema.declarations = Box::leak(vec![abstract_base].into_boxed_slice());
    assert_eq!(
        decode::decode(&doc, &schema, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::AbstractEntity
    );
}

#[test]
fn schema_decode_alias_cycles_external_references_and_populations() {
    let alias = Declaration {
        name: "COUNT",
        schema: 0,
        span: SPAN,
        kind: DeclarationKind::Type {
            domain: Domain::Named(DeclarationId(1)),
            where_rules: &[],
        },
    };
    let attrs =
        Box::leak(vec![attr("amount", Domain::Named(DeclarationId(1)), false)].into_boxed_slice());
    let schema = SchemaSet {
        declarations: Box::leak(vec![entity("BASE", &[], attrs), alias].into_boxed_slice()),
        ..SCHEMAS
    };
    assert_eq!(
        decode::decode(
            &parse("#1=BASE(1);"),
            &schema,
            "test",
            Limits {
                max_depth: usize::MAX,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::ResourceLimit
    );
    let external = text("#1=BASE(1,#2);").replace(
        "DATA;",
        "REFERENCE;#2=<https://example.invalid/item>;ENDSEC;DATA;",
    );
    // URI resources use angle brackets without string delimiters.
    let external = external.replace(
        "<'https://example.invalid/item'>",
        "<https://example.invalid/item>",
    );
    let doc = tessstep_model::parse(external.as_bytes(), ParseLimits::default()).unwrap();
    assert_eq!(
        decode::decode(&doc, &SCHEMAS, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    let population = text("").replace("DATA;", "DATA('population',('TEST'));");
    let doc = tessstep_model::parse(population.as_bytes(), ParseLimits::default()).unwrap();
    assert_eq!(
        decode::decode(&doc, &SCHEMAS, "test", Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
}

#[test]
fn schema_decode_fuzz_smoke() {
    let seed = text("#1=BASE(1,#2);#2=CHILD(2,#1,(3,$,4));");
    let limits = Limits {
        max_work: 10_000,
        max_depth: 32,
    };
    let mut accepted = 0;
    for position in 0..seed.len() {
        for byte in [b'0', b'\'', b'$', b'*', b'(', b')', b';', b'X'] {
            let mut bytes = seed.as_bytes().to_vec();
            bytes[position] = byte;
            if let Ok(doc) = tessstep_model::parse(bytes.as_slice(), ParseLimits::default()) {
                accepted += 1;
                let a = decode::decode(&doc, &SCHEMAS, "test", limits);
                let b = decode::decode(&doc, &SCHEMAS, "test", limits);
                match (a, b) {
                    (Ok(a), Ok(b)) => assert_eq!(a.entities().len(), b.entities().len()),
                    (Err(a), Err(b)) => {
                        assert_eq!(a, b);
                        if let Some(span) = a.source {
                            assert!(span.end.offset as usize <= bytes.len());
                        }
                    }
                    _ => panic!("nondeterministic decoding"),
                }
            }
        }
    }
    assert!(accepted > 100);
}
