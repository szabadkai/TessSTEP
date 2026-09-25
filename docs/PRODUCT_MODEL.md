# Products and representations

Status: design contract for Milestone 5; not implemented in this delivery.

Semantic adapters must follow product definitions and representation relationships, including contexts, maps and mapped items. They must not discover geometry by scanning ADVANCED_FACE records. Products, shapes, representations and instances remain distinct. Normalize explicitly interpreted units into documented units; do not assume millimeters. Preserve reusable geometry and per-instance transforms. Generic model storage currently contains none of these semantics.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
