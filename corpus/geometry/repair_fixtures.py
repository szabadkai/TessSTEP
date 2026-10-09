#!/usr/bin/env python3
"""Generate the original topology repair fixtures (brep-repair-*.step).

Each fixture is an authored solid with one topology defect of the kind the opt-in
repair stage (Milestone 22.5) handles or must reject: duplicated vertices and edges,
a broken wire, a face or a whole shell written inside out, a disconnected (unsewn)
closed shell, and gaps beyond the sewing tolerance. Strict import rejects every one
of them. The root is always #1000 and lengths are millimetres; the boxes are
10 x 20 x 30 mm and the cylinder r=10 h=20 mm. Run from the repository root:
    python3 corpus/geometry/repair_fixtures.py
The output is deterministic; check_geometry.py regenerates the fixtures into a
temporary directory (``--output DIR``) and requires identical bytes.
"""
import argparse
from pathlib import Path

import brep_fixtures
from brep_fixtures import Step, num

HERE = Path(__file__).resolve().parent
LO, HI = (0, 0, 0), (10, 20, 30)
# Each face: corner indices counterclockwise seen from outside, normal, reference
# (as in brep_fixtures.box).
RINGS = [
    ((0, 2, 3, 1), (0, 0, -1), (0, 1, 0)), ((4, 5, 7, 6), (0, 0, 1), (1, 0, 0)),
    ((0, 1, 5, 4), (0, -1, 0), (1, 0, 0)), ((2, 6, 7, 3), (0, 1, 0), (0, 0, 1)),
    ((0, 4, 6, 2), (-1, 0, 0), (0, 0, 1)), ((1, 3, 7, 5), (1, 0, 0), (0, 1, 0)),
]
TOP = 1


def corner(c):
    return tuple(HI[k] if (c >> k) & 1 else LO[k] for k in range(3))


def patched_box(s, vertex_key=lambda face, c: c, edge_key=lambda face, a, b: None,
                offset=lambda key: (0, 0, 0), endpoint=lambda a, b, key: key, inside_out=(),
                reversed_loops=(), wrong_sense=()):
    """A 10 x 20 x 30 mm box whose face `f` uses, for corner `c`, the vertex keyed
    ``vertex_key(f, c)`` at the corner moved by ``offset(key)``, and for its edge from
    corner `a` to `b` the edge keyed by both vertex keys and ``edge_key(f, a, b)``. The
    edge's end at corner `a` is the vertex keyed ``endpoint(a, b, key)``. Faces in
    `inside_out` are written self-consistently inward: SAME_SENSE .F. and the loop
    reversed. Faces in `reversed_loops` have their loop reversed but keep SAME_SENSE
    .T., and faces in `wrong_sense` write SAME_SENSE .F. with their outward loop.
    Returns the faces."""
    vertices, edges = {}, {}

    def ensure(key, c):
        if key not in vertices:
            p = tuple(x + d for x, d in zip(corner(c), offset(key)))
            vertices[key] = (p, s.vertex(p))
        return key

    def edge(face, a, b):
        ka, kb = ensure(vertex_key(face, a), a), ensure(vertex_key(face, b), b)
        extra = edge_key(face, a, b)
        if (kb, ka, extra) in edges:
            return edges[(kb, ka, extra)], False
        if (ka, kb, extra) not in edges:
            sa, sb = ensure(endpoint(a, b, ka), a), ensure(endpoint(b, a, kb), b)
            pa, pb = vertices[sa][0], vertices[sb][0]
            line = s.line(pa, [pb[k] - pa[k] for k in range(3)])
            edges[(ka, kb, extra)] = s.edge(vertices[sa][1], vertices[sb][1], line)
        return edges[(ka, kb, extra)], True

    faces = []
    for f, (ring, normal, ref) in enumerate(RINGS):
        uses = [edge(f, ring[i], ring[(i + 1) % 4]) for i in range(4)]
        plane = s.plane(vertices[vertex_key(f, ring[0])][0], normal, ref)
        if f in inside_out or f in reversed_loops:
            uses = [(e, not forward) for e, forward in reversed(uses)]
        same = f not in inside_out and f not in wrong_sense
        faces.append(s.face([s.bound(uses, outer=True)], plane, same=same))
    return faces


def unsewn_cylinder(s, radius=10, height=20):
    """Solid cylinder whose caps have their own vertices and circle edges, coincident
    with the side face's; the bottom cap's circle runs the other way round."""
    def circle_edge(z, axis):
        v = s.vertex((radius, 0, z))
        return s.edge(v, v, s.circle((0, 0, z), axis, (1, 0, 0), radius))
    bottom, top = circle_edge(0, (0, 0, 1)), circle_edge(height, (0, 0, 1))
    surface = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},{num(radius)})")
    side = s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], surface)
    # The bottom cap's own circle runs clockwise about +z (its axis is -z), so the cap,
    # facing -z, traverses it forward; the top cap's circle runs as the side's does.
    cap_bottom = circle_edge(0, (0, 0, -1))
    cap_top = circle_edge(height, (0, 0, 1))
    return [
        side,
        s.face([s.bound([(cap_bottom, True)], outer=True)], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(cap_top, True)], outer=True)], s.plane((0, 0, height), (0, 0, 1))),
    ]


def fixtures():
    # Corner 7 (10, 20, 30) written twice: the top face uses a copy 5e-5 mm away along
    # x (within the default sewing tolerance, beyond the model tolerance), so the top
    # face's two edges at that corner are free; and the front face has its own copy of
    # the bottom front edge. Sewing merges one vertex and three edges.
    s = Step("Original box 10 x 20 x 30 mm with a duplicated corner vertex and three duplicated edges")
    s.write("brep-repair-duplicated.step", patched_box(
        s,
        vertex_key=lambda f, c: ("copy", c) if (f, c) == (TOP, 7) else c,
        edge_key=lambda f, a, b: "copy" if (f, {a, b}) == (2, {0, 1}) else None,
        offset=lambda key: (5e-5, 0, 0) if key == ("copy", 7) else (0, 0, 0)))

    # The edge between the top and right faces ends at a coincident copy of corner 7,
    # so both faces' loops break there: a broken wire that one vertex merge repairs.
    s = Step("Original box 10 x 20 x 30 mm whose loops break at a duplicated corner vertex")
    s.write("brep-repair-broken-wire.step", patched_box(
        s, endpoint=lambda a, b, key: ("copy", 7) if (a, b) == (7, 5) else key))

    s = Step("Original box 10 x 20 x 30 mm whose top face is written inside out")
    s.write("brep-repair-flipped-face.step", patched_box(s, inside_out={TOP}))

    # The top face's loop runs clockwise about its outward normal: the face would lie
    # outside its boundary. Its uses also disagree with the neighbouring faces.
    s = Step("Original box 10 x 20 x 30 mm whose top face loop runs against its surface")
    s.write("brep-repair-reversed-loop.step", patched_box(s, reversed_loops={TOP}))

    # The top face writes SAME_SENSE .F. with a loop for its outward normal: its uses
    # agree with the neighbouring faces, but the loop runs against the stated normal.
    s = Step("Original box 10 x 20 x 30 mm whose top face has a wrong SAME_SENSE")
    s.write("brep-repair-wrong-sense.step", patched_box(s, wrong_sense={TOP}))

    s = Step("Original box 10 x 20 x 30 mm with every face written inside out")
    s.write("brep-repair-inside-out.step", patched_box(s, inside_out=set(range(6))))

    # Every face has its own vertices and edges, coincident with its neighbours'.
    s = Step("Original disconnected closed shell: box 10 x 20 x 30 mm whose faces share no edges or vertices")
    s.write("brep-repair-unsewn.step", patched_box(s, vertex_key=lambda f, c: (f, c)))

    s = Step("Original disconnected closed shell: cylinder r=10 h=20 mm whose caps share no edges or vertices with its side")
    s.write("brep-repair-unsewn-cylinder.step", unsewn_cylinder(s))

    # Every face moved 0.01 mm out along its normal: faces share nothing and their
    # edges lie at least 0.01 mm apart, beyond the sewing tolerance.
    s = Step("Original disconnected closed shell beyond the sewing tolerance: box faces 0.01 mm apart")
    s.write("brep-repair-unsewn-gap.step", patched_box(
        s, vertex_key=lambda f, c: (f, c),
        offset=lambda key: tuple(0.01 * n for n in RINGS[key[0]][1])))

    # Corner 7 written twice for the top face, 0.05 mm away along x: a gap beyond the
    # sewing tolerance in an otherwise connected shell.
    s = Step("Original box 10 x 20 x 30 mm with a 0.05 mm gap at one corner")
    s.write("brep-repair-gap.step", patched_box(
        s,
        vertex_key=lambda f, c: ("copy", c) if (f, c) == (TOP, 7) else c,
        offset=lambda key: (0.05, 0, 0) if key == ("copy", 7) else (0, 0, 0)))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=HERE, help="Directory for the generated fixtures")
    brep_fixtures.OUTPUT = parser.parse_args().output
    fixtures()
