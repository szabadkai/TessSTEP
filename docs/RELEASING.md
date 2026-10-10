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
from a temporary directory against included authored fixtures. The iOS job builds
the static library for devices and the Apple silicon simulator on macOS, runs the
`tessstep-capi` tests in a booted simulator, and uploads a consumer-tested
XCFramework archive (`build-ios`); see [C_API.md](C_API.md#ios).

The external corpus job downloads the immutable source revisions listed in
`corpus/sources.json`. The NIST archive has an additional SHA-256 pin. Every
unique input must match the existing `corpus/baseline.json` inventory before
testing begins. Downloads are read as data; upstream code is never executed.
Third-party inputs are not copied into this repository or release assets, and
their upstream licenses are not replaced by TessSTEP's MIT license.

The Actions run summary shows outcome counts and baseline changes. Download
`corpus-report` from that run to inspect `index.html`, `latest.json`,
`summary.md`, `cases.csv` and `junit.xml`. JUnit represents compatibility regressions,
not full STEP conformance. A known rejection can be a passing regression test.
Acquisition failures, missing inputs, crashes and timeouts fail CI; reporting
and artifact upload run even after a test failure. Historical runs and their
summaries remain visible in Actions; downloadable reports expire after 14 days.

Each stable OS job also uploads `test-reports-<os>` for 14 days, containing
per-test Rust evidence and authored schema, product and generated transformation
corpora. Format-2 reports distinguish expectations from baseline compatibility
and unreviewed observations, with stage/source coverage and failure details.
Physical parsing runs twice to detect nondeterminism. Product reports include
numerical relationship/count expectations, so those failures appear in JUnit.

The nightly workflow performs bounded instrumented fuzzing, release-mode tests
and benchmarks. The benchmark job also reconstructs the pinned corpus for the
session benchmark's largest inputs; a failed download only skips those inputs.
These jobs also support manual dispatch. The corpus job runs
on every PR and main push and is required again by the release workflow.

## Create a release

Record each user-visible change under `## Unreleased` in `CHANGELOG.md` as it
lands: that section becomes the release notes.

1. Check that CI is green on the `main` commit to release.
2. In the Actions tab, run **Prepare release** on `main` with the new version,
   for example `1.1.0`. It refuses a malformed version, one not above the current
   version, a major version change, an empty or missing Unreleased section, an
   existing tag, and a commit without a successful CI run. It sets the version in
   `Cargo.toml` (workspace and exact internal dependency versions), both lockfiles
   and `CMakeLists.txt`, renames `## Unreleased` to `## <version> — <date>`, commits
   that to `main` as the user who started it, pushes the commit and the
   `v<version>` tag atomically, and starts the `Release` workflow on the tag.
   Pushes made with the workflow token start no workflows, so this commit has no
   separate `main` CI run; the `Release` workflow runs CI on it instead.
3. The `Release` workflow checks the tag/version agreement and writes the notes
   from the version's CHANGELOG section, failing at once when there is none. It
   then reruns CI, including the full corpus and the iOS simulator gate, runs
   release-mode tests, and builds packages for Linux, Windows, macOS Apple
   Silicon and macOS Intel. It creates a **draft GitHub release** with those
   archives, the iOS XCFramework archive from its CI run, SHA-256 checksum files
   and the notes, whose relative links point at files at the tag and which name
   the run and compare with the previous release. Review the notes and packages,
   then publish the draft.
4. The next change starts a new `## Unreleased` section.

A major version also changes the CMake package's `SameMajorVersion` compatibility
floor and the `find_package` minima in the README, `C_API.md` and `tests/capi`, so
it is prepared by hand in a reviewed change (versions as above, plus the dated
CHANGELOG heading). Then tag it; a pushed tag starts the `Release` workflow:

```sh
git tag -a v2.0.0 -m "TessSTEP 2.0.0"
git push origin v2.0.0
```

Manual dispatch of `Release` must select an existing matching tag; dispatching on
a branch fails safely.

Desktop release assets contain `stepdump`, `expressc`, the shared and static ABI 1 libraries, C/C++
headers, relocatable CMake package, documentation, MIT license, and authored examples.
`tessstep-<version>-ios.zip` contains `TessSTEP.xcframework` (the static library for
iOS devices and the simulator, with the C/C++ headers), the ABI record, documentation
and license. GitHub also provides the tagged source archive for Rust
library consumers. This workflow does not publish crates to crates.io.

Packaging installs and relocates the C/C++ SDK, compiles/runs standalone C11/C++17
consumers in both Debug and Release, and checks the exported symbols against the frozen ABI record `abi/exports.txt`, which the archive also ships.
No Rust toolchain is used by those consumers. The existing Linux/Windows/macOS release
matrix runs this gate before creating archives; Linux/macOS CI also instruments native
consumers with ASan/UBSan. The package ships the shared library and a static archive
(`TessSTEP::TessSTEP_static`), and the gate also links and runs the consumers
statically in Release. General schema and B-rep construction operations remain unavailable. Mesh views,
scenes, appearance, discovery and selected faceted, planar and curved STEP-to-mesh conversion are exposed. Local
verification does not substitute for the cross-platform CI results in `C_API.md`.

Local equivalents:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/fetch_corpus.py --output target/step-corpus
python3 scripts/corpus.py --corpus target/step-corpus --check
python3 scripts/corpus_summary.py
cargo build --workspace --release --locked
python3 scripts/package_release.py --target "$(rustc -vV | sed -n 's/^host: //p')"
cargo build --release --locked -p tessstep-capi --target aarch64-apple-ios
cargo build --release --locked -p tessstep-capi --target aarch64-apple-ios-sim
TESSSTEP_IOS_SIMULATOR="iPad Pro 13-inch (M4)" python3 scripts/package_ios.py
python3 scripts/release.py notes --repository szabadkai/TessSTEP
```

Python 3.11+ is needed for release packaging; CI uses Python 3.12.
