# Schema bindings and reflection

Status: design contract for Milestones 3–4; not implemented in this delivery.

Generated typed views will coexist with generic instances. Metadata must expose inheritance, attribute names and types, aggregates, SELECT alternatives, schema identity and validation constraints. A generated-code compile test must prove determinism and usability. Attribute count/type/cardinality and SELECT checks belong here, never in the physical parser. Milestone 2 now supplies EXPRESS AST/IR and basic declaration validation. The conformance manifest tracks physical and frontend features separately; runtime instance validation and generated entity coverage remain future work.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
