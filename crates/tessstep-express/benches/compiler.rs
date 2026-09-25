use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use tessstep_express::{Limits, Source, compile, lex, parse};
fn main() {
    let mut input = String::from("SCHEMA bench; TYPE measure = REAL; END_TYPE;\n");
    for i in 0..5000 {
        input.push_str(&format!("ENTITY item_{i}; x,y,z : measure; END_ENTITY;\n"));
    }
    input.push_str("END_SCHEMA;");
    let limits = Limits::default();
    bench("express-lexer", input.len(), || {
        black_box(lex(0, &input, limits).unwrap());
    });
    bench("express-ast", input.len(), || {
        let ast = parse(0, &input, limits).unwrap();
        assert_eq!(ast[0].declarations.len(), 5001);
        black_box(ast);
    });
    bench("express-compile", input.len(), || {
        let c = compile(
            &[Source {
                name: "bench.exp",
                text: &input,
            }],
            limits,
        );
        assert!(c.diagnostics.is_empty());
        assert_eq!(c.ir.as_ref().unwrap().declarations.len(), 5001);
        black_box(c);
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
        "{name}: {iterations} iterations, {:.2} ms/input, {:.2} MiB/s, {bytes} bytes/input",
        elapsed * 1000.0 / f64::from(iterations),
        bytes as f64 * f64::from(iterations) / elapsed / 1048576.0
    );
}
