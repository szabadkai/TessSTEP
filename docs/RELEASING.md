# CI and releases

TessSTEP is MIT licensed. Its public GitHub repository uses standard hosted
runners, which GitHub provides free for public repositories. This depends on
repository visibility and runner type, not the license name. No paid larger
runners or external CI services are configured. Build artifacts expire after
7 days and corpus reports after 14 days.

## Pull requests and main

The `CI` workflow builds, lints, tests and packages on Linux x86-64, Windows
x86-64 and macOS Apple Silicon. It also checks Rust 1.85, dependency licenses
and advisories, fixture provenance, architecture boundaries, generated
conformance documentation, and CLI output. Each OS uploads a smoke-tested
binary archive and SHA-256 checksum. Packaging runs copied executables
from a temporary directory against included authored fixtures.

The external corpus job downloads the immutable source revisions listed in
`corpus/sources.json`. The NIST archive has an additional SHA-256 pin. Every
unique input must match the existing `corpus/baseline.json` inventory before
testing begins. Downloads are read as data; upstream code is never executed.
Third-party inputs are not copied into this repository or release assets, and
their upstream licenses are not replaced by TessSTEP's MIT license.

The Actions run summary shows outcome counts and baseline changes. Download
`corpus-report` from that run to inspect `index.html`, `latest.json`,
`summary.md` and `junit.xml`. JUnit represents compatibility regressions,
not full STEP conformance. A known rejection can be a passing regression test.
Acquisition failures, missing inputs, crashes and timeouts fail CI; reporting
and artifact upload run even after a test failure. Historical runs and their
summaries remain visible in Actions; downloadable reports expire after 14 days.

The nightly workflow performs bounded instrumented fuzzing, release-mode tests
and benchmarks. These jobs also support manual dispatch. The corpus job runs
on every PR and main push and is required again by the release workflow.

## Create a release

1. Update the workspace version, exact internal dependency versions in
   `Cargo.toml`, lockfiles and release documentation in a reviewed change.
2. Merge to `main` and check CI is green.
3. Create and push a tag matching the version, for example:

   ```sh
   git tag -a v0.1.0 -m "TessSTEP 0.1.0"
   git push origin v0.1.0
   ```

4. The `Release` workflow checks the tag/version agreement, reruns CI including
   the full corpus, runs release-mode tests, and builds packages for Linux,
   Windows, macOS Apple Silicon and macOS Intel. It creates a **draft GitHub
   release** with archives and SHA-256 checksum files. Review the notes and
   packages, then publish the draft. Manual dispatch must select an existing
   matching tag; dispatching on a branch fails safely.

Release assets contain `stepdump`, `expressc`, the shared ABI 1 library, C/C++
headers, relocatable CMake package, documentation, MIT license, and authored examples. GitHub also provides the tagged source archive for Rust
library consumers. This workflow does not publish crates to crates.io.

Packaging installs and relocates the C/C++ SDK, compiles/runs standalone C11/C++17
consumers in both Debug and Release, and checks the exact exported ABI symbol set.
No Rust toolchain is used by those consumers. The existing Linux/Windows/macOS release
matrix runs this gate before creating archives; Linux/macOS CI also instruments native
consumers with ASan/UBSan. Shared linkage is the only packaged configuration. Schema
operations, geometry and mesh views are not exposed by the public ABI yet. Local
verification does not substitute for the cross-platform CI results in `C_API.md`.

Local equivalents:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/fetch_corpus.py --output target/step-corpus
python3 scripts/corpus.py --corpus target/step-corpus --check
python3 scripts/corpus_summary.py
cargo build --workspace --release --locked
python3 scripts/package_release.py --target "$(rustc -vV | sed -n 's/^host: //p')"
```

Python 3.11+ is needed for release packaging; CI uses Python 3.12.
