# Corpus

`manifest.json` records source, license, expected syntax class and SHA-256 for every
committed fixture. All fixtures are original synthetic Part 21 or EXPRESS frontend cases. A syntactically
valid file may intentionally contain missing references. No fixture claims to represent
a valid AP242 model. `scripts/check_corpus.py` verifies provenance coverage and hashes.

Keep confidential/local exports in ignored `private/`. The optional external compatibility corpus is described in
`docs/corpus-testing.md`; physical acceptance does not establish CAD correctness. Any future third-party sample must have a recorded legal source and license.

`express/valid` contains original schema declarations and dependency/opaque-body fixtures.
`imports.exp` needs `base.exp` supplied alongside it. `express/invalid/semantic.exp` is
syntactically valid but fails basic schema checks; the other invalid files fail parsing.
`python3 scripts/check_express.py` verifies these separate stages through the compiler CLI.
No fixture asserts expression semantics or schema validity for physical STEP instances.

`scripts/check_metamorphic.py` generates a separate original corpus of equivalent
graphs and adversarial boundary cases. Its deterministic recipe is the source;
temporary inputs are not added to this manifest. Reports retain each content
hash and expected outcome under `reports/generated/`. See
[`docs/corpus-testing.md`](../docs/corpus-testing.md) for scope and report verdicts.
