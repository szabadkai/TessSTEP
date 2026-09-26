# Benchmarks

`cargo bench -p tessstep-part21 --bench parser` runs an optimized, dependency-free
harness over a generated 957,938-byte / 20,000-entity input. Generation is outside timing.
One warmup precedes a two-second sample for each phase. Throughput includes allocation,
value decoding and dropping each token/event. Assertions verify fixture parsing.

Lexing and parsing are measured separately. This is an initial reproducible framework,
not a statistical performance guarantee or a large-assembly memory benchmark. Record
CPU/platform/toolchain, byte and entity counts, iterations, elapsed time and throughput
when comparing runs. Avoid comparing uncalibrated machines. Add schema, semantic,
geometry, trim and tessellation benchmarks as those implementations become available.

The initial local results are in `docs/VALIDATION.md`.

`cargo bench -p tessstep-express --bench compiler` measures whole-source EXPRESS lexing,
AST parsing and full basic compilation separately. It builds an original 233,946-byte
schema with 5,000 entities and one type before timing. Each phase has one warmup and a
two-second sample; allocation and destruction are included. Assertions check declaration
counts and compilation diagnostics. This measures local frontend throughput, not AP-scale
schema compatibility or code generation.

`cargo bench -p tessstep-model --bench decoder` measures the initial structural decoder
on 20,000 simple entities with cyclic references and supplied reflection metadata.
Physical parsing/fixture creation are outside timing. One warmup precedes a two-second
sample; borrowed view/index allocation and destruction are included. This is a structural
throughput observation, not AP conformance or a memory benchmark.

`cargo bench -p tessstep-tessellate --bench boundaries` measures structural validation /
normalization, UV seam reconstruction, and shared-edge sampling with face UV mapping.
It uses a constructed cylinder strip, one warmup and one-second samples. Raw cloning
is included in validation timing; geometry construction is outside timing. This is a
local throughput observation, not a face triangulation or whole-model benchmark.

`cargo bench -p tessstep-import --bench tessellated` imports a generated closed
tessellated box: a 1.5 MB document with 60,002 shared points and 120,000 strip
triangles over six faces. Parsing and generation are outside timing. One warmup
checks closure, triangle count and volume, then imports repeat for two seconds.
Timing includes selected-root decoding, index/normal validation, mesh construction
and owned-mesh validation. This is a local throughput observation, not a memory
benchmark or a comparison with B-rep tessellation.
