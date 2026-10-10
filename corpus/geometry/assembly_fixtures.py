#!/usr/bin/env python3
"""Generate the original assembly fixtures (AP214 physical layout, authored).

assembly-nested.step: product `rig` uses `pair` twice (identity, and a quarter turn
about z then 100 mm along x); `pair` uses the 10 x 20 x 30 mm faceted `block` twice
(identity, and 40 mm along y). Every occurrence is placed by a context dependent
shape representation over an item defined transformation. Four block instances.

assembly-missing-placement.step: as nested, without the placement of the second
block occurrence in `pair`.

assembly-unplaced.step: one placed block, and a second FACETED_BREP in a shape
representation that no product definition uses.

assembly-styled.step: one block styled with a translucent RGB fill and rendering
colour on the solid, a pre-defined blue over-riding style on its top face, a curve
style on a point (no surface colour) and a presentation layer assignment.

assembly-inch.step: `pair` alone in inch units written as a conversion-based unit
with `NAMED_UNIT(*)`, a deviation the tolerant default accepts and strict rejects.

assembly-mapped.step: the nested structure with every occurrence placed by a
MAPPED_ITEM, as CATIA- and Creo-style exporters write it: the parent's shape
representation holds the mapped item, and the occurrence's product definition shape
has its own shape representation holding the same item. `pair` is mapped from its
second block frame (40 mm along y), so the target-times-inverse-origin rule moves it.

assembly-operator.step: `rig` places `block` three times by 3D Cartesian
transformation operators: a mirror in y and a uniform scale 2 at 100 mm along x as
context dependent placements, and a uniform scale 0.5 at 100 mm along y as the target
of a mapped item.

assembly-si-dimensions.step: `pair` in millimetres whose SI units write explicit
DIMENSIONAL_EXPONENTS and, for the length and solid angle units, complex components
out of canonical order (an XStep-style encoding the tolerant default accepts).

assembly-untyped-measure.step: `pair` in inches whose conversion factor and length
uncertainty are bare numbers where a typed LENGTH_MEASURE is required.

assembly-reversed-items.step: the nested structure in the layout HOOPS Exchange writes
(seen in Shapr3D exports): every product has its own geometric context, a shape
representation holding one identity frame shared by all representations, its solid in
a related B-rep representation and a draughting model mapping it; each occurrence's
item defined transformation lists the parent's placement first and the shared frame
second, the reverse of its relationship's representations. World boxes as nested.
"""
import argparse
from pathlib import Path

HEADER = """ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('{description}'),'2;1');
FILE_NAME('{name}','2026-09-27',('TessSTEP'),('TessSTEP'),'authored fixture','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));
ENDSEC;
DATA;
"""


class Writer:
    def __init__(self):
        self.lines = []
        self.next = 1

    def add(self, text):
        n = self.next
        self.next += 1
        self.lines.append(f"#{n}={text};")
        return n

    def point(self, xyz):
        return self.add("CARTESIAN_POINT('',(" + ",".join(f"{v:.1f}" for v in xyz) + "))")

    def direction(self, xyz):
        return self.add("DIRECTION('',(" + ",".join(f"{v:.1f}" for v in xyz) + "))")

    def frame(self, origin, z=(0, 0, 1), x=(1, 0, 0)):
        p = self.point(origin)
        return self.add(f"AXIS2_PLACEMENT_3D('',#{p},#{self.direction(z)},#{self.direction(x)})")


def context(w, inch=False, encoding=None):
    """Product contexts and a geometric context in millimetres (or inches). With
    encoding "explicit" the SI units write their dimensional exponents, the length and
    solid angle units out of component order; with "untyped" the conversion factor
    and uncertainty are bare numbers."""
    app = w.add("APPLICATION_CONTEXT('automotive design')")
    product_context = w.add(f"PRODUCT_CONTEXT('',#{app},'mechanical')")
    definition_context = w.add(f"PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')")
    measure = "{}" if encoding == "untyped" else "LENGTH_MEASURE({})"
    if inch:
        mm = w.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))")
        factor = w.add(f"LENGTH_MEASURE_WITH_UNIT({measure.format('25.4')},#{mm})")
        dimensions = "*"
        if encoding == "untyped":
            dimensions = f"#{w.add('DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.)')}"
        length = w.add(f"(CONVERSION_BASED_UNIT('INCH',#{factor})LENGTH_UNIT()NAMED_UNIT({dimensions}))")
    elif encoding == "explicit":
        metre = w.add("DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.)")
        length = w.add(f"(NAMED_UNIT(#{metre})LENGTH_UNIT()SI_UNIT(.MILLI.,.METRE.))")
    else:
        length = w.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))")
    if encoding == "explicit":
        none = w.add("DIMENSIONAL_EXPONENTS(0.,0.,0.,0.,0.,0.,0.)")
        angle = w.add(f"(NAMED_UNIT(#{none})PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))")
        solid = w.add(f"(NAMED_UNIT(#{none})SOLID_ANGLE_UNIT()SI_UNIT($,.STERADIAN.))")
    else:
        angle = w.add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))")
        solid = w.add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())")
    uncertainty = w.add(f"UNCERTAINTY_MEASURE_WITH_UNIT({measure.format('1.E-05')},#{length},'distance_accuracy_value','')")
    geometric = w.add(f"(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty}))"
                      f"GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid}))REPRESENTATION_CONTEXT('',''))")
    return product_context, definition_context, geometric


def block(w, geometric, offset=(0, 0, 0)):
    """A 10 x 20 x 30 faceted box and its shape representation; returns (rep, origin, brep)."""
    corners = [(0, 0, 0), (10, 0, 0), (10, 20, 0), (0, 20, 0), (0, 0, 30), (10, 0, 30), (10, 20, 30), (0, 20, 30)]
    p = [w.point([c[k] + offset[k] for k in range(3)]) for c in corners]
    faces = []
    for loop, anchor, z, x in [((0, 3, 2, 1), 0, (0, 0, -1), (1, 0, 0)), ((4, 5, 6, 7), 4, (0, 0, 1), (1, 0, 0)),
                               ((0, 1, 5, 4), 0, (0, -1, 0), (1, 0, 0)), ((3, 7, 6, 2), 3, (0, 1, 0), (1, 0, 0)),
                               ((0, 4, 7, 3), 0, (-1, 0, 0), (0, 1, 0)), ((1, 2, 6, 5), 1, (1, 0, 0), (0, 1, 0))]:
        poly = w.add("POLY_LOOP('',(" + ",".join(f"#{p[i]}" for i in loop) + "))")
        bound = w.add(f"FACE_OUTER_BOUND('',#{poly},.T.)")
        axis, reference = w.direction(z), w.direction(x)
        placement = w.add(f"AXIS2_PLACEMENT_3D('',#{p[anchor]},#{axis},#{reference})")
        plane = w.add(f"PLANE('',#{placement})")
        faces.append(w.add(f"FACE_SURFACE('',(#{bound}),#{plane},.T.)"))
    w.faces = faces
    shell = w.add("CLOSED_SHELL('',(" + ",".join(f"#{f}" for f in faces) + "))")
    brep = w.add(f"FACETED_BREP('block',#{shell})")
    origin = w.frame((0, 0, 0))
    rep = w.add(f"FACETED_BREP_SHAPE_REPRESENTATION('block',(#{brep},#{origin}),#{geometric})")
    return rep, origin, brep


def product(w, name, product_context, definition_context, rep):
    p = w.add(f"PRODUCT('{name}','{name}','',(#{product_context}))")
    f = w.add(f"PRODUCT_DEFINITION_FORMATION('','',#{p})")
    d = w.add(f"PRODUCT_DEFINITION('design','',#{f},#{definition_context})")
    shape = w.add(f"PRODUCT_DEFINITION_SHAPE('','',#{d})")
    w.add(f"SHAPE_DEFINITION_REPRESENTATION(#{shape},#{rep})")
    return d


def occurrence(w, ident, parent, child, child_rep, child_origin, parent_rep, parent_frame, placed=True):
    nauo = w.add(f"NEXT_ASSEMBLY_USAGE_OCCURRENCE('{ident}','{ident}','',#{parent},#{child},$)")
    shape = w.add(f"PRODUCT_DEFINITION_SHAPE('','',#{nauo})")
    if placed:
        idt = w.add(f"ITEM_DEFINED_TRANSFORMATION('','',#{child_origin},#{parent_frame})")
        rel = w.add(f"(REPRESENTATION_RELATIONSHIP('','',#{child_rep},#{parent_rep})"
                    f"REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#{idt})SHAPE_REPRESENTATION_RELATIONSHIP())")
        w.add(f"CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#{rel},#{shape})")


def pair(w, product_context, definition_context, geometric, placed=True):
    """`pair` with two `block` occurrences; returns (definition, rep, origin)."""
    block_rep, block_origin, _ = block(w, geometric)
    block_definition = product(w, "block", product_context, definition_context, block_rep)
    origin = w.frame((0, 0, 0))
    first = w.frame((0, 0, 0))
    second = w.frame((0, 40, 0))
    rep = w.add(f"SHAPE_REPRESENTATION('pair',(#{origin},#{first},#{second}),#{geometric})")
    definition = product(w, "pair", product_context, definition_context, rep)
    occurrence(w, "block-1", definition, block_definition, block_rep, block_origin, rep, first)
    occurrence(w, "block-2", definition, block_definition, block_rep, block_origin, rep, second, placed)
    return definition, rep, origin


def nested(placed=True):
    w = Writer()
    product_context, definition_context, geometric = context(w)
    pair_definition, pair_rep, pair_origin = pair(w, product_context, definition_context, geometric, placed)
    origin = w.frame((0, 0, 0))
    first = w.frame((0, 0, 0))
    turned = w.frame((100, 0, 0), z=(0, 0, 1), x=(0, 1, 0))
    rep = w.add(f"SHAPE_REPRESENTATION('rig',(#{origin},#{first},#{turned}),#{geometric})")
    rig = product(w, "rig", product_context, definition_context, rep)
    occurrence(w, "pair-1", rig, pair_definition, pair_rep, pair_origin, rep, first)
    occurrence(w, "pair-2", rig, pair_definition, pair_rep, pair_origin, rep, turned)
    return w


def unplaced():
    w = Writer()
    product_context, definition_context, geometric = context(w)
    rep, _, _ = block(w, geometric)
    product(w, "block", product_context, definition_context, rep)
    block(w, geometric, offset=(50, 0, 0))
    return w


def surface_style(w, colour, transparency=None):
    fill = w.add(f"FILL_AREA_STYLE_COLOUR('',#{colour})")
    area = w.add(f"FILL_AREA_STYLE('',(#{fill}))")
    elements = [w.add(f"SURFACE_STYLE_FILL_AREA(#{area})")]
    if transparency is not None:
        transparent = w.add(f"SURFACE_STYLE_TRANSPARENT({transparency})")
        elements.append(w.add(f"SURFACE_STYLE_RENDERING_WITH_PROPERTIES(.NORMAL_SHADING.,#{colour},(#{transparent}))"))
    side = w.add("SURFACE_SIDE_STYLE('',(" + ",".join(f"#{e}" for e in elements) + "))")
    usage = w.add(f"SURFACE_STYLE_USAGE(.BOTH.,#{side})")
    return w.add(f"PRESENTATION_STYLE_ASSIGNMENT((#{usage}))")


def styled():
    w = Writer()
    product_context, definition_context, geometric = context(w)
    rep, _, brep = block(w, geometric)
    top = w.faces[1]
    product(w, "block", product_context, definition_context, rep)
    red = w.add("COLOUR_RGB('',0.8,0.2,0.2)")
    solid = w.add(f"STYLED_ITEM('color',(#{surface_style(w, red, 0.25)}),#{brep})")
    blue = w.add("DRAUGHTING_PRE_DEFINED_COLOUR('blue')")
    face = w.add(f"OVER_RIDING_STYLED_ITEM('color',(#{surface_style(w, blue)}),#{top},#{solid})")
    font = w.add("DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous')")
    curve = w.add(f"CURVE_STYLE('',#{font},POSITIVE_LENGTH_MEASURE(0.1),#{blue})")
    marker = w.point((5, 5, 5))
    point = w.add(f"STYLED_ITEM('',(#{w.add(f'PRESENTATION_STYLE_ASSIGNMENT((#{curve}))')}),#{marker})")
    w.add(f"MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',(#{solid},#{face},#{point}),#{geometric})")
    w.add(f"PRESENTATION_LAYER_ASSIGNMENT('1','',(#{brep}))")
    return w


def inch():
    w = Writer()
    product_context, definition_context, geometric = context(w, inch=True)
    pair(w, product_context, definition_context, geometric)
    return w


def mapped_item(w, child_rep, child_origin, target):
    """A mapped item of the child's shape representation; target is a frame or operator."""
    source = w.add(f"REPRESENTATION_MAP(#{child_origin},#{child_rep})")
    return w.add(f"MAPPED_ITEM('',#{source},#{target})")


def mapped_occurrence(w, ident, parent, child, item, geometric):
    """An occurrence whose own shape representation holds the mapped item placing it."""
    nauo = w.add(f"NEXT_ASSEMBLY_USAGE_OCCURRENCE('{ident}','{ident}','',#{parent},#{child},$)")
    shape = w.add(f"PRODUCT_DEFINITION_SHAPE('','',#{nauo})")
    rep = w.add(f"SHAPE_REPRESENTATION('{ident}',(#{item}),#{geometric})")
    w.add(f"SHAPE_DEFINITION_REPRESENTATION(#{shape},#{rep})")


def mapped():
    w = Writer()
    product_context, definition_context, geometric = context(w)
    block_rep, block_origin, _ = block(w, geometric)
    block_definition = product(w, "block", product_context, definition_context, block_rep)
    first, second = w.frame((0, 0, 0)), w.frame((0, 40, 0))
    items = [mapped_item(w, block_rep, block_origin, first), mapped_item(w, block_rep, block_origin, second)]
    pair_rep = w.add(f"SHAPE_REPRESENTATION('pair',(#{first},#{second},#{items[0]},#{items[1]}),#{geometric})")
    pair_definition = product(w, "pair", product_context, definition_context, pair_rep)
    for k, item in enumerate(items):
        mapped_occurrence(w, f"block-{k + 1}", pair_definition, block_definition, item, geometric)
    first, turned = w.frame((0, 0, 0)), w.frame((100, 0, 0), z=(0, 0, 1), x=(0, 1, 0))
    items = [mapped_item(w, pair_rep, second, first), mapped_item(w, pair_rep, second, turned)]
    rig_rep = w.add(f"SHAPE_REPRESENTATION('rig',(#{first},#{turned},#{items[0]},#{items[1]}),#{geometric})")
    rig = product(w, "rig", product_context, definition_context, rig_rep)
    for k, item in enumerate(items):
        mapped_occurrence(w, f"pair-{k + 1}", rig, pair_definition, item, geometric)
    return w


def operator(w, origin, axis1=None, axis2=None, axis3=None, scale=None):
    """A CARTESIAN_TRANSFORMATION_OPERATOR_3D in its physical layout: the item name, the
    functionally defined transformation's name and description, axis1, axis2, local
    origin, scale and axis3."""
    axes = ["$" if a is None else f"#{w.direction(a)}" for a in (axis1, axis2, axis3)]
    p = w.point(origin)
    s = "$" if scale is None else f"{scale:.1f}"
    return w.add(f"CARTESIAN_TRANSFORMATION_OPERATOR_3D('','',$,{axes[0]},{axes[1]},#{p},{s},{axes[2]})")


def operators():
    w = Writer()
    product_context, definition_context, geometric = context(w)
    block_rep, block_origin, _ = block(w, geometric)
    block_definition = product(w, "block", product_context, definition_context, block_rep)
    origin = w.frame((0, 0, 0))
    half = operator(w, (0, 100, 0), scale=0.5)
    item = mapped_item(w, block_rep, block_origin, half)
    rep = w.add(f"SHAPE_REPRESENTATION('rig',(#{origin},#{item}),#{geometric})")
    rig = product(w, "rig", product_context, definition_context, rep)
    for ident, op in [("mirror", operator(w, (0, 0, 0), (1, 0, 0), (0, -1, 0), (0, 0, 1))),
                      ("double", operator(w, (100, 0, 0), scale=2.0))]:
        nauo = w.add(f"NEXT_ASSEMBLY_USAGE_OCCURRENCE('{ident}','{ident}','',#{rig},#{block_definition},$)")
        shape = w.add(f"PRODUCT_DEFINITION_SHAPE('','',#{nauo})")
        rel = w.add(f"(REPRESENTATION_RELATIONSHIP('','',#{block_rep},#{rep})"
                    f"REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#{op})SHAPE_REPRESENTATION_RELATIONSHIP())")
        w.add(f"CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#{rel},#{shape})")
    mapped_occurrence(w, "half", rig, block_definition, item, geometric)
    return w


def unit_context(w, name):
    """A millimetre geometric context of its own, as HOOPS Exchange writes one per product."""
    length = w.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))")
    angle = w.add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))")
    solid = w.add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())")
    uncertainty = w.add(f"UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-03),#{length},'DISTANCE_ACCURACY_VALUE','')")
    return w.add(f"(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty}))"
                 f"GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid}))REPRESENTATION_CONTEXT('','{name}'))")


def presented(w, rep, geometric):
    """The draughting model holding a mapped item of a shape representation."""
    origin = w.frame((0, 0, 0))
    w.add(f"DRAUGHTING_MODEL('',(#{mapped_item(w, rep, origin, origin)}),#{geometric})")


def reversed_occurrence(w, ident, parent, child, child_rep, parent_rep, parent_frame, shared):
    """An occurrence placed as HOOPS Exchange writes it: rep_1 is the child, but the item
    defined transformation lists the parent's placement first and the shared frame second."""
    nauo = w.add(f"NEXT_ASSEMBLY_USAGE_OCCURRENCE('{ident}','{ident}','',#{parent},#{child},$)")
    shape = w.add(f"PRODUCT_DEFINITION_SHAPE('','',#{nauo})")
    idt = w.add(f"ITEM_DEFINED_TRANSFORMATION('','',#{parent_frame},#{shared})")
    rel = w.add(f"(REPRESENTATION_RELATIONSHIP('','',#{child_rep},#{parent_rep})"
                f"REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#{idt})SHAPE_REPRESENTATION_RELATIONSHIP())")
    w.add(f"CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#{rel},#{shape})")


def reversed_items():
    w = Writer()
    product_context, definition_context, rig_context = context(w)
    # One identity frame is an item of every shape representation, in every context.
    shared = w.frame((0, 0, 0))
    block_context = unit_context(w, "block")
    brep_rep, _, _ = block(w, block_context)
    block_rep = w.add(f"SHAPE_REPRESENTATION('block',(#{shared}),#{block_context})")
    w.add(f"SHAPE_REPRESENTATION_RELATIONSHIP('','',#{block_rep},#{brep_rep})")
    presented(w, block_rep, block_context)
    block_definition = product(w, "block", product_context, definition_context, block_rep)
    pair_context = unit_context(w, "pair")
    first, second = w.frame((0, 0, 0)), w.frame((0, 40, 0))
    pair_rep = w.add(f"SHAPE_REPRESENTATION('pair',(#{shared},#{first},#{second}),#{pair_context})")
    presented(w, pair_rep, pair_context)
    pair_definition = product(w, "pair", product_context, definition_context, pair_rep)
    for ident, frame in (("block-1", first), ("block-2", second)):
        reversed_occurrence(w, ident, pair_definition, block_definition, block_rep, pair_rep, frame, shared)
    first, turned = w.frame((0, 0, 0)), w.frame((100, 0, 0), z=(0, 0, 1), x=(0, 1, 0))
    rig_rep = w.add(f"SHAPE_REPRESENTATION('rig',(#{shared},#{first},#{turned}),#{rig_context})")
    presented(w, rig_rep, rig_context)
    rig = product(w, "rig", product_context, definition_context, rig_rep)
    for ident, frame in (("pair-1", first), ("pair-2", turned)):
        reversed_occurrence(w, ident, rig, pair_definition, pair_rep, rig_rep, frame, shared)
    return w


def encoded(encoding):
    w = Writer()
    product_context, definition_context, geometric = context(w, inch=encoding == "untyped", encoding=encoding)
    pair(w, product_context, definition_context, geometric)
    return w


FIXTURES = {
    "assembly-nested.step": ("Original nested assembly: rig uses pair twice, pair uses block twice; millimetres", nested),
    "assembly-missing-placement.step": ("Original assembly whose second block occurrence has no placement", lambda: nested(placed=False)),
    "assembly-unplaced.step": ("Original part with a second solid that no product definition uses", unplaced),
    "assembly-styled.step": ("Original styled block: translucent solid colour, blue top face, a curve style and a layer", styled),
    "assembly-inch.step": ("Original assembly in inches with a starred conversion-based unit dimension", inch),
    "assembly-mapped.step": ("Original nested assembly whose occurrences are placed by mapped items", mapped),
    "assembly-operator.step": ("Original assembly placed by mirroring and scaling Cartesian operators", operators),
    "assembly-si-dimensions.step": ("Original assembly with explicit SI unit dimensions and unordered unit components",
                                    lambda: encoded("explicit")),
    "assembly-untyped-measure.step": ("Original assembly in inches with untyped measure values", lambda: encoded("untyped")),
    "assembly-reversed-items.step": ("Original nested assembly whose item defined transformations list the parent placement first",
                                     reversed_items),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    for name, (description, build) in FIXTURES.items():
        w = build()
        text = HEADER.format(description=description, name=name) + "\n".join(w.lines) + "\nENDSEC;\nEND-ISO-10303-21;\n"
        (args.output / name).write_text(text, encoding="ascii", newline="\n")


if __name__ == "__main__":
    main()
