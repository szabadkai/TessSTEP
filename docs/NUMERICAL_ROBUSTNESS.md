# Numerical policy

Status: design contract for Milestones 6 onward; not implemented in this delivery.

Physical numeric parsing currently rejects i64 overflow, non-finite f64 values and nonzero literals underflowing to zero. It preserves negative zero and accepts representable subnormals. Decimal-to-binary rounding is unavoidable within the chosen representation and does not assert geometric accuracy. Future algorithms must receive typed model/tessellation tolerances, validate finite inputs and distinguish machine precision from modeling tolerance. Robust predicate work requires independent tests and a documented algorithm.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
