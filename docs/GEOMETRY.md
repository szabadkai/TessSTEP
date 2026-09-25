# Exact geometry

Status: design contract for Milestones 6–10; not implemented in this delivery.

Mathematical primitives will be immutable values in STEP-independent crates. Points and vectors should distinguish model/local/parameter spaces; lengths and angles must be typed. Curve and surface enums expose evaluation, derivatives and domains without raw entity access. Model and tessellation tolerances are distinct objects. Add an independently tested mathematical primitive before adding its schema adapter. No evaluator exists in this delivery.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
