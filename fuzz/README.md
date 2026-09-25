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

The `product_adapter` target parses, structurally decodes with the original product
test schema and adapts bounded graphs twice to check determinism. Seed with
`corpus/product`. Generated metadata is shared with adapter consumer tests and
verified by `scripts/check_product.py`. It does not fuzz a complete AP schema.

`math_primitives` uses the same arbitrary IEEE-754 bit-pattern harness as the independent
math crate's stable smoke test. Each input consumes at most 144 bytes, with fixed-size
inversion, frame and coordinate operations. Run
`cargo fuzz run math_primitives -- -max_total_time=120 -max_len=144` for instrumentation.
Successful finite results and repeatable inverse failures are checked; this is not an
exact arithmetic oracle or an industrial numerical-hardening claim.

`analytic_curves` exercises analytic curve values through a fixed-size
3D harness: axes, positive lengths, arbitrary parameters and oriented spans. It shares
its 128-byte, bounded-work exercise with the stable curve smoke test. Run
`cargo fuzz run analytic_curves -- -max_total_time=120 -max_len=128` for instrumentation.
Successful outputs must be finite and repeatable; this is not an exact numerical oracle.

`nurbs_curves` and `surface_evaluators` share the new stable bounded spline/geometry
harnesses. Run each with `-max_len=128`; nightly CI configures 120 seconds per target.
Inputs cover malformed knots/degrees, arbitrary finite/non-finite coordinates, weights,
parameters, evaluation and refinement. Small logical limits bound allocations. Seeded
stable mutation tests supplement arbitrary-bit inputs; none is an exact arithmetic oracle.

`brep_pipeline` mutates original constructed topology fixtures, index references,
orientations, memberships and arbitrary-bit edge/pcurve intervals. Successful structural
validation feeds bounded trim reconstruction, shared-edge sampling and per-face UV
mapping, planar triangulation and adaptive mesh assembly with index/finite checks.
Six seeds include a capped cylinder and a NURBS bump. Inputs are limited to 96 bytes; the stable test runs 3,000 random mutations
plus 582 fixture prefixes. It has no STEP parser or external corpus dependency.
