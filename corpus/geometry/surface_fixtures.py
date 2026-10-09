#!/usr/bin/env python3
"""Generate the original surface-model import fixtures (surface-*.step).

Every fixture is an authored SHELL_BASED_SURFACE_MODEL set, not an exporter file;
lengths are millimetres, the roots start at #1000 and one
MANIFOLD_SURFACE_SHAPE_REPRESENTATION in a millimetre context holds them all.

surface-open-patches.step: #1000 is one OPEN_SHELL of three faces, a half-cylinder
trough (r=10, h=20) between a planar flange and a bilinear B-spline flange (each
5 x 20); area 200 pi + 200 mm2. #1001 uses the same shell through an
ORIENTED_OPEN_SHELL(.F.), so every face is reversed.

surface-closed-shells.step: #1000 holds a CLOSED_SHELL cylinder (r=10, h=20, outward;
volume 2000 pi mm3) and an ORIENTED_CLOSED_SHELL(.F.) of a 10 mm box beside it
(inward; signed volume -1000 mm3), both shells, not solids. #1001 declares the open
trough faces as a CLOSED_SHELL (rejected: open); #1002 is a VERTEX_SHELL (outside
the profile).

Run from the repository root: python3 corpus/geometry/surface_fixtures.py
The output is deterministic; check_geometry.py regenerates the fixtures into a
temporary directory (``--output DIR``) and requires identical bytes.
"""
import argparse
from pathlib import Path

from brep_fixtures import Step, box, cylinder, num

HERE = Path(__file__).resolve().parent
OUTPUT = HERE


def write(s, name, roots):
    """Roots are SHELL_BASED_SURFACE_MODEL bodies from #1000; context from #2000."""
    assert s.next < 1000
    lines = list(s.lines)
    ids = []
    for k, body in enumerate(roots):
        ids.append(1000 + k)
        lines.append(f"#{1000 + k}={body};")
    lines += [
        "#2001=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));",
        "#2002=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));",
        "#2006=(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT());",
        "#2007=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-5),#2001,'DISTANCE_ACCURACY_VALUE','');",
        "#2008=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#2007))"
        "GLOBAL_UNIT_ASSIGNED_CONTEXT((#2001,#2002,#2006))REPRESENTATION_CONTEXT('',''));",
        f"#2009=MANIFOLD_SURFACE_SHAPE_REPRESENTATION('',({','.join(f'#{i}' for i in ids)}),#2008);",
    ]
    header = [
        "ISO-10303-21;",
        "HEADER;",
        f"FILE_DESCRIPTION(('{s.description}'),'2;1');",
        f"FILE_NAME('{name}','2026-10-02',('TessSTEP'),('TessSTEP'),'authored fixture','','');",
        "FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));",
        "ENDSEC;",
        "DATA;",
    ]
    text = "\n".join(header + lines + ["ENDSEC;", "END-ISO-10303-21;", ""])
    (OUTPUT / name).write_text(text, newline="\n")


def trough(s):
    """Half-cylinder r=10 over y >= 0 from z=0 to 20 (outward normal), with flanges
    in y=0 from x=10 to 15 (a plane) and from x=-15 to -10 (a bilinear B-spline),
    both facing +y, consistently oriented with the cylinder. Returns faces."""
    a0, a1 = s.vertex((15, 0, 0)), s.vertex((15, 0, 20))
    b0, b1 = s.vertex((10, 0, 0)), s.vertex((10, 0, 20))
    c0, c1 = s.vertex((-10, 0, 0)), s.vertex((-10, 0, 20))
    d0, d1 = s.vertex((-15, 0, 0)), s.vertex((-15, 0, 20))

    def rail(start, end, origin):
        return s.edge(start, end, s.line(origin, (0, 0, 1)))

    def run(start, end, origin, direction):
        return s.edge(start, end, s.line(origin, direction))

    ea, eb, ec, ed = rail(a0, a1, (15, 0, 0)), rail(b0, b1, (10, 0, 0)), rail(c0, c1, (-10, 0, 0)), rail(d0, d1, (-15, 0, 0))
    ab0, ab1 = run(a0, b0, (15, 0, 0), (-1, 0, 0)), run(a1, b1, (15, 0, 20), (-1, 0, 0))
    cd0, cd1 = run(c0, d0, (-10, 0, 0), (-1, 0, 0)), run(c1, d1, (-10, 0, 20), (-1, 0, 0))
    arc0 = s.edge(b0, c0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    arc1 = s.edge(b1, c1, s.circle((0, 0, 20), (0, 0, 1), (1, 0, 0), 10))
    side = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},{num(10)})")
    rows = [f"(#{s.point((-10, 0, 0))},#{s.point((-10, 0, 20))})", f"(#{s.point((-15, 0, 0))},#{s.point((-15, 0, 20))})"]
    patch = s.add(f"B_SPLINE_SURFACE_WITH_KNOTS('',1,1,({','.join(rows)}),.UNSPECIFIED.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.)")
    return [
        s.face([s.bound([(ab0, True), (eb, True), (ab1, False), (ea, False)], outer=True)], s.plane((10, 0, 0), (0, 1, 0))),
        s.face([s.bound([(arc0, True), (ec, True), (arc1, False), (eb, False)], outer=True)], side),
        s.face([s.bound([(cd0, True), (ed, True), (cd1, False), (ec, False)], outer=True)], patch),
    ]


def fixtures():
    s = Step("Original open surface model: half-cylinder trough between a planar and a B-spline flange")
    shell = s.add(f"OPEN_SHELL('',({','.join(f'#{f}' for f in trough(s))}))")
    reversed_shell = s.add(f"ORIENTED_OPEN_SHELL('',*,#{shell},.F.)")
    write(s, "surface-open-patches.step", [
        f"SHELL_BASED_SURFACE_MODEL('',(#{shell}))",
        f"SHELL_BASED_SURFACE_MODEL('',(#{reversed_shell}))",
    ])

    s = Step("Original closed surface model: a cylinder shell and an inward box shell; two rejected models")
    cylinder_shell = s.add(f"CLOSED_SHELL('',({','.join(f'#{f}' for f in cylinder(s, 10, 0, 20))}))")
    box_shell = s.add(f"CLOSED_SHELL('',({','.join(f'#{f}' for f in box(s, (30, 0, 0), (40, 10, 10)))}))")
    inward = s.add(f"ORIENTED_CLOSED_SHELL('',*,#{box_shell},.F.)")
    declared_closed = s.add(f"CLOSED_SHELL('',({','.join(f'#{f}' for f in trough(s))}))")
    vertex_shell = s.add(f"VERTEX_SHELL('',#{s.add(f'VERTEX_LOOP({chr(39)}{chr(39)},#{s.vertex((0, 0, 50))})')})")
    write(s, "surface-closed-shells.step", [
        f"SHELL_BASED_SURFACE_MODEL('',(#{cylinder_shell},#{inward}))",
        f"SHELL_BASED_SURFACE_MODEL('',(#{declared_closed}))",
        f"SHELL_BASED_SURFACE_MODEL('',(#{vertex_shell}))",
    ])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=HERE, help="Directory for the generated fixtures")
    OUTPUT = parser.parse_args().output
    fixtures()
