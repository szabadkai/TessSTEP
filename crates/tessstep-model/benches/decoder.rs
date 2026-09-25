use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use tessstep_model::{
    decode::{Limits, decode},
    parse,
};
use tessstep_schema::*;
const SPAN: SourceSpan = SourceSpan {
    source: 0,
    start: 0,
    end: 0,
    line: 1,
    column: 1,
};
static SCHEMAS: SchemaSet = SchemaSet {
    sources: &["benchmark.exp"],
    schemas: &[Schema {
        name: "BENCH",
        span: SPAN,
        dependencies: &[],
        exports: &[],
        symbols: &[Symbol {
            name: "NODE",
            declaration: DeclarationId(0),
        }],
    }],
    declarations: &[Declaration {
        name: "NODE",
        schema: 0,
        span: SPAN,
        kind: DeclarationKind::Entity {
            abstract_entity: false,
            supertype_constraint: None,
            supertypes: &[],
            unique: &[],
            where_rules: &[],
            attributes: &[
                Attribute {
                    name: "VALUE",
                    span: SPAN,
                    domain: Domain::Builtin {
                        kind: Builtin::Integer,
                        width: None,
                        fixed: false,
                    },
                    optional: false,
                    kind: AttributeKind::Explicit,
                },
                Attribute {
                    name: "NEXT",
                    span: SPAN,
                    domain: Domain::Named(DeclarationId(0)),
                    optional: false,
                    kind: AttributeKind::Explicit,
                },
            ],
        },
    }],
    unsupported: &[],
};
fn main() {
    let count = 20_000;
    let mut input = String::from(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION(('bench'),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('BENCH'));ENDSEC;DATA;",
    );
    for id in 1..=count {
        input.push_str(&format!("#{id}=NODE({id},#{});", id % count + 1));
    }
    input.push_str("ENDSEC;END-ISO-10303-21;");
    let document = parse(input.as_bytes(), Default::default()).unwrap();
    let once = || {
        let decoded = decode(
            black_box(&document),
            black_box(&SCHEMAS),
            "BENCH",
            Limits::default(),
        )
        .unwrap();
        assert_eq!(decoded.entities().len(), count);
        black_box(decoded);
    };
    once();
    let start = Instant::now();
    let mut iterations = 0;
    while start.elapsed() < Duration::from_secs(2) {
        once();
        iterations += 1;
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "decoder: {} bytes, {count} entities, {iterations} iterations, {:.3} s, {:.3} ms/input, {:.0} entities/s",
        input.len(),
        elapsed,
        elapsed * 1000.0 / iterations as f64,
        count as f64 * iterations as f64 / elapsed
    );
}
