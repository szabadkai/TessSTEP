#!/usr/bin/env python3
"""Generate the original curved B-rep import fixtures (brep-*.step).

Every fixture is an authored solid with a closed-form volume, not an exporter file.
The root is always #1000 and lengths are millimetres. Run from the repository root:
    python3 corpus/geometry/brep_fixtures.py
The output is deterministic; check_geometry.py regenerates the fixtures into a
temporary directory (``--output DIR``) and requires identical bytes.
"""
import argparse
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUTPUT = HERE


def num(x):
    if abs(x) < 5e-16:
        x = 0.0
    text = repr(float(x))
    return text if ("." in text or "e" in text) else text + "."


class Step:
    def __init__(self, description):
        self.description = description
        self.lines = []
        self.next = 1

    def add(self, body):
        index = self.next
        self.next += 1
        self.lines.append(f"#{index}={body};")
        return index

    def point(self, p):
        return self.add(f"CARTESIAN_POINT('',({','.join(num(c) for c in p)}))")

    def direction(self, d):
        return self.add(f"DIRECTION('',({','.join(num(c) for c in d)}))")

    def placement(self, origin, axis, ref):
        return self.add(f"AXIS2_PLACEMENT_3D('',#{self.point(origin)},#{self.direction(axis)},#{self.direction(ref)})")

    def vertex(self, p):
        return self.add(f"VERTEX_POINT('',#{self.point(p)})")

    def line(self, origin, direction):
        vector = self.add(f"VECTOR('',#{self.direction(direction)},1.)")
        return self.add(f"LINE('',#{self.point(origin)},#{vector})")

    def circle(self, center, axis, ref, radius):
        return self.add(f"CIRCLE('',#{self.placement(center, axis, ref)},{num(radius)})")

    def edge(self, start, end, curve, same=True):
        return self.add(f"EDGE_CURVE('',#{start},#{end},#{curve},{'.T.' if same else '.F.'})")

    def loop(self, uses):
        oriented = [self.add(f"ORIENTED_EDGE('',*,*,#{e},{'.T.' if f else '.F.'})") for e, f in uses]
        return self.add(f"EDGE_LOOP('',({','.join(f'#{o}' for o in oriented)}))")

    def bound(self, uses, outer=False, orientation=True):
        kind = "FACE_OUTER_BOUND" if outer else "FACE_BOUND"
        return self.add(f"{kind}('',#{self.loop(uses)},{'.T.' if orientation else '.F.'})")

    def face(self, bounds, surface, same=True):
        return self.add(f"ADVANCED_FACE('',({','.join(f'#{b}' for b in bounds)}),#{surface},{'.T.' if same else '.F.'})")

    def plane(self, origin, axis, ref=(1, 0, 0)):
        return self.add(f"PLANE('',#{self.placement(origin, axis, ref)})")

    def write(self, name, faces, root="MANIFOLD_SOLID_BREP", context=None):
        shell = self.add(f"CLOSED_SHELL('',({','.join(f'#{f}' for f in faces)}))")
        assert self.next < 1000
        self.lines.append(f"#1000={root}('',#{shell});")
        if context:
            self.lines.extend(context(self))
        header = [
            "ISO-10303-21;",
            "HEADER;",
            f"FILE_DESCRIPTION(('{self.description}'),'2;1');",
            f"FILE_NAME('{name}','2026-09-26',('TessSTEP'),('TessSTEP'),'authored fixture','','');",
            "FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));",
            "ENDSEC;",
            "DATA;",
        ]
        text = "\n".join(header + self.lines + ["ENDSEC;", "END-ISO-10303-21;", ""])
        (OUTPUT / name).write_text(text)


def context(length="(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))", degrees=False,
            uncertainty="1.E-5"):
    """Shape representation and context; ids from #2000 to stay clear of geometry."""
    def lines(_):
        out = [f"#2001={length};"]
        if degrees:
            out += [
                "#2002=(CONVERSION_BASED_UNIT('DEGREE',#2004)NAMED_UNIT(#2003)PLANE_ANGLE_UNIT());",
                "#2003=DIMENSIONAL_EXPONENTS(0.,0.,0.,0.,0.,0.,0.);",
                "#2004=PLANE_ANGLE_MEASURE_WITH_UNIT(PLANE_ANGLE_MEASURE(0.0174532925199433),#2005);",
                "#2005=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));",
            ]
        else:
            out.append("#2002=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));")
        out += [
            "#2006=(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT());",
            f"#2007=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE({uncertainty}),#2001,'DISTANCE_ACCURACY_VALUE','');",
            "#2008=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#2007))"
            "GLOBAL_UNIT_ASSIGNED_CONTEXT((#2001,#2002,#2006))REPRESENTATION_CONTEXT('',''));",
            "#2009=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#1000),#2008);",
        ]
        return out
    return lines


def cylinder(s, radius, z0, z1, caps=True, seam=False, cap_outer=True):
    """Solid cylinder along +z. Returns faces."""
    v0 = s.vertex((radius, 0, z0))
    v1 = s.vertex((radius, 0, z1))
    bottom = s.edge(v0, v0, s.circle((0, 0, z0), (0, 0, 1), (1, 0, 0), radius))
    top = s.edge(v1, v1, s.circle((0, 0, z1), (0, 0, 1), (1, 0, 0), radius))
    surface = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},{num(radius)})")
    if seam:
        line = s.line((radius, 0, z0), (0, 0, 1))
        # Supplied pcurves are retained as links and never read by the importer.
        ctx = s.add("(GEOMETRIC_REPRESENTATION_CONTEXT(2)PARAMETRIC_REPRESENTATION_CONTEXT()REPRESENTATION_CONTEXT('2D SPACE',''))")
        pcurves = []
        for u in (0.0, 2 * math.pi):
            p2 = s.add(f"CARTESIAN_POINT('',({num(u)},{num(z0)}))")
            d2 = s.add("DIRECTION('',(0.,1.))")
            v2 = s.add(f"VECTOR('',#{d2},1.)")
            l2 = s.add(f"LINE('',#{p2},#{v2})")
            rep = s.add(f"DEFINITIONAL_REPRESENTATION('',(#{l2}),#{ctx})")
            pcurves.append(s.add(f"PCURVE('',#{surface},#{rep})"))
        seam_curve = s.add(f"SEAM_CURVE('',#{line},(#{pcurves[0]},#{pcurves[1]}),.CURVE_3D.)")
        seam_edge = s.edge(v0, v1, seam_curve)
        side = s.face([s.bound([(bottom, True), (seam_edge, True), (top, False), (seam_edge, False)], outer=True)], surface)
    else:
        side = s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], surface)
    faces = [side]
    if caps:
        faces.append(s.face([s.bound([(bottom, False)], outer=cap_outer)], s.plane((0, 0, z0), (0, 0, 1)), same=False))
        faces.append(s.face([s.bound([(top, True)], outer=cap_outer)], s.plane((0, 0, z1), (0, 0, 1))))
    return faces


def fixtures():
    s = Step("Original solid cylinder r=10 h=20 mm; annular side face without a seam edge")
    s.write("brep-cylinder.step", cylinder(s, 10, 0, 20), context=context())

    s = Step("Original solid cylinder r=10 h=20 mm; side face with an explicit SEAM_CURVE and supplied PCURVEs")
    s.write("brep-seam-cylinder.step", cylinder(s, 10, 0, 20, seam=True))

    # Cone frustum widening upward: r=10 at z=0, semi-angle 30 degrees, height 10.
    s = Step("Original cone frustum r0=10 h=10 mm, semi-angle 30 degrees in a degree context")
    h, r0, alpha = 10.0, 10.0, math.radians(30)
    r1 = r0 + h * math.tan(alpha)
    v0 = s.vertex((r0, 0, 0))
    v1 = s.vertex((r1, 0, h))
    bottom = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), r0))
    top = s.edge(v1, v1, s.circle((0, 0, h), (0, 0, 1), (1, 0, 0), r1))
    cone = s.add(f"CONICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},{num(r0)},30.)")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], cone),
        s.face([s.bound([(bottom, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(top, True)])], s.plane((0, 0, h), (0, 0, 1))),
    ]
    s.write("brep-cone.step", faces, context=context(degrees=True))

    # Ring torus R=20, r=5 bounded by one parallel and one meridian seam.
    s = Step("Original ring torus R=20 r=5 mm with parallel and meridian seam circles")
    v = s.vertex((25, 0, 0))
    parallel = s.edge(v, v, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 25))
    meridian = s.edge(v, v, s.circle((20, 0, 0), (0, -1, 0), (1, 0, 0), 5))
    torus = s.add(f"TOROIDAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},20.,5.)")
    face = s.face([s.bound([(parallel, True), (meridian, True), (parallel, False), (meridian, False)], outer=True)], torus)
    s.write("brep-torus.step", [face])

    # Spherical zone (barrel) between z=-6 and z=6 on a sphere of radius 10.
    s = Step("Original spherical zone R=10 between z=-6 and z=6 mm with planar caps")
    v0 = s.vertex((8, 0, -6))
    v1 = s.vertex((8, 0, 6))
    bottom = s.edge(v0, v0, s.circle((0, 0, -6), (0, 0, 1), (1, 0, 0), 8))
    top = s.edge(v1, v1, s.circle((0, 0, 6), (0, 0, 1), (1, 0, 0), 8))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], sphere),
        s.face([s.bound([(bottom, False)])], s.plane((0, 0, -6), (0, 0, 1)), same=False),
        s.face([s.bound([(top, True)])], s.plane((0, 0, 6), (0, 0, 1))),
    ]
    s.write("brep-sphere-zone.step", faces)

    # Washer: outer r=20, inner r=10, thickness 5; planar faces carry two plain bounds.
    s = Step("Original washer R=20 r=10 t=5 mm; planar faces without FACE_OUTER_BOUND")
    vo0, vo1 = s.vertex((20, 0, 0)), s.vertex((20, 0, 5))
    vi0, vi1 = s.vertex((10, 0, 0)), s.vertex((10, 0, 5))
    ob = s.edge(vo0, vo0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 20))
    ot = s.edge(vo1, vo1, s.circle((0, 0, 5), (0, 0, 1), (1, 0, 0), 20))
    ib = s.edge(vi0, vi0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    it = s.edge(vi1, vi1, s.circle((0, 0, 5), (0, 0, 1), (1, 0, 0), 10))
    outer = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},20.)")
    inner = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(ob, True)]), s.bound([(ot, False)])], outer),
        # The bore faces inward: its loops are reversed relative to the outer cylinder.
        s.face([s.bound([(ib, False)]), s.bound([(it, True)])], inner, same=False),
        s.face([s.bound([(ob, False)]), s.bound([(ib, True)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(ot, True)]), s.bound([(it, False)])], s.plane((0, 0, 5), (0, 0, 1))),
    ]
    s.write("brep-washer.step", faces)

    # 10 mm cube whose top face is a degree-2 B-spline surface (a rational complex
    # instance) bounded by degree-2 B-spline edges, one of them a complex instance too.
    s = Step("Original 10 mm cube with a degree-2 B-spline top face and B-spline top edges")
    corners = [(0, 0, 0), (10, 0, 0), (10, 10, 0), (0, 10, 0)]
    vb = [s.vertex(p) for p in corners]
    vt = [s.vertex((x, y, 10)) for x, y, _ in corners]

    def bspline_edge(a, b, pa, pb, rational=False):
        mid = [(pa[k] + pb[k]) / 2 for k in range(3)]
        pts = ",".join(f"#{s.point(p)}" for p in (pa, mid, pb))
        if rational:
            curve = s.add(f"(BOUNDED_CURVE()B_SPLINE_CURVE(2,({pts}),.UNSPECIFIED.,.F.,.F.)"
                          "B_SPLINE_CURVE_WITH_KNOTS((3,3),(0.,1.),.UNSPECIFIED.)CURVE()"
                          "GEOMETRIC_REPRESENTATION_ITEM()RATIONAL_B_SPLINE_CURVE((1.,2.,1.))REPRESENTATION_ITEM(''))")
        else:
            curve = s.add(f"B_SPLINE_CURVE_WITH_KNOTS('',2,({pts}),.UNSPECIFIED.,.F.,.F.,(3,3),(0.,1.),.UNSPECIFIED.)")
        return s.edge(a, b, curve)

    bottom = [s.edge(vb[i], vb[(i + 1) % 4], s.line(corners[i], [corners[(i + 1) % 4][k] - corners[i][k] for k in range(3)]))
              for i in range(4)]
    top = [bspline_edge(vt[i], vt[(i + 1) % 4], (*corners[i][:2], 10), (*corners[(i + 1) % 4][:2], 10), rational=(i == 1))
           for i in range(4)]
    vertical = [s.edge(vb[i], vt[i], s.line(corners[i], (0, 0, 1))) for i in range(4)]
    rows = []
    for i in range(3):
        rows.append("(" + ",".join(f"#{s.point((5 * i, 5 * j, 10))}" for j in range(3)) + ")")
    weights = "((1.,1.,1.),(1.,2.,1.),(1.,1.,1.))"
    surface = s.add(f"(BOUNDED_SURFACE()B_SPLINE_SURFACE(2,2,({','.join(rows)}),.PLANE_SURF.,.F.,.F.,.F.)"
                    "B_SPLINE_SURFACE_WITH_KNOTS((3,3),(3,3),(0.,1.),(0.,1.),.UNSPECIFIED.)"
                    f"GEOMETRIC_REPRESENTATION_ITEM()RATIONAL_B_SPLINE_SURFACE({weights})REPRESENTATION_ITEM('')SURFACE())")
    faces = [
        s.face([s.bound([(bottom[3], False), (bottom[2], False), (bottom[1], False), (bottom[0], False)], outer=True)],
               s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(top[0], True), (top[1], True), (top[2], True), (top[3], True)], outer=True)], surface),
    ]
    for i in range(4):
        j = (i + 1) % 4
        normal = [(0, -1, 0), (1, 0, 0), (0, 1, 0), (-1, 0, 0)][i]
        ref = [corners[j][k] - corners[i][k] for k in range(3)]
        faces.append(s.face([s.bound([(bottom[i], True), (vertical[j], True), (top[i], False), (vertical[i], False)], outer=True)],
                            s.plane(corners[i], normal, [r / 10 for r in ref])))
    s.write("brep-bspline-cube.step", faces)

    # Negative: circle edges of radius 10 on a cylinder of radius 10.5.
    s = Step("Original rejected solid: circle edges do not lie on their cylinder")
    faces = cylinder(s, 10, 0, 20)
    s.lines = [line.replace("CYLINDRICAL_SURFACE('',#", "CYLINDRICAL_SURFACE('',#") for line in s.lines]
    s.lines = [line.replace(",10.0)", ",10.5)") if "CYLINDRICAL_SURFACE" in line else line for line in s.lines]
    s.write("brep-off-surface.step", faces)

    # The annular side face has its loop vertices a quarter turn apart: the importer
    # splits the top circle where the seam through the bottom vertex crosses it.
    s = Step("Original solid cylinder r=10 h=20 mm; annular side face whose loop vertices are a quarter turn apart")
    v0 = s.vertex((10, 0, 0))
    v1 = s.vertex((0, 10, 20))
    bottom = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    top = s.edge(v1, v1, s.circle((0, 0, 20), (0, 0, 1), (0, 1, 0), 10))
    surface = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], surface),
        s.face([s.bound([(bottom, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(top, True)])], s.plane((0, 0, 20), (0, 0, 1))),
    ]
    s.write("brep-misaligned.step", faces)

    # Spherical cap R=10 above z=6 on a planar disk; the STEP frame's north pole lies
    # inside the cap, so the importer plans the face in a rotated sphere frame.
    s = Step("Original spherical cap R=10 mm above z=6 mm on a planar base")
    v = s.vertex((8, 0, 6))
    rim = s.edge(v, v, s.circle((0, 0, 6), (0, 0, 1), (1, 0, 0), 8))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(rim, True)])], sphere),
        s.face([s.bound([(rim, False)])], s.plane((0, 0, 6), (0, 0, 1)), same=False),
    ]
    s.write("brep-sphere-cap.step", faces)

    # Hemisphere R=10 on a planar disk: every sphere frame puts a pole on or inside
    # the face, so it needs pole charts, which are unsupported.
    s = Step("Original unsupported hemisphere R=10 mm on a planar base")
    v = s.vertex((10, 0, 0))
    rim = s.edge(v, v, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(rim, True)])], sphere),
        s.face([s.bound([(rim, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-hemisphere.step", faces)

    # Negative: cone tip closed by a VERTEX_LOOP at the apex.
    s = Step("Original unsupported solid: cone tip bounded by a vertex loop")
    apex = s.vertex((0, 0, 10))
    v0 = s.vertex((10, 0, 0))
    base = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    cone = s.add(f"CONICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, -1), (1, 0, 0))},10.,{num(math.pi / 4)})")
    vertex_loop = s.add(f"VERTEX_LOOP('',#{apex})")
    faces = [
        s.face([s.bound([(base, False)]), s.add(f"FACE_BOUND('',#{vertex_loop},.T.)")], cone),
        s.face([s.bound([(base, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-vertex-loop.step", faces)

    # Negative: planar face with two counterclockwise loops and no FACE_OUTER_BOUND.
    s = Step("Original rejected solid: ambiguous outer bound on a planar face")
    faces = cylinder(s, 10, 0, 20, cap_outer=False)
    v = s.vertex((3, 0, 20))
    extra = s.edge(v, v, s.circle((0, 0, 20), (0, 0, 1), (1, 0, 0), 3))
    s.lines = [line for line in s.lines]
    top_face = faces[2]
    line = next(l for l in s.lines if l.startswith(f"#{top_face}="))
    bound = s.bound([(extra, True)])
    s.lines[s.lines.index(line)] = line.replace("(#", f"(#{bound},#", 1)
    s.write("brep-ambiguous-outer.step", faces)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=HERE, help="Directory for the generated fixtures")
    OUTPUT = parser.parse_args().output
    fixtures()
