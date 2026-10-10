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
    """Format a real with 15 significant digits so that one-ulp differences between
    platform libm results (for example ``tan``) never change the fixture bytes."""
    if abs(x) < 5e-16:
        x = 0.0
    text = repr(float(f"{float(x):.15g}")).replace("e", "E")
    if "." in text:
        return text
    # STEP reals need the decimal point, also before an exponent.
    return text.replace("E", ".E") if "E" in text else text + "."


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

    def write(self, name, faces, root="MANIFOLD_SOLID_BREP", context=None, voids=()):
        shell = self.add(f"CLOSED_SHELL('',({','.join(f'#{f}' for f in faces)}))")
        assert self.next < 1000
        if voids:
            self.lines.append(f"#1000={root}('',#{shell},({','.join(f'#{v}' for v in voids)}));")
        else:
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
        (OUTPUT / name).write_text(text, newline="\n")


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
        # Supplied seam pcurves: u = 0 and u = 2 pi lines on the cylinder.
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


def box(s, lo, hi):
    """Axis-aligned box with outward faces. Returns faces."""
    corners = {}
    for i in range(8):
        p = tuple(hi[k] if (i >> k) & 1 else lo[k] for k in range(3))
        corners[i] = (p, s.vertex(p))
    edges = {}

    def edge(a, b):
        if (b, a) in edges:
            return edges[(b, a)], False
        if (a, b) not in edges:
            pa, pb = corners[a][0], corners[b][0]
            edges[(a, b)] = s.edge(corners[a][1], corners[b][1], s.line(pa, [pb[k] - pa[k] for k in range(3)]))
        return edges[(a, b)], True

    faces = []
    # Each face: corner indices counterclockwise seen from outside, normal, reference.
    for ring, normal, ref in [
        ((0, 2, 3, 1), (0, 0, -1), (0, 1, 0)), ((4, 5, 7, 6), (0, 0, 1), (1, 0, 0)),
        ((0, 1, 5, 4), (0, -1, 0), (1, 0, 0)), ((2, 6, 7, 3), (0, 1, 0), (0, 0, 1)),
        ((0, 4, 6, 2), (-1, 0, 0), (0, 0, 1)), ((1, 3, 7, 5), (1, 0, 0), (0, 1, 0)),
    ]:
        uses = [edge(ring[i], ring[(i + 1) % 4]) for i in range(4)]
        faces.append(s.face([s.bound(uses, outer=True)], s.plane(corners[ring[0]][0], normal, ref)))
    return faces


def polyhedron(s, points, rings):
    """Planar faces of a polyhedron: corner coordinates and, per face, a ring of corner
    indices counterclockwise seen from outside with the plane's normal and reference
    direction. Edges are shared by corner pair, so a ring may run out and back along
    two edges of one line."""
    vertices = [s.vertex(p) for p in points]
    edges = {}

    def edge(a, b):
        if (b, a) in edges:
            return edges[(b, a)], False
        if (a, b) not in edges:
            pa, pb = points[a], points[b]
            edges[(a, b)] = s.edge(vertices[a], vertices[b], s.line(pa, [pb[k] - pa[k] for k in range(3)]))
        return edges[(a, b)], True

    faces = []
    for ring, normal, ref in rings:
        uses = [edge(ring[i], ring[(i + 1) % len(ring)]) for i in range(len(ring))]
        faces.append(s.face([s.bound(uses, outer=True)], s.plane(points[ring[1]], normal, ref)))
    return faces


def fixtures():
    s = Step("Original solid cylinder r=10 h=20 mm; annular side face without a seam edge")
    s.write("brep-cylinder.step", cylinder(s, 10, 0, 20), context=context())

    s = Step("Original solid cylinder r=10 h=20 mm; side face with an explicit SEAM_CURVE and supplied PCURVEs")
    s.write("brep-seam-cylinder.step", cylinder(s, 10, 0, 20, seam=True))

    # A seamed cylinder whose circles are SURFACE_CURVEs with supplied pcurves on the
    # side (v = z lines) and on each cap (2D circles); the top circle's side pcurve is
    # written 1 mm too high. Every use has an exact computed image, which wins, so no
    # supplied pcurve (right or wrong) is used.
    s = Step("Original seamed cylinder whose circle edges carry supplied pcurves, one of them wrong")
    faces = cylinder(s, 10, 0, 20, seam=True)
    ctx = s.add("(GEOMETRIC_REPRESENTATION_CONTEXT(2)PARAMETRIC_REPRESENTATION_CONTEXT()REPRESENTATION_CONTEXT('2D SPACE',''))")
    side = next(l for l in s.lines if "CYLINDRICAL_SURFACE" in l).split("=")[0][1:]
    planes = [l.split("=")[0][1:] for l in s.lines if "=PLANE(" in l]

    def pcurve(surface, curve):
        representation = s.add(f"DEFINITIONAL_REPRESENTATION('',(#{curve}),#{ctx})")
        return s.add(f"PCURVE('',#{surface},#{representation})")

    def line2(p, d):
        point = s.add(f"CARTESIAN_POINT('',({num(p[0])},{num(p[1])}))")
        direction = s.add(f"DIRECTION('',({num(d[0])},{num(d[1])}))")
        vector = s.add(f"VECTOR('',#{direction},1.)")
        return s.add(f"LINE('',#{point},#{vector})")

    def circle2(radius):
        centre = s.add("CARTESIAN_POINT('',(0.,0.))")
        reference = s.add("DIRECTION('',(1.,0.))")
        placement = s.add(f"AXIS2_PLACEMENT_2D('',#{centre},#{reference})")
        return s.add(f"CIRCLE('',#{placement},{num(radius)})")

    circles = [l for l in s.lines if "=CIRCLE(" in l]
    for (z, written), plane, line in zip(((0, 0), (20, 21)), planes, circles):
        circle = line.split("=")[0][1:]
        wrapped = s.add(f"SURFACE_CURVE('',#{circle},(#{pcurve(side, line2((0, written), (1, 0)))},"
                        f"#{pcurve(plane, circle2(10))}),.CURVE_3D.)")
        s.lines = [l.replace(f",#{circle},.T.)", f",#{wrapped},.T.)") if l.startswith("#") and "EDGE_CURVE" in l else l
                   for l in s.lines]
    s.write("brep-pcurve-cylinder.step", faces)


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

    # Spindle torus a=5, b=12: the tube crosses the axis at z=±sqrt(119). The apple
    # (select_outer) zone and the lemon zone between z=-6 and z=6 have parallel radii
    # 5+sqrt(108) and sqrt(108)-5; the lemon's du x dv points at the axis, so its
    # face is reversed.
    for outer, name in ((True, "apple"), (False, "lemon")):
        radius = 5 + 108 ** 0.5 if outer else 108 ** 0.5 - 5
        s = Step(f"Original spindle torus {name} zone a=5 b=12 mm between z=-6 and z=6 with planar caps")
        v0 = s.vertex((radius, 0, -6))
        v1 = s.vertex((radius, 0, 6))
        bottom = s.edge(v0, v0, s.circle((0, 0, -6), (0, 0, 1), (1, 0, 0), radius))
        top = s.edge(v1, v1, s.circle((0, 0, 6), (0, 0, 1), (1, 0, 0), radius))
        spindle = s.add(f"DEGENERATE_TOROIDAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},5.,12.,.{'T' if outer else 'F'}.)")
        faces = [
            s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], spindle, same=outer),
            s.face([s.bound([(bottom, False)])], s.plane((0, 0, -6), (0, 0, 1)), same=False),
            s.face([s.bound([(top, True)])], s.plane((0, 0, 6), (0, 0, 1))),
        ]
        s.write(f"brep-spindle-{name}.step", faces)

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


    # The B-spline cube with its top edges as SURFACE_CURVEs carrying supplied pcurves
    # on the B-spline top face, which has no exact computed images: lines, and for the
    # rational edge a rational quadratic with the edge's weights, so all four agree.
    lines = (OUTPUT / "brep-bspline-cube.step").read_text().splitlines()
    data = [l for l in lines if l.startswith("#")]
    top_surface = next(l for l in data if "(BOUNDED_SURFACE()B_SPLINE_SURFACE(" in l).split("=")[0]
    curves = [l.split("=")[0] for l in data if "B_SPLINE_CURVE" in l]
    extra = []
    ids = iter(range(700, 1000))

    def new(body):
        name = f"#{next(ids)}"
        extra.append(f"{name}={body};")
        return name

    ctx = new("(GEOMETRIC_REPRESENTATION_CONTEXT(2)PARAMETRIC_REPRESENTATION_CONTEXT()REPRESENTATION_CONTEXT('2D SPACE',''))")
    # Top edges run corner i to corner i+1: (0,0)->(1,0)->(1,1)->(0,1)->(0,0) in (u, v).
    starts = [((0, 0), (1, 0)), ((1, 0), (0, 1)), ((1, 1), (-1, 0)), ((0, 1), (0, -1))]
    wrapped = {}
    for i, (curve, (origin, direction)) in enumerate(zip(curves, starts)):
        if i == 1:
            poles = ",".join(new(f"CARTESIAN_POINT('',(1.,{num(v)}))") for v in (0, 0.5, 1))
            line = new(f"(BOUNDED_CURVE()B_SPLINE_CURVE(2,({poles}),.UNSPECIFIED.,.F.,.F.)"
                       "B_SPLINE_CURVE_WITH_KNOTS((3,3),(0.,1.),.UNSPECIFIED.)CURVE()"
                       "GEOMETRIC_REPRESENTATION_ITEM()RATIONAL_B_SPLINE_CURVE((1.,2.,1.))REPRESENTATION_ITEM(''))")
        else:
            point = new(f"CARTESIAN_POINT('',({num(origin[0])},{num(origin[1])}))")
            unit = new(f"DIRECTION('',({num(direction[0])},{num(direction[1])}))")
            vector = new("VECTOR(''," + unit + ",1.)")
            line = new(f"LINE('',{point},{vector})")
        representation = new(f"DEFINITIONAL_REPRESENTATION('',({line}),{ctx})")
        pcurve = new(f"PCURVE('',{top_surface},{representation})")
        wrapped[curve] = new(f"SURFACE_CURVE('',{curve},({pcurve}),.PCURVE_S1.)")
    body = []
    for l in lines:
        if l.startswith("#") and "EDGE_CURVE(" in l:
            for curve, surface_curve in wrapped.items():
                l = l.replace(f",{curve},", f",{surface_curve},")
        body.append(l)
    at = body.index("ENDSEC;", body.index("DATA;"))
    body[at:at] = extra
    body[2] = "FILE_DESCRIPTION(('Original 10 mm B-spline cube whose top edges carry supplied pcurves on the B-spline face'),'2;1');"
    body[3] = body[3].replace("brep-bspline-cube.step", "brep-pcurve-bspline-cube.step")
    (OUTPUT / "brep-pcurve-bspline-cube.step").write_text("\n".join(body) + "\n", newline="\n")

    # 10 mm cubes whose top face and top edges use implicit-knot forms. Piecewise
    # Bezier: two quadratic segments per direction (interior knot multiplicity 2) and
    # the 3 x 3 interior poles raised 4 mm, so the boundary stays planar, the joins
    # stay C1 and the bulge adds 4 * (20/3)^2 = 1600/9 mm3. Uniform: unclamped knots -d..N+1, so the control nets
    # are extended to make each curve end on its vertices.
    for form in ("BEZIER", "UNIFORM"):
        s = Step(f"Original 10 mm cube with a degree-2 {form}_SURFACE top face and {form}_CURVE top edges")
        vb = [s.vertex(p) for p in corners]
        vt = [s.vertex((x, y, 10)) for x, y, _ in corners]

        def implicit_edge(a, b, pa, pb):
            mid = [(pa[k] + pb[k]) / 2 for k in range(3)]
            if form == "BEZIER":
                poles = [[pa[k] + (pb[k] - pa[k]) * i / 4 for k in range(3)] for i in range(5)]
            else:
                poles = ([2 * pa[k] - mid[k] for k in range(3)], mid, [2 * pb[k] - mid[k] for k in range(3)])
            pts = ",".join(f"#{s.point(p)}" for p in poles)
            return s.edge(a, b, s.add(f"{form}_CURVE('',2,({pts}),.UNSPECIFIED.,.F.,.F.)"))

        bottom = [s.edge(vb[i], vb[(i + 1) % 4], s.line(corners[i], [corners[(i + 1) % 4][k] - corners[i][k] for k in range(3)]))
                  for i in range(4)]
        top = [implicit_edge(vt[i], vt[(i + 1) % 4], (*corners[i][:2], 10), (*corners[(i + 1) % 4][:2], 10))
               for i in range(4)]
        vertical = [s.edge(vb[i], vt[i], s.line(corners[i], (0, 0, 1))) for i in range(4)]
        grid = (0, 2.5, 5, 7.5, 10) if form == "BEZIER" else (-5, 5, 15)
        raised = {1, 2, 3} if form == "BEZIER" else set()
        rows = ["(" + ",".join(f"#{s.point((x, y, 14 if i in raised and j in raised else 10))}"
                               for j, y in enumerate(grid)) + ")" for i, x in enumerate(grid)]
        surface = s.add(f"{form}_SURFACE('',2,2,({','.join(rows)}),.PLANE_SURF.,.F.,.F.,.F.)")
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
        s.write(f"brep-{form.lower()}-cube.step", faces)

    # 10 mm prisms over a region between an open conic arc (u in [-1, 1]) and its
    # chord: a parabola with focal distance 5 (area 200/3 mm2) and a rectangular
    # hyperbola with semi-axes 5 (area 12.5 (sinh 2 - 2) mm2). The curved side is the
    # arc's exact quadratic, or rational quadratic, B-spline extrusion.
    for kind in ("parabola", "hyperbola"):
        s = Step(f"Original 10 mm prism bounded by a {kind.upper()} arc and its chord")
        if kind == "parabola":
            conic = lambda z: s.add(f"PARABOLA('',#{s.placement((0, 0, z), (0, 0, 1), (1, 0, 0))},5.)")
            poles, weight = [(5, -10), (-5, 0), (5, 10)], 1.
        else:
            conic = lambda z: s.add(f"HYPERBOLA('',#{s.placement((0, 0, z), (0, 0, 1), (1, 0, 0))},5.,5.)")
            c1, s1 = math.cosh(1), math.sinh(1)
            poles, weight = [(5 * c1, -5 * s1), (5 / c1, 0), (5 * c1, 5 * s1)], c1
        a, b = poles[0], poles[2]
        va, vb = s.vertex((*a, 0)), s.vertex((*b, 0))
        vc, vd = s.vertex((*a, 10)), s.vertex((*b, 10))
        arc0, arc1 = s.edge(va, vb, conic(0)), s.edge(vc, vd, conic(10))
        chord0 = s.edge(vb, va, s.line((*b, 0), (0, -1, 0)))
        chord1 = s.edge(vd, vc, s.line((*b, 10), (0, -1, 0)))
        side_a = s.edge(va, vc, s.line((*a, 0), (0, 0, 1)))
        side_b = s.edge(vb, vd, s.line((*b, 0), (0, 0, 1)))
        rows = ",".join("(" + ",".join(f"#{s.point((x, y, z))}" for z in (0, 10)) + ")" for x, y in poles)
        common = f"B_SPLINE_SURFACE(2,1,({rows}),.UNSPECIFIED.,.F.,.F.,.F.)B_SPLINE_SURFACE_WITH_KNOTS((3,3),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.)"
        if weight == 1.:
            surface = s.add(f"B_SPLINE_SURFACE_WITH_KNOTS('',2,1,({rows}),.UNSPECIFIED.,.F.,.F.,.F.,(3,3),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.)")
        else:
            w = num(weight)
            surface = s.add(f"(BOUNDED_SURFACE(){common}GEOMETRIC_REPRESENTATION_ITEM()"
                            f"RATIONAL_B_SPLINE_SURFACE(((1.,1.),({w},{w}),(1.,1.)))REPRESENTATION_ITEM('')SURFACE())")
        faces = [
            s.face([s.bound([(arc0, True), (chord0, True)], outer=True)], s.plane((0, 0, 0), (0, 0, -1)), same=True),
            s.face([s.bound([(chord1, False), (arc1, False)], outer=True)], s.plane((0, 0, 10), (0, 0, 1))),
            s.face([s.bound([(chord0, False), (side_b, True), (chord1, True), (side_a, False)], outer=True)],
                   s.plane((*b, 0), (1, 0, 0), (0, 1, 0))),
            # The surface normal (arc tangent x z) points into the prism.
            s.face([s.bound([(side_a, True), (arc1, True), (side_b, False), (arc0, False)], outer=True)], surface, same=False),
        ]
        s.write(f"brep-{kind}-prism.step", faces)

    # Negative: circle edges of radius 10 on a cylinder of radius 10.5.
    s = Step("Original rejected solid: circle edges do not lie on their cylinder")
    faces = cylinder(s, 10, 0, 20)
    s.lines = [line.replace("CYLINDRICAL_SURFACE('',#", "CYLINDRICAL_SURFACE('',#") for line in s.lines]
    s.lines = [line.replace(",10.0)", ",10.5)") if "CYLINDRICAL_SURFACE" in line else line for line in s.lines]
    s.write("brep-off-surface.step", faces)

    # Tolerance policy (Milestone 22.3): a 1 um context uncertainty and a cylinder
    # r=10 h=20 whose top circle edge misses the side surface by 3 um (accepted, 3x
    # the tolerance) or 50 um (rejected, beyond 16x), and one whose top vertex sits
    # 3 um outside its circle in the top plane (accepted).
    def offset_cylinder(s, top_radius, top_vertex):
        v0 = s.vertex((10, 0, 0))
        v1 = s.vertex(top_vertex)
        bottom = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
        top = s.edge(v1, v1, s.circle((0, 0, 20), (0, 0, 1), (1, 0, 0), top_radius))
        surface = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
        return [
            s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], surface),
            s.face([s.bound([(bottom, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
            s.face([s.bound([(top, True)])], s.plane((0, 0, 20), (0, 0, 1))),
        ]

    for name, radius, vertex, description in [
        ("brep-near-surface.step", 10.003, (10.003, 0, 20), "top circle edge 3 um off its side cylinder"),
        ("brep-far-surface.step", 10.05, (10.05, 0, 20), "top circle edge 50 um off its side cylinder"),
        ("brep-vertex-off-curve.step", 10, (10.003, 0, 20), "top vertex 3 um off its circle edge"),
    ]:
        s = Step(f"Original cylinder r=10 h=20 mm under a 1 um uncertainty; {description}")
        s.write(name, offset_cylinder(s, radius, vertex), context=context(uncertainty="1.E-3"))

    # 10 mm tetrahedron ABCD with A cut by a 20 um edge A'A, A' = (-t, t, 0): the
    # bilinear B-spline base (A, A', B, C) has that edge as its u=0 side, which
    # collapses only within the 50 um uncertainty, so the side is charted as a regular
    # boundary, not a pole. A sliver face (A, D, A') on x + y = 0 and a face (C, A', D)
    # close the solid; the volume is (1000 + 100 t) / 6 mm3.
    s = Step("Original 10 mm tetrahedron whose B-spline base has a 20 um side along an edge, under a 50 um uncertainty")
    t = 0.014
    a, a2, b, c, d = (0, 0, 0), (-t, t, 0), (10, 0, 0), (0, 10, 0), (0, 0, 10)
    va, va2, vb, vc, vd = (s.vertex(p) for p in (a, a2, b, c, d))

    def segment(p, q, vp, vq):
        return s.edge(vp, vq, s.line(p, [q[k] - p[k] for k in range(3)]))

    def unit(v):
        n = math.sqrt(sum(x * x for x in v))
        return [x / n for x in v]

    def cross(u, v):
        return [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]

    ab, bc, ca2, a2a = segment(a, b, va, vb), segment(b, c, vb, vc), segment(c, a2, vc, va2), segment(a2, a, va2, va)
    ad, bd, cd, a2d = segment(a, d, va, vd), segment(b, d, vb, vd), segment(c, d, vc, vd), segment(a2, d, va2, vd)
    patch = s.add(f"B_SPLINE_SURFACE_WITH_KNOTS('',1,1,((#{s.point(a)},#{s.point(a2)}),(#{s.point(b)},#{s.point(c)})),"
                  ".UNSPECIFIED.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.)")
    root3 = 1 / math.sqrt(3)
    near_x = unit(cross([c[k] - d[k] for k in range(3)], [a2[k] - c[k] for k in range(3)]))
    faces = [
        # The patch normal is +z; the base faces -z.
        s.face([s.bound([(a2a, False), (ca2, False), (bc, False), (ab, False)], outer=True)], patch, same=False),
        s.face([s.bound([(ab, True), (bd, True), (ad, False)], outer=True)], s.plane(a, (0, -1, 0))),
        s.face([s.bound([(cd, False), (ca2, True), (a2d, True)], outer=True)], s.plane(d, near_x, (0, 1, 0))),
        s.face([s.bound([(ad, True), (a2d, False), (a2a, True)], outer=True)],
               s.plane(a, (-1 / math.sqrt(2), -1 / math.sqrt(2), 0), (0, 0, 1))),
        s.face([s.bound([(bc, True), (cd, True), (bd, False)], outer=True)],
               s.plane(b, (root3, root3, root3), (-1 / math.sqrt(2), 1 / math.sqrt(2), 0))),
    ]
    s.write("brep-short-side.step", faces, context=context(uncertainty="5.E-2"))

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
    # the face, so a seam to the enclosed pole and a collapsed pole edge close its chart.
    s = Step("Original hemisphere R=10 mm on a planar base; every sphere frame has a pole on or inside the face")
    v = s.vertex((10, 0, 0))
    rim = s.edge(v, v, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(rim, True)])], sphere),
        s.face([s.bound([(rim, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-hemisphere.step", faces)

    # Cone tip R=10 h=10 closed by a VERTEX_LOOP at the apex.
    s = Step("Original cone R=10 h=10 mm whose lateral face is bounded by its base circle and a vertex loop at the apex")
    apex = s.vertex((0, 0, 10))
    v0 = s.vertex((10, 0, 0))
    base = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    cone = s.add(f"CONICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, -1), (1, 0, 0))},10.,{num(math.pi / 4)})")
    vertex_loop = s.add(f"VERTEX_LOOP('',#{apex})")
    faces = [
        s.face([s.bound([(base, True)]), s.add(f"FACE_BOUND('',#{vertex_loop},.T.)")], cone),
        s.face([s.bound([(base, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-vertex-loop.step", faces)

    # Cone tip R=10 h=10 whose lateral loop runs up a generator seam to the apex vertex
    # and back: the loop meets the apex, where a collapsed edge joins its chart ends.
    s = Step("Original cone R=10 h=10 mm whose lateral loop runs up a seam generator to the apex and back")
    apex = s.vertex((0, 0, 10))
    v0 = s.vertex((10, 0, 0))
    base = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    generator = s.edge(v0, apex, s.line((10, 0, 0), (-1 / math.sqrt(2), 0, 1 / math.sqrt(2))))
    cone = s.add(f"CONICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, -1), (1, 0, 0))},10.,{num(math.pi / 4)})")
    faces = [
        s.face([s.bound([(base, True), (generator, True), (generator, False)], outer=True)], cone),
        s.face([s.bound([(base, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-cone-seam.step", faces)

    # Full sphere R=10 bounded only by a VERTEX_LOOP on the STEP frame's equator: the
    # face is planned in the frame whose pole is that vertex, between both poles.
    s = Step("Original sphere R=10 mm bounded by one vertex loop")
    point = s.vertex((10, 0, 0))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    vertex_loop = s.add(f"VERTEX_LOOP('',#{point})")
    s.write("brep-sphere.step", [s.face([s.add(f"FACE_BOUND('',#{vertex_loop},.T.)")], sphere)])

    # Full sphere R=10 with a meridian seam edge from the south to the north pole,
    # used in both directions: exact meridian pcurves and a collapsed edge at each pole.
    s = Step("Original sphere R=10 mm bounded by a pole-to-pole meridian seam edge")
    south, north = s.vertex((0, 0, -10)), s.vertex((0, 0, 10))
    meridian = s.edge(south, north, s.circle((0, 0, 0), (0, -1, 0), (0, 0, -1), 10))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    s.write("brep-sphere-seam.step", [s.face([s.bound([(meridian, True), (meridian, False)], outer=True)], sphere)])

    # Tetrahedron with legs of 10 mm whose base is a bilinear B-spline patch with its
    # u=0 side collapsed to the corner A: the base loop meets that side at A, where a
    # collapsed edge joins its chart ends.
    s = Step("Original 10 mm tetrahedron whose base is a bilinear B-spline patch with one side collapsed to a corner")
    a, b, c, d = (0, 0, 0), (10, 0, 0), (0, 10, 0), (0, 0, 10)
    va, vb, vc, vd = (s.vertex(p) for p in (a, b, c, d))

    def segment(p, q, vp, vq):
        return s.edge(vp, vq, s.line(p, [q[k] - p[k] for k in range(3)]))

    ab, bc, ca = segment(a, b, va, vb), segment(b, c, vb, vc), segment(c, a, vc, va)
    ad, bd, cd = segment(a, d, va, vd), segment(b, d, vb, vd), segment(c, d, vc, vd)
    pa = s.point(a)
    patch = s.add(f"B_SPLINE_SURFACE_WITH_KNOTS('',1,1,((#{pa},#{pa}),(#{s.point(b)},#{s.point(c)})),"
                  ".UNSPECIFIED.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.)")
    root3 = 1 / math.sqrt(3)
    faces = [
        # The patch normal is +z; the base faces -z.
        s.face([s.bound([(ca, False), (bc, False), (ab, False)], outer=True)], patch, same=False),
        s.face([s.bound([(ab, True), (bd, True), (ad, False)], outer=True)], s.plane(a, (0, -1, 0))),
        s.face([s.bound([(ad, True), (cd, False), (ca, True)], outer=True)], s.plane(a, (-1, 0, 0), (0, 1, 0))),
        s.face([s.bound([(bc, True), (cd, True), (bd, False)], outer=True)],
               s.plane(b, (root3, root3, root3), (-1 / math.sqrt(2), 1 / math.sqrt(2), 0))),
    ]
    s.write("brep-bspline-pole.step", faces)

    # Cylinder r=10 h=20 whose side is SURFACE_OF_LINEAR_EXTRUSION of its base circle:
    # an exact cylinder chart with one inserted seam.
    s = Step("Original cylinder r=10 h=20 mm whose side is an extruded circle")
    v0, v1 = s.vertex((10, 0, 0)), s.vertex((10, 0, 20))
    circle = s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10)
    bottom = s.edge(v0, v0, circle)
    top = s.edge(v1, v1, s.circle((0, 0, 20), (0, 0, 1), (1, 0, 0), 10))
    extrusion = s.add(f"SURFACE_OF_LINEAR_EXTRUSION('',#{circle},#{s.add(f'VECTOR({chr(39)}{chr(39)},#{s.direction((0, 0, 1))},20.)')})")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], extrusion),
        s.face([s.bound([(bottom, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(top, True)])], s.plane((0, 0, 20), (0, 0, 1))),
    ]
    s.write("brep-extruded-cylinder.step", faces)

    # Elliptic prism a=15 b=10 h=20: the side extrudes a TRIMMED_CURVE of the base
    # ellipse whose sense disagrees with it, so the STEP surface normal points inward
    # and the face uses same_sense .F.; the chart is a periodic NURBS extrusion cut by
    # an isocurve seam.
    s = Step("Original elliptic prism a=15 b=10 h=20 mm whose side extrudes a reversed trimmed ellipse")
    v0, v1 = s.vertex((15, 0, 0)), s.vertex((15, 0, 20))

    def ellipse(z):
        return s.add(f"ELLIPSE('',#{s.placement((0, 0, z), (0, 0, 1), (1, 0, 0))},15.,10.)")

    base = ellipse(0)
    bottom = s.edge(v0, v0, base)
    top = s.edge(v1, v1, ellipse(20))
    trimmed = s.add(f"TRIMMED_CURVE('',#{base},(PARAMETER_VALUE(0.)),(PARAMETER_VALUE({num(2 * math.pi)})),.F.,.PARAMETER.)")
    extrusion = s.add(f"SURFACE_OF_LINEAR_EXTRUSION('',#{trimmed},#{s.add(f'VECTOR({chr(39)}{chr(39)},#{s.direction((0, 0, 1))},1.)')})")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], extrusion, same=False),
        s.face([s.bound([(bottom, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(top, True)])], s.plane((0, 0, 20), (0, 0, 1))),
    ]
    s.write("brep-extruded-ellipse.step", faces)

    # Cone tip R=10 h=10 whose lateral face revolves its generator line about z: an
    # exact cone chart closed at the apex by a VERTEX_LOOP.
    s = Step("Original cone R=10 h=10 mm whose lateral face revolves a line, with a vertex loop at the apex")
    apex = s.vertex((0, 0, 10))
    v0 = s.vertex((10, 0, 0))
    base = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    generator = s.line((10, 0, 0), (-1 / math.sqrt(2), 0, 1 / math.sqrt(2)))
    axis = s.add(f"AXIS1_PLACEMENT('',#{s.point((0, 0, 0))},#{s.direction((0, 0, 1))})")
    revolution = s.add(f"SURFACE_OF_REVOLUTION('',#{generator},#{axis})")
    vertex_loop = s.add(f"VERTEX_LOOP('',#{apex})")
    faces = [
        s.face([s.bound([(base, True)]), s.add(f"FACE_BOUND('',#{vertex_loop},.T.)")], revolution),
        s.face([s.bound([(base, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-revolved-cone.step", faces)

    # Sphere R=10 revolving a rational B-spline semicircle from pole to pole: a NURBS
    # chart whose two profile ends collapse to the poles; the profile is also the face's
    # seam edge, used in both directions.
    s = Step("Original sphere R=10 mm revolving a rational B-spline semicircle, bounded by the profile as a seam")
    south, north = s.vertex((0, 0, -10)), s.vertex((0, 0, 10))
    pts = [(0, 0, -10), (10, 0, -10), (10, 0, 0), (10, 0, 10), (0, 0, 10)]
    q = num(1 / math.sqrt(2))
    profile = s.add("(BOUNDED_CURVE()B_SPLINE_CURVE(2,(" + ",".join(f"#{s.point(p)}" for p in pts)
                    + "),.CIRCULAR_ARC.,.F.,.F.)B_SPLINE_CURVE_WITH_KNOTS((3,2,3),(0.,0.5,1.),.UNSPECIFIED.)"
                    f"CURVE()GEOMETRIC_REPRESENTATION_ITEM()RATIONAL_B_SPLINE_CURVE((1.,{q},1.,{q},1.))REPRESENTATION_ITEM(''))")
    meridian = s.edge(south, north, profile)
    axis = s.add(f"AXIS1_PLACEMENT('',#{s.point((0, 0, 0))},#{s.direction((0, 0, 1))})")
    revolution = s.add(f"SURFACE_OF_REVOLUTION('',#{profile},#{axis})")
    s.write("brep-revolved-sphere.step",
            [s.face([s.bound([(meridian, True), (meridian, False)], outer=True)], revolution)])

    # Ring torus R=20 r=5 revolving its meridian circle about z: an exact torus chart.
    s = Step("Original ring torus R=20 r=5 mm revolving a circle, with parallel and meridian seam circles")
    v = s.vertex((25, 0, 0))
    parallel = s.edge(v, v, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 25))
    profile = s.circle((20, 0, 0), (0, -1, 0), (1, 0, 0), 5)
    meridian = s.edge(v, v, profile)
    axis = s.add(f"AXIS1_PLACEMENT('',#{s.point((0, 0, 0))},#{s.direction((0, 0, 1))})")
    revolution = s.add(f"SURFACE_OF_REVOLUTION('',#{profile},#{axis})")
    face = s.face([s.bound([(parallel, True), (meridian, True), (parallel, False), (meridian, False)], outer=True)], revolution)
    s.write("brep-revolved-torus.step", [face])

    # 20 mm cube with a centred 10 mm cubical cavity: BREP_WITH_VOIDS whose void is an
    # outward inner cube shell reversed by ORIENTED_CLOSED_SHELL(.F.).
    s = Step("Original 20 mm cube with a centred 10 mm cubical cavity")
    outer = box(s, (0, 0, 0), (20, 20, 20))
    inner = s.add(f"CLOSED_SHELL('',({','.join(f'#{f}' for f in box(s, (5, 5, 5), (15, 15, 15)))}))")
    void = s.add(f"ORIENTED_CLOSED_SHELL('',*,#{inner},.F.)")
    s.write("brep-box-cavity.step", outer, root="BREP_WITH_VOIDS", voids=[void])

    # Cylinder r=10 h=20 with a spherical cavity r=5 at its centre, the sphere bounded by
    # one VERTEX_LOOP; the void reverses an outward sphere shell.
    s = Step("Original cylinder r=10 h=20 mm with a centred spherical cavity r=5 mm")
    outer = cylinder(s, 10, 0, 20)
    point = s.vertex((5, 0, 10))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 10), (0, 0, 1), (1, 0, 0))},5.)")
    ball = s.face([s.add(f"FACE_BOUND('',#{s.add(f'VERTEX_LOOP({chr(39)}{chr(39)},#{point})')},.T.)")], sphere)
    void = s.add(f"ORIENTED_CLOSED_SHELL('',*,#{s.add(f'CLOSED_SHELL({chr(39)}{chr(39)},(#{ball}))')},.F.)")
    s.write("brep-sphere-cavity.step", outer, root="BREP_WITH_VOIDS", voids=[void])

    # Negative: a cavity shell facing out of its cavity (not reversed).
    s = Step("Original rejected solid: a 20 mm cube whose cavity shell faces out of the cavity")
    outer = box(s, (0, 0, 0), (20, 20, 20))
    inner = s.add(f"CLOSED_SHELL('',({','.join(f'#{f}' for f in box(s, (5, 5, 5), (15, 15, 15)))}))")
    void = s.add(f"ORIENTED_CLOSED_SHELL('',*,#{inner},.T.)")
    s.write("brep-inverted-cavity.step", outer, root="BREP_WITH_VOIDS", voids=[void])

    # Negative: a VERTEX_LOOP at a regular point of a cone.
    s = Step("Original unsupported solid: vertex loop away from the cone apex")
    v0 = s.vertex((10, 0, 0))
    base = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    cone = s.add(f"CONICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, -1), (1, 0, 0))},10.,{num(math.pi / 4)})")
    vertex_loop = s.add(f"VERTEX_LOOP('',#{v0})")
    faces = [
        s.face([s.bound([(base, True)]), s.add(f"FACE_BOUND('',#{vertex_loop},.T.)")], cone),
        s.face([s.bound([(base, False)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-regular-vertex-loop.step", faces)

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

    # Solid cylinder r=10 h=20 mm whose side is a rational quadratic B-spline surface
    # (a nine-point circle swept along z) with U_CLOSED unknown (.U.). The surface is
    # closed, so its u axis gets a periodic chart and the annular side face an inserted
    # seam; a non-periodic chart would give every point of its closing line two chart
    # preimages and the edges there either one.
    s = Step("Original solid cylinder r=10 h=20 mm whose side is a closed B-spline surface flagged U_CLOSED .U.")
    v0 = s.vertex((10, 0, 0))
    v1 = s.vertex((10, 0, 20))
    bottom = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    top = s.edge(v1, v1, s.circle((0, 0, 20), (0, 0, 1), (1, 0, 0), 10))
    ring = [(10, 0), (10, 10), (0, 10), (-10, 10), (-10, 0), (-10, -10), (0, -10), (10, -10), (10, 0)]
    half = num(math.sqrt(0.5))
    columns = ["(" + ",".join(f"#{s.point((x, y, z))}" for z in (0, 20)) + ")" for x, y in ring]
    weights = ",".join(f"({w},{w})" for w in ("1." if i % 2 == 0 else half for i in range(9)))
    surface = s.add(f"(BOUNDED_SURFACE()B_SPLINE_SURFACE(2,1,({','.join(columns)}),.CYLINDRICAL_SURF.,.U.,.F.,.F.)"
                    "B_SPLINE_SURFACE_WITH_KNOTS((3,2,2,2,3),(2,2),(0.,0.25,0.5,0.75,1.),(0.,1.),.UNSPECIFIED.)"
                    f"GEOMETRIC_REPRESENTATION_ITEM()RATIONAL_B_SPLINE_SURFACE(({weights}))REPRESENTATION_ITEM('')SURFACE())")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], surface),
        s.face([s.bound([(bottom, False)], outer=True)], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(top, True)], outer=True)], s.plane((0, 0, 20), (0, 0, 1))),
    ]
    s.write("brep-unknown-closure-cylinder.step", faces)

    # Sphere R=10 below z=6 (a cap cut away), on a sphere whose STEP axis is x: in that
    # chart the rim does not wind around the axis and the face lies outside it, around
    # both poles. The importer plans it in a frame whose axis passes through the rim,
    # where it closes at the far pole (one re-chart, a pole seam, a collapsed edge).
    s = Step("Original sphere R=10 mm below z=6 mm on a planar top, with its STEP axis along x")
    v = s.vertex((8, 0, 6))
    rim = s.edge(v, v, s.circle((0, 0, 6), (0, 0, 1), (1, 0, 0), 8))
    sphere = s.add(f"SPHERICAL_SURFACE('',#{s.placement((0, 0, 0), (1, 0, 0), (0, 1, 0))},10.)")
    faces = [
        s.face([s.bound([(rim, False)])], sphere),
        s.face([s.bound([(rim, True)])], s.plane((0, 0, 6), (0, 0, 1))),
    ]
    s.write("brep-cut-sphere.step", faces)

    # Negative: a 20 x 20 x 10 mm box with a triangular frustum pocket in its top face
    # whose opening touches the top face's outer boundary at one vertex, C = (10, 0, 10)
    # on the front edge. The top face has one loop that passes through C twice (the
    # opening hangs off the outer boundary there); every vertex link is still one fan.
    # Trimming rejects the self-touching loop and names C.
    s = Step("Original rejected solid: box whose pocket opening touches the top face boundary at a vertex")
    points = {
        "b00": (0, 0, 0), "b20": (20, 0, 0), "b22": (20, 20, 0), "b02": (0, 20, 0),
        "t00": (0, 0, 10), "t20": (20, 0, 10), "t22": (20, 20, 10), "t02": (0, 20, 10),
        "C": (10, 0, 10), "P": (14, 6, 10), "Q": (6, 6, 10),
        # The floor is the opening scaled by 1/2 about its centroid (10, 4), so every
        # pocket wall is a planar trapezoid.
        "c": (10, 2, 5), "p": (12, 5, 5), "q": (8, 5, 5),
    }
    vertices = {name: s.vertex(p) for name, p in points.items()}
    edges = {}

    def use(a, b):
        if (b, a) in edges:
            return edges[(b, a)], False
        if (a, b) not in edges:
            pa, pb = points[a], points[b]
            edges[(a, b)] = s.edge(vertices[a], vertices[b], s.line(pa, [pb[k] - pa[k] for k in range(3)]))
        return edges[(a, b)], True

    def polygon(ring, normal):
        """Planar face over a ring of point names, counterclockwise about its outward
        normal (the ring is reversed when its Newell normal disagrees)."""
        p = [points[n] for n in ring]
        newell = [sum((p[i][(k + 1) % 3] - p[(i + 1) % len(p)][(k + 1) % 3]) *
                      (p[i][(k + 2) % 3] + p[(i + 1) % len(p)][(k + 2) % 3]) for i in range(len(p)))
                  for k in range(3)]
        if sum(newell[k] * normal[k] for k in range(3)) < 0:
            ring = ring[::-1]
            p = p[::-1]
        uses = [use(ring[i], ring[(i + 1) % len(ring)]) for i in range(len(ring))]
        d = [p[1][k] - p[0][k] for k in range(3)]
        length = math.sqrt(sum(x * x for x in d))
        size = math.sqrt(sum(x * x for x in normal))
        return s.face([s.bound(uses, outer=True)],
                      s.plane(p[0], [x / size for x in normal], [x / length for x in d]))

    faces = [
        polygon(["b00", "b02", "b22", "b20"], (0, 0, -1)),
        polygon(["t00", "C", "Q", "P", "C", "t20", "t22", "t02"], (0, 0, 1)),
        polygon(["b00", "b20", "t20", "C", "t00"], (0, -1, 0)),
        polygon(["b20", "b22", "t22", "t20"], (1, 0, 0)),
        polygon(["b22", "b02", "t02", "t22"], (0, 1, 0)),
        polygon(["b02", "b00", "t00", "t02"], (-1, 0, 0)),
        polygon(["c", "p", "q"], (0, 0, 1)),
    ]
    # Pocket walls face into the pocket (towards the opening's centroid axis).
    for a, b, fa, fb in (("C", "P", "c", "p"), ("P", "Q", "p", "q"), ("Q", "C", "q", "c")):
        pa, pb, qa = points[a], points[b], points[fa]
        d = [pb[k] - pa[k] for k in range(3)]
        w = [qa[k] - pa[k] for k in range(3)]
        n = [d[1] * w[2] - d[2] * w[1], d[2] * w[0] - d[0] * w[2], d[0] * w[1] - d[1] * w[0]]
        centre = (10, 4, 7.5)
        if sum(n[k] * (centre[k] - pa[k]) for k in range(3)) < 0:
            n = [-x for x in n]
        faces.append(polygon([a, b, fb, fa], n))
    s.write("brep-pinched-pocket.step", faces)

    # Loop healing for exporter parity (Milestone 22.4 follow-up).
    # Washer R=20 r=10 t=5 whose top face writes FACE_OUTER_BOUND on the bore circle
    # and FACE_BOUND on the rim. Both loops keep their geometric senses (rim
    # counterclockwise, bore clockwise about the face normal), so only the label is
    # on the wrong loop: the default import bounds the face by the rim (a counted
    # relabelled outer bound); strict import keeps the label and tessellation rejects
    # the clockwise outer bound.
    s = Step("Original washer R=20 r=10 t=5 mm whose top face labels the bore circle FACE_OUTER_BOUND")
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
        s.face([s.bound([(ib, False)]), s.bound([(it, True)])], inner, same=False),
        s.face([s.bound([(ob, False)], outer=True), s.bound([(ib, True)])], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(it, False)], outer=True), s.bound([(ot, True)])], s.plane((0, 0, 5), (0, 0, 1))),
    ]
    s.write("brep-hole-outer-bound.step", faces)

    # Cone R=10 h=10 (apex (0, 0, 10)) split into two half faces by its generators
    # through A = (10, 0, 0) and B = (-10, 0, 0), under a 1 um uncertainty. A's
    # generator LINE is written with its origin 5 km down the line beyond the base and
    # passes 10 nm from the apex on the axis side (B's runs from B through the apex).
    # Evaluating A's line at the apex cancels kilometre-sized terms, so its exact chart
    # image ends a rounding error below the apex (v < 0), outside the cone's domain;
    # a fitted image instead takes its apex coordinate from a point 1.4 nm along the
    # line, nearer the 10 nm miss than the generator, and joins B's generator there
    # with a full turn.
    s = Step("Original cone R=10 h=10 mm in two half faces; one generator line has its origin 5 km away and misses the apex by 10 nm")
    apex = s.vertex((0, 0, 10))
    va, vb = s.vertex((10, 0, 0)), s.vertex((-10, 0, 0))
    front = s.edge(va, vb, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    back = s.edge(vb, va, s.circle((0, 0, 0), (0, 0, 1), (-1, 0, 0), 10))
    far, miss = 5e6, 1e-5
    root = 1 / math.sqrt(2)
    # A's generator runs from the apex outward (against the edge); B's inward.
    ga = s.edge(va, apex, s.line((10 + (far - miss) * root, 0, -(far + miss) * root), (root, 0, -root)), same=False)
    gb = s.edge(vb, apex, s.line((-10, 0, 0), (root, 0, root)))
    cone = s.add(f"CONICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, -1), (1, 0, 0))},10.,{num(math.pi / 4)})")
    faces = [
        s.face([s.bound([(front, True), (gb, True), (ga, False)], outer=True)], cone),
        s.face([s.bound([(back, True), (ga, True), (gb, False)], outer=True)], cone),
        s.face([s.bound([(front, False), (back, False)], outer=True)], s.plane((0, 0, 0), (0, 0, 1)), same=False),
    ]
    s.write("brep-far-generator.step", faces, context=context(uncertainty="1.E-3"))

    # Cylinder r=10 h=20 under a 2.7 um uncertainty whose top edge is one closed
    # cubic B-spline of four standard quarter-circle Bezier arcs (kappa = 4/3 (sqrt 2
    # - 1)). Each arc bulges outward by up to 2.725 um, at 21% and 79% of its span,
    # but by at most 2.668 um at the sixteenths of its span where the pcurve fit
    # samples it. The importer must find the peak between those samples to count the
    # edge as off its side surface; tessellation would otherwise meet it against the
    # unwidened tolerance.
    s = Step("Original cylinder r=10 h=20 mm whose top edge is four Bezier quarter arcs, 2.7 um off the side at their peaks")
    v0, v1 = s.vertex((10, 0, 0)), s.vertex((10, 0, 20))
    bottom = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    k = 10 * 4 / 3 * (math.sqrt(2) - 1)
    controls = []
    arc = [(10, 0), (10, k), (k, 10), (0, 10)]
    for quarter in range(4):
        controls.extend((x, y, 20) for x, y in (arc if quarter == 0 else arc[1:]))
        arc = [(-y, x) for x, y in arc]  # a quarter turn, exactly
    curve = s.add(f"B_SPLINE_CURVE_WITH_KNOTS('',3,({','.join(f'#{s.point(p)}' for p in controls)}),"
                  ".UNSPECIFIED.,.T.,.F.,(4,3,3,3,4),(0.,1.,2.,3.,4.),.UNSPECIFIED.)")
    top = s.edge(v1, v1, curve)
    side = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(top, False)])], side),
        s.face([s.bound([(bottom, False)], outer=True)], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(top, True)], outer=True)], s.plane((0, 0, 20), (0, 0, 1))),
    ]
    s.write("brep-sampled-peak.step", faces, context=context(uncertainty="2.7E-3"))

    # A 10 x 10 x 2 mm plate with three prismatic through holes: a 2 x 2 square in
    # the middle, listed first on both plate faces, between an L along the left and
    # bottom sides and an L along the right and top sides. No corner of a plate face
    # is visible from the square past the Ls, so bridged in face order it has no
    # bridge into the face polygon; the triangulator must bridge the Ls first. The
    # top face writes every loop as FACE_BOUND (its outer bound is inferred), as the
    # exporter of the customer file this reproduces does.
    s = Step("Original 10 x 10 x 2 mm plate with a square through hole enclosed by two L-shaped through holes listed after it")
    rings = {
        "o": [(0, 0), (10, 0), (10, 10), (0, 10)],
        "s": [(4, 4), (6, 4), (6, 6), (4, 6)],
        "l": [(1, 1), (9, 1), (9, 2), (2, 2), (2, 7), (1, 7)],
        "u": [(9, 9), (1, 9), (1, 8), (8, 8), (8, 3), (9, 3)],
    }
    points = {f"{name}{i}{z}": (x, y, 2 if z == "t" else 0)
              for name, ring in rings.items() for i, (x, y) in enumerate(ring) for z in "bt"}
    vertices = {name: s.vertex(p) for name, p in points.items()}
    edges = {}

    def use(a, b):
        if (b, a) in edges:
            return edges[(b, a)], False
        if (a, b) not in edges:
            pa, pb = points[a], points[b]
            edges[(a, b)] = s.edge(vertices[a], vertices[b], s.line(pa, [pb[k] - pa[k] for k in range(3)]))
        return edges[(a, b)], True

    def loop(names, normal, hole=False):
        """Edge uses around the point names, counterclockwise about `normal` (clockwise
        for a hole); the names are reversed when their Newell normal disagrees."""
        p = [points[n] for n in names]
        newell = [sum((p[i][(k + 1) % 3] - p[(i + 1) % len(p)][(k + 1) % 3]) *
                      (p[i][(k + 2) % 3] + p[(i + 1) % len(p)][(k + 2) % 3]) for i in range(len(p)))
                  for k in range(3)]
        if (sum(newell[k] * normal[k] for k in range(3)) < 0) != hole:
            names = names[::-1]
        return [use(names[i], names[(i + 1) % len(names)]) for i in range(len(names))]

    def face(loops, normal, labelled=True):
        """Planar face bounded by the first loop with the others as holes; `labelled`
        writes FACE_OUTER_BOUND on the first, else every loop is a FACE_BOUND."""
        p0, p1 = points[loops[0][0]], points[loops[0][1]]
        d = [p1[k] - p0[k] for k in range(3)]
        length = math.sqrt(sum(x * x for x in d))
        bounds = [s.bound(loop(loops[0], normal), outer=labelled)]
        bounds += [s.bound(loop(names, normal, hole=True)) for names in loops[1:]]
        return s.face(bounds, s.plane(p0, normal, [x / length for x in d]))

    faces = [
        face([[f"{name}{i}t" for i in range(len(ring))] for name, ring in rings.items()], (0, 0, 1), labelled=False),
        face([[f"{name}{i}b" for i in range(len(ring))] for name, ring in rings.items()], (0, 0, -1)),
    ]
    # Walls face away from the material: out of the plate, into each hole.
    for name, ring in rings.items():
        for i in range(len(ring)):
            (x0, y0), (x1, y1) = ring[i], ring[(i + 1) % len(ring)]
            d = (x1 - x0, y1 - y0)
            length = math.hypot(*d)
            normal = (d[1] / length, -d[0] / length, 0) if name == "o" else (-d[1] / length, d[0] / length, 0)
            j = (i + 1) % len(ring)
            faces.append(face([[f"{name}{i}b", f"{name}{j}b", f"{name}{j}t", f"{name}{i}t"]], normal))
    s.write("brep-enclosed-hole.step", faces)

    # Slab 10 x 10 x 5 mm with two fins 2 x 3 x 3 mm standing flush with its wall
    # x = 0, over y in [7, 10] and over y in [0, 3]. The wall's top edge A-B spans the
    # whole wall while each fin's base edge spans only its part, so the slab top
    # between the fins runs out to B along the wall edge and back along the first
    # fin's base edge, and in along the second fin's base edge and out along the wall
    # edge at A: two loop spurs of no area. The first fin's base corner C sits 5 nm
    # off the wall edge, within the 10 nm uncertainty; the second's is exactly on it.
    # Volume 500 + 18 + 18 mm^3.
    s = Step("Original slab 10x10x5 mm with two 2x3x3 fins flush with one wall; the slab top runs out and back along the wall edge at both fins")
    points = [(0, 0, 0), (0, 10, 0), (-5e-6, 7, 0), (-2, 7, 0), (-10, 7, 0), (-10, 3, 0), (-2, 3, 0), (0, 3, 0),
              (-10, 0, 0), (-10, 10, 0), (-2, 10, 0), (-2, 0, 0),
              (0, 7, 3), (0, 10, 3), (-2, 10, 3), (-2, 7, 3),
              (0, 0, 3), (0, 3, 3), (-2, 3, 3), (-2, 0, 3),
              (0, 0, -5), (0, 10, -5), (-10, 10, -5), (-10, 0, -5)]
    faces = polyhedron(s, points, [
        ((7, 0, 1, 2, 3, 4, 5, 6), (0, 0, 1), (1, 0, 0)),  # slab top between the fins, with both spurs
        ((3, 10, 9, 4), (0, 0, 1), (1, 0, 0)),  # slab top beside the first fin
        ((6, 5, 8, 11), (0, 0, 1), (1, 0, 0)),  # slab top beside the second fin
        ((20, 21, 1, 0), (1, 0, 0), (0, 1, 0)),  # wall x = 0: its top edge A-B spans both fins
        ((2, 1, 13, 12), (1, 0, 0), (0, 1, 0)),  # first fin front, on the base edge B-C
        ((0, 7, 17, 16), (1, 0, 0), (0, 1, 0)),  # second fin front, on the base edge A-C''
        ((20, 23, 22, 21), (0, 0, -1), (0, 1, 0)),  # slab bottom
        ((8, 5, 4, 9, 22, 23), (-1, 0, 0), (0, 0, 1)),  # wall x = -10
        ((0, 11, 8, 23, 20), (0, -1, 0), (1, 0, 0)),  # wall y = 0
        ((21, 22, 9, 10, 1), (0, 1, 0), (0, 0, 1)),  # wall y = 10
        ((3, 15, 14, 10), (-1, 0, 0), (0, 0, 1)),  # first fin back
        ((2, 12, 15, 3), (0, -1, 0), (1, 0, 0)),  # first fin side y = 7
        ((1, 10, 14, 13), (0, 1, 0), (0, 0, 1)),  # first fin side y = 10
        ((12, 13, 14, 15), (0, 0, 1), (1, 0, 0)),  # first fin top
        ((11, 19, 18, 6), (-1, 0, 0), (0, 0, 1)),  # second fin back
        ((0, 16, 19, 11), (0, -1, 0), (1, 0, 0)),  # second fin side y = 0
        ((7, 6, 18, 17), (0, 1, 0), (0, 0, 1)),  # second fin side y = 3
        ((16, 17, 18, 19), (0, 0, 1), (1, 0, 0)),  # second fin top
    ])
    s.write("brep-loop-spurs.step", faces)

    # Mesh loop healing: a 10 mm box whose top face also covers a 1 mm strip beyond
    # the front edge (its loop detours around the strip), with a second planar face
    # under the strip facing down: a flap of no thickness, as an exporter wrote a
    # plane face over the strip of a fillet cylinder it cuts 0.03 um below its
    # tangent line. The strip face lies on the top face's plane facing away from it,
    # so it is merged into the top face (one collapsed face); the box stays 1000 mm3.
    s = Step("Original 10 mm box with a zero-thickness planar flap folded under its top face beyond the front edge")
    corners = {}
    for i in range(8):
        p = tuple(10 if (i >> k) & 1 else 0 for k in range(3))
        corners[i] = (p, s.vertex(p))
    edges = {}

    def use(a, b):
        if (b, a) in edges:
            return edges[(b, a)], False
        if (a, b) not in edges:
            pa, pb = corners[a][0], corners[b][0]
            edges[(a, b)] = s.edge(corners[a][1], corners[b][1], s.line(pa, [pb[k] - pa[k] for k in range(3)]))
        return edges[(a, b)], True

    faces = []
    for ring, normal, ref in [
        ((0, 2, 3, 1), (0, 0, -1), (0, 1, 0)), ((0, 1, 5, 4), (0, -1, 0), (1, 0, 0)),
        ((2, 6, 7, 3), (0, 1, 0), (0, 0, 1)), ((0, 4, 6, 2), (-1, 0, 0), (0, 0, 1)),
        ((1, 3, 7, 5), (1, 0, 0), (0, 1, 0)),
    ]:
        uses = [use(ring[i], ring[(i + 1) % 4]) for i in range(4)]
        faces.append(s.face([s.bound(uses, outer=True)], s.plane(corners[ring[0]][0], normal, ref)))
    # The top face (4, 5, 7, 6) with the front edge 4 -> 5 replaced by the detour
    # 4 -> 4' -> 5' -> 5 around the strip y in [-1, 0]; the strip face runs 4 -> 5
    # along the front face's top edge and back along the detour.
    a2, b2 = (0, -1, 10), (10, -1, 10)
    va2, vb2 = s.vertex(a2), s.vertex(b2)
    e1 = s.edge(corners[4][1], va2, s.line(corners[4][0], (0, -1, 0)))
    e2 = s.edge(va2, vb2, s.line(a2, (1, 0, 0)))
    e3 = s.edge(vb2, corners[5][1], s.line(b2, (0, 1, 0)))
    top = [(e1, True), (e2, True), (e3, True), use(5, 7), use(7, 6), use(6, 4)]
    faces.append(s.face([s.bound(top, outer=True)], s.plane(corners[4][0], (0, 0, 1), (1, 0, 0))))
    strip = [use(4, 5), (e3, False), (e2, False), (e1, False)]
    faces.append(s.face([s.bound(strip, outer=True)], s.plane(corners[4][0], (0, 0, -1), (1, 0, 0))))
    s.write("brep-lens-flap.step", faces, context=context())

    # A cylinder r=10 h=20 under a 1 um uncertainty whose top circle is two half
    # arcs between V1 = (10.003, 0, 20), 3 um outside the circle, and V2 = (-10, 0,
    # 20); the cap is bounded by a second arc 3 um inside the first at its apex
    # (centre (0, -0.003, 20)), and a crescent face fills the gap between the arcs.
    # The crescent's arcs coincide only within the working tolerance the vertex
    # implies (7 um), so it is a sliver face only there: collapsed, the cap uses the
    # outer arc and the mesh is the full cylinder.
    s = Step("Original cylinder r=10 h=20 mm under a 1 um uncertainty with a 3 um crescent sliver on its cap beside a vertex 3 um off its arcs")
    v0 = s.vertex((10, 0, 0))
    v1, v2 = s.vertex((10.003, 0, 20)), s.vertex((-10, 0, 20))
    bottom = s.edge(v0, v0, s.circle((0, 0, 0), (0, 0, 1), (1, 0, 0), 10))
    outer = s.circle((0, 0, 20), (0, 0, 1), (1, 0, 0), 10)
    arc1 = s.edge(v1, v2, outer)
    arc2 = s.edge(v2, v1, outer)
    inner = s.edge(v1, v2, s.circle((0, -0.003, 20), (0, 0, 1), (1, 0, 0), math.sqrt(100 + 0.003 ** 2)))
    surface = s.add(f"CYLINDRICAL_SURFACE('',#{s.placement((0, 0, 0), (0, 0, 1), (1, 0, 0))},10.)")
    faces = [
        s.face([s.bound([(bottom, True)]), s.bound([(arc2, False), (arc1, False)])], surface),
        s.face([s.bound([(bottom, False)], outer=True)], s.plane((0, 0, 0), (0, 0, 1)), same=False),
        s.face([s.bound([(inner, True), (arc2, True)], outer=True)], s.plane((0, 0, 20), (0, 0, 1))),
        s.face([s.bound([(arc1, True), (inner, False)], outer=True)], s.plane((0, 0, 20), (0, 0, 1))),
    ]
    s.write("brep-crescent-sliver.step", faces, context=context(uncertainty="1.E-3"))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=HERE, help="Directory for the generated fixtures")
    OUTPUT = parser.parse_args().output
    fixtures()
