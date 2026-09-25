# NURBS

Status: design contract for Milestones 9–10; not implemented in this delivery.

Implement iterative basis/span algorithms and homogeneous rational evaluation directly. Validate degrees, multiplicities, control counts, finite weights and domains before allocating or evaluating. Tests must include repeated knots, endpoints, periodic domains, tensor products, derivatives and invariance under knot insertion. Cite the actual published algorithm when implementing it; no algorithm or numerical accuracy is claimed by this design note.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
