# UV trimming

Status: design contract for Milestone 12; not implemented in this delivery.

Reconstruct trim loops in surface parameter space, retaining an outer loop, holes, orientation and optional pcurves. Handle periodic unwrapping and seam uses explicitly. Treat singularities, collapsed edges and missing pcurves as diagnosed conditions. Arbitrary 3D projection must not replace trim reconstruction. Each future algorithm needs constructed geometry tests independent of STEP files.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and dependency boundaries. Future
implementation must update conformance evidence and the test suite before changing this status.
