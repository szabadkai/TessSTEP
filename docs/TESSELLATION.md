# Tessellation

Status: design contract for Milestones 13–17; not implemented in this delivery.

Tessellation must consume normalized B-reps and explicit chord/angular tolerances. Sample each shared topological edge once, then constrain incident face boundaries to those samples. Refine interior triangles based on measured geometric error and propagate failures when limits cannot be met. Verify manifoldness, orientations and shared boundaries before claiming a watertight solid. Exporters belong in tessstep-io; this delivery produces no meshes.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
