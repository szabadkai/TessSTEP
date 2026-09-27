# Schemas

No ISO schema is bundled. Place legally supplied EXPRESS files here for compiler
work; document their origin and redistribution permission before committing them.
Compile them with `cargo run -p expressc -- schema.exp dependencies.exp`. Dependencies
must be supplied explicitly. The original test schemas are in `corpus/express/`; they
are not ISO schema extracts. Physical parsing remains schema-independent.

The planar and faceted importers include original reduced structural profiles at
`corpus/geometry/planar.exp` and `corpus/geometry/faceted.exp`.
These are not ISO schemas or AP validators; see
[STEP_IMPORT.md](../docs/STEP_IMPORT.md) for selected-root scope.

The corpus schema stage compiles the AP242, AP214 and AP203 EXPRESS long forms that
stepcode distributes in its `data/` directory. They are fetched at build time by
`scripts/fetch_schemas.py` from the revision and SHA-256 values pinned in
`corpus/ap-schemas.json` and are not committed here: ISO holds the copyright of the
schemas, and stepcode redistributes them for implementation use. Compile them with
`scripts/build_ap_validator.py`; see [corpus-testing.md](../docs/corpus-testing.md#ap-schema-stage-milestone-21).
