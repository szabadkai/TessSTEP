# Schemas

No ISO schema is bundled. Place legally supplied EXPRESS files here for compiler
work; document their origin and redistribution permission before committing them.
Compile them with `cargo run -p expressc -- schema.exp dependencies.exp`. Dependencies
must be supplied explicitly. The original test schemas are in `corpus/express/`; they
are not ISO schema extracts. Physical parsing remains schema-independent.
