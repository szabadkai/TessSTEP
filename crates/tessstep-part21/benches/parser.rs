use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use tessstep_part21::{Event, Lexer, ParseLimits, Parser, TokenKind};
fn main() {
    let mut input = String::from(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION(('bench'),'2;1');FILE_NAME('','','a','b','','','');FILE_SCHEMA(('TEST'));ENDSEC;DATA;",
    );
    input = input.replace("'a','b'", "('a'),('b')");
    for id in 1..=20_000 {
        input.push_str(&format!(
            "#{id}=SAMPLE('sample',1.25E2,($,*,.T.,#{}));\n",
            id % 20_000 + 1
        ));
    }
    input.push_str("ENDSEC;END-ISO-10303-21;");
    bench("lexer", input.len(), || {
        let mut lexer = Lexer::new(input.as_bytes(), ParseLimits::default());
        let mut count = 0;
        while lexer.next_token().expect("fixture lexes").kind != TokenKind::Eof {
            count += 1;
        }
        black_box(count);
    });
    bench("parser-events", input.len(), || {
        let mut count = 0;
        for event in Parser::new(input.as_bytes(), ParseLimits::default()).unwrap() {
            if matches!(event.unwrap(), Event::Entity(_)) {
                count += 1;
            }
        }
        assert_eq!(count, 20_000);
        black_box(count);
    });
}
fn bench(name: &str, bytes: usize, mut run: impl FnMut()) {
    run();
    let start = Instant::now();
    let mut iterations = 0;
    while start.elapsed() < Duration::from_secs(2) {
        run();
        iterations += 1;
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "{name}: {iterations} iterations, {:.2} ms/iteration, {:.2} MiB/s, {bytes} bytes/iteration",
        elapsed * 1000.0 / f64::from(iterations),
        bytes as f64 * f64::from(iterations) / elapsed / 1048576.0
    );
}
