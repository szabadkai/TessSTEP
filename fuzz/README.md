# Fuzzing

Development-only workspace, excluded from production builds. `libfuzzer-sys` supplies
the engine; TessSTEP supplies the parsers and shared finite budgets.

Stable deterministic regression smoke:

```sh
cargo test -p tessstep-model --test fuzz_smoke -- --nocapture
cargo test -p tessstep-express --test fuzz_smoke -- --nocapture
cargo test -p tessstep-codegen --test generated codegen_fuzz_smoke -- --nocapture
cargo check --manifest-path fuzz/Cargo.toml --locked
```

Coverage-guided fuzzing requires rustup, nightly Rust, and cargo-fuzz:

```sh
cargo install cargo-fuzz --locked
cargo +nightly fuzz run part21_parser corpus/part21/valid corpus/part21/invalid -- -max_total_time=120 -max_len=65536
cargo +nightly fuzz run part21_lexer corpus/part21/valid corpus/part21/invalid -- -max_total_time=120 -max_len=65536
cargo +nightly fuzz run entity_database corpus/part21/valid corpus/part21/invalid -- -max_total_time=120 -max_len=65536
cargo +nightly fuzz run express_frontend corpus/express/valid corpus/express/invalid -- -max_total_time=120 -max_len=8192
cargo +nightly fuzz run schema_codegen corpus/express/valid corpus/express/invalid -- -max_total_time=120 -max_len=8192
```

Copy seeds to a scratch corpus directory when running extended sessions: libFuzzer may
write new seeds into its first corpus directory. Do not commit unreviewed generated files.
Artifacts are ignored under `fuzz/artifacts`; convert minimized failures to regression tests.
Directly running a stable-built driver executes inputs but has no sanitizer/coverage
instrumentation. Such runs must be reported as driver smoke only.

The schema-codegen target compiles bounded EXPRESS input and checks deterministic
generation/error handling and output size; it does not invoke rustc per fuzz case.

The EXPRESS target exercises lexing and basic compilation under small finite budgets,
checks deterministic results and valid diagnostic spans, and skips non-UTF-8 bytes
because the library API accepts `&str`. NURBS, trim and triangulation targets will be added
when those crates exist.
They are not represented by fake targets now.

The [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/tutorial.html)
describes the cargo-fuzz driver and crash-minimization workflow.

`schema_decoder` uses generated metadata from `corpus/schema/sample.exp` and the shared
bounded harness in `support/decoder.rs`. Seed with `corpus/schema`; stable mutations run
in the model crate's `decoder_fuzz` test. Regenerate `support/decoder_schema.rs` with
`cargo run -p expressc -- --rust corpus/schema/sample.exp` when the schema changes; the
codegen test checks this exact output. Nightly CI instruments the decoder target.
