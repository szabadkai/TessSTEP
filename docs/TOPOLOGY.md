# Topology and validity

Status: design contract for Milestone 11; not implemented in this delivery.

Introduce typed vertex, edge, coedge, wire, face, shell and solid handles. Preserve edge-use orientation and separate 3D curves from per-face pcurves. Build RawBrep, validate into ValidatedBrep, and normalize into immutable NormalizedBrep. Validation reports defects without modifying geometry; optional healing records each change and tolerance. The future tessellator consumes only normalized objects. Generic EntityId is not a topology handle.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
