//! Edge-based B-rep solids with elementary and B-spline geometry.
use super::*;
use crate::pcurve::{
    Chart, CurveShape, Frame, Inverter, PcurveError, Singular, V3, add, dot, nearest, norm, scale,
    sub,
};
use std::f64::consts::TAU;
use tessstep_curves::spline::{NurbsCurve, SplineLimits};
use tessstep_math::{Angle, AngleUnit, Length, ParameterSpace};
use tessstep_surfaces::{NurbsSurface, Surface};

/// Representation adaptations applied while importing. None changes geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Adaptations {
    /// Faces with several bounds and no FACE_OUTER_BOUND whose outer bound was
    /// identified as the unique counterclockwise loop in the surface chart.
    pub inferred_outer_bounds: usize,
    /// Synthetic isoparametric seam edges that join the two loops of an annular face
    /// on a periodic surface into one chart boundary. The seam lies on the surface
    /// between existing vertices; it adds no mesh vertices except along itself.
    pub inserted_seams: usize,
    /// Edges split at a curve parameter so that an annular face has a vertex pair
    /// for its seam. Both halves keep the original curve; every face using the edge
    /// is updated, so shared topology stays consistent.
    pub split_edges: usize,
    /// Spherical faces planned in a rotated frame because the STEP frame puts a pole
    /// on or inside them; the sphere and its orientation are unchanged.
    pub recharted_spheres: usize,
    /// Collapsed edges that close a face chart along a pole or apex line: at a loop
    /// vertex where two uses meet at the singular point, at a VERTEX_LOOP, or at the
    /// end of an inserted seam from a loop that encloses the pole. They have no 3D
    /// extent; the pole vertex is inserted when no VERTEX_LOOP names it.
    pub collapsed_edges: usize,
}

/// Import one selected MANIFOLD_SOLID_BREP whose faces lie on planes, cylinders,
/// cones, spheres, ring tori or B-spline surfaces (including rational and
/// quasi-uniform forms), bounded by LINE, CIRCLE, ELLIPSE or B-spline EDGE_CURVEs
/// (directly or as the 3D curve of a SURFACE_CURVE, SEAM_CURVE or
/// INTERSECTION_CURVE). Shared vertices/edges retain VERTEX_POINT/EDGE_CURVE identity.
///
/// Pcurves are computed from the 3D curves; supplied PCURVE geometry is retained as
/// a link and never read. The outer bound of a face without FACE_OUTER_BOUND is its
/// unique counterclockwise loop. An annular face whose two loops wind around a
/// periodic surface direction is cut by an inserted isoparametric seam between two
/// aligned vertices. Both adaptations are counted in [`ImportedSolid::adaptations`].
///
/// `angle` scales CONICAL_SURFACE semi-angles, the only plane-angle measure read.
/// Vertex loops (surface poles and apices), faces that enclose a pole, cavity shells
/// (BREP_WITH_VOIDS), degenerate/horn tori and swept surfaces are unsupported. No
/// coordinate welding, healing or unit inference is performed. ORIENTED_EDGE
/// endpoint slots must be `*`, or `$` unless `options.strict`.
pub fn import_brep_solid(
    document: &Document,
    root: EntityId,
    length: LengthUnit,
    angle: AngleUnit,
    tolerance: ModelTolerance,
    options: ImportOptions,
) -> Result<ImportedSolid, Error> {
    use brep_profile::schema_tessstep_brep as s;
    use tessstep_schema::EntityBinding;
    let schema = &brep_profile::SCHEMA_SET;
    let schema_name = "tessstep_brep";
    let omitted = [
        (s::Entity_ORIENTED_EDGE::DECLARATION, "EDGE_START"),
        (s::Entity_ORIENTED_EDGE::DECLARATION, "EDGE_END"),
        (s::Entity_ORIENTED_CLOSED_SHELL::DECLARATION, "CFS_FACES"),
    ]
    .map(|(entity, attribute)| decode::OmittedSlot {
        entity,
        attribute,
        allow_unset: !options.strict,
    });
    let links = [decode::LinkSlot {
        entity: s::Entity_SURFACE_CURVE::DECLARATION,
        attribute: "ASSOCIATED_GEOMETRY",
    }];
    let decoded = decode::decode_reachable_profile_with_links(
        document,
        schema,
        schema_name,
        &[root],
        &omitted,
        &links,
        decode::Limits {
            max_work: options.max_work,
            ..decode::Limits::default()
        },
    )
    .map_err(|e| profile_error(document, schema, schema_name, e))?;
    let mut c = Context {
        decoded: &decoded,
        raw: RawBrep::default(),
        points: BTreeMap::new(),
        point_entities: Vec::new(),
        edges: BTreeMap::new(),
        edge_entities: BTreeMap::new(),
        remaining: options.max_work,
        max_records: options.max_records.min(1_000_000),
        records: 0,
        current: root,
        unit: length,
        tolerance,
    };
    let mut b = Builder {
        angle,
        edges: Vec::new(),
        edge_index: BTreeMap::new(),
        faces: Vec::new(),
        work: pcurve::Work {
            remaining: options.max_work,
        },
        adaptations: Adaptations::default(),
    };
    let solid = c.view(root)?;
    if c.is(solid, "BREP_WITH_VOIDS") {
        return Err(c.error(
            ErrorKind::Unsupported,
            "BREP_WITH_VOIDS cavity shells are not supported",
        ));
    }
    if !c.is(solid, "MANIFOLD_SOLID_BREP") {
        return Err(c.error(
            ErrorKind::Unsupported,
            "selected root does not match the requested solid profile",
        ));
    }
    let shell = c.reference(solid, "OUTER")?;
    for face_ref in c.aggregate(shell, "CFS_FACES")? {
        c.charge(1)?;
        let face = c.target(face_ref)?;
        b.face(&mut c, face)?;
    }
    b.build(&mut c, root, options)
}

fn profile_error(
    document: &Document,
    schema: &tessstep_schema::SchemaSet,
    schema_name: &str,
    e: decode::Error,
) -> Error {
    Error {
        kind: match e.kind {
            decode::ErrorKind::ResourceLimit => ErrorKind::ResourceLimit,
            decode::ErrorKind::UnknownEntity | decode::ErrorKind::Unsupported => {
                ErrorKind::Unsupported
            }
            decode::ErrorKind::MissingReference => ErrorKind::MissingEntity,
            _ => ErrorKind::InvalidGeometry,
        },
        stage: Stage::Profile,
        entity: e.entity,
        source: e.source,
        message: if e.kind == decode::ErrorKind::UnknownEntity {
            outside_profile(document, schema, schema_name, e.entity)
        } else {
            e.to_string()
        },
    }
}

#[derive(Clone)]
struct EdgeRecord {
    entity: Option<EntityId>,
    curve: CurveGeometry<ModelSpace, 3>,
    shape: CurveShape,
    /// Increasing curve parameter interval; `vertices[0]` is at `range[0]`.
    range: [f64; 2],
    vertices: [VertexId; 2],
}
/// One oriented use of an edge; `forward` follows the canonical edge direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Use {
    edge: usize,
    forward: bool,
}
#[derive(Clone)]
struct LoopRecord {
    /// Uses oriented with the surface: the face lies to their left in the UV chart.
    uses: Vec<Use>,
    outer: bool,
    /// The vertex of a VERTEX_LOOP bound, which has no uses.
    vertex: Option<VertexId>,
}
#[derive(Clone)]
struct FaceRecord {
    entity: EntityId,
    chart: Chart,
    surface: SurfaceGeometry,
    same: bool,
    loops: Vec<LoopRecord>,
}
struct Builder {
    angle: AngleUnit,
    edges: Vec<EdgeRecord>,
    edge_index: BTreeMap<EntityId, (usize, bool)>,
    faces: Vec<FaceRecord>,
    work: pcurve::Work,
    adaptations: Adaptations,
}

impl<'d, 'a> Context<'d, 'a> {
    fn real(&self, v: &'d EntityView<'a>, name: &str) -> Result<f64, Error> {
        match self.attr(v, name)?.kind {
            ValueKind::Real(x) => Ok(x),
            ValueKind::Integer(x) => Ok(x as f64),
            _ => Err(self.error(ErrorKind::InvalidGeometry, "expected a numeric value")),
        }
    }
    fn integer(&self, v: &StepValue) -> Result<i64, Error> {
        match v.kind {
            ValueKind::Integer(x) => Ok(x),
            _ => Err(self.error(ErrorKind::InvalidGeometry, "expected an integer")),
        }
    }
    fn number(&self, v: &StepValue) -> Result<f64, Error> {
        match v.kind {
            ValueKind::Real(x) => Ok(x),
            ValueKind::Integer(x) => Ok(x as f64),
            _ => Err(self.error(ErrorKind::InvalidGeometry, "expected a numeric value")),
        }
    }
    fn length(&self, v: &'d EntityView<'a>, name: &str) -> Result<f64, Error> {
        let value = self.checked(self.unit.to_metres(self.real(v, name)?))?;
        if value <= 0. {
            return Err(self.error(
                ErrorKind::InvalidGeometry,
                format!("{name} must be a positive length"),
            ));
        }
        Ok(value)
    }
    fn placement(&self, v: &'d EntityView<'a>) -> Result<PlaneFrame<ModelSpace, 3>, Error> {
        self.frame(v)
    }
    fn points(&self, values: &'a [StepValue]) -> Result<Vec<Point<ModelSpace, 3>>, Error> {
        values
            .iter()
            .map(|value| self.point(self.target(value)?))
            .collect()
    }
}

/// Expanded knot vector from STEP multiplicities, or the implicit quasi-uniform one.
fn knot_vector<'d, 'a>(
    c: &Context<'d, 'a>,
    v: &'d EntityView<'a>,
    prefix: &str,
    degree: usize,
    controls: usize,
) -> Result<Vec<f64>, Error> {
    let mut knots = Vec::new();
    if c.is(v, "QUASI_UNIFORM_CURVE") || c.is(v, "QUASI_UNIFORM_SURFACE") {
        let spans = controls
            .checked_sub(degree)
            .filter(|&n| n > 0)
            .ok_or_else(|| c.error(ErrorKind::InvalidGeometry, "too few control points"))?;
        knots.extend(std::iter::repeat_n(0., degree + 1));
        knots.extend((1..spans).map(|i| i as f64));
        knots.extend(std::iter::repeat_n(spans as f64, degree + 1));
        return Ok(knots);
    }
    let (m, k) = if prefix.is_empty() {
        ("KNOT_MULTIPLICITIES".to_string(), "KNOTS".to_string())
    } else {
        (
            format!("{prefix}_MULTIPLICITIES"),
            format!("{prefix}_KNOTS"),
        )
    };
    let multiplicities = c.aggregate(v, &m)?;
    let values = c.aggregate(v, &k)?;
    if multiplicities.len() != values.len() {
        return Err(c.error(
            ErrorKind::InvalidGeometry,
            "knot multiplicity and value counts differ",
        ));
    }
    for (m, k) in multiplicities.iter().zip(values) {
        let m = c.integer(m)?;
        if !(1..=(degree as i64 + 1)).contains(&m) || knots.len() > 2 * HARD_KNOTS {
            return Err(c.error(ErrorKind::InvalidGeometry, "invalid knot multiplicity"));
        }
        knots.extend(std::iter::repeat_n(c.number(k)?, m as usize));
    }
    if knots.len() != controls + degree + 1 {
        return Err(c.error(
            ErrorKind::InvalidGeometry,
            "knot count does not match degree and control points",
        ));
    }
    Ok(knots)
}
const HARD_KNOTS: usize = 100_034;
fn degree<'d, 'a>(c: &Context<'d, 'a>, v: &'d EntityView<'a>, name: &str) -> Result<usize, Error> {
    match c.attr(v, name)?.kind {
        ValueKind::Integer(d) if (1..=16).contains(&d) => Ok(d as usize),
        _ => Err(c.error(ErrorKind::Unsupported, "B-spline degree outside 1..=16")),
    }
}

impl Builder {
    fn curve<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        v: &'d EntityView<'a>,
    ) -> Result<(CurveGeometry<ModelSpace, 3>, CurveShape), Error> {
        c.charge(1)?;
        c.current = v.id;
        if c.is(v, "SURFACE_CURVE") {
            let inner = c.reference(v, "CURVE_3D")?;
            if c.is(inner, "SURFACE_CURVE") {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "nested SURFACE_CURVE 3D curves are not supported",
                ));
            }
            return self.curve(c, inner);
        }
        if c.is(v, "LINE") {
            let origin = c.point(c.reference(v, "PNT")?)?;
            let vector = c.reference(v, "DIR")?;
            let direction = c.direction(c.reference(vector, "ORIENTATION")?)?;
            let magnitude = c.real(vector, "MAGNITUDE")?;
            if magnitude <= 0. {
                return Err(c.error(
                    ErrorKind::InvalidGeometry,
                    "LINE requires positive vector magnitude",
                ));
            }
            let tangent = c.checked(direction.scaled(c.checked(c.unit.to_metres(magnitude))?))?;
            let curve = c.checked(Curve::line(origin, tangent))?;
            return Ok((
                CurveGeometry::Analytic(curve),
                CurveShape::Line {
                    origin: origin.coordinates(),
                    tangent: tangent.components(),
                },
            ));
        }
        if c.is(v, "CIRCLE") || c.is(v, "ELLIPSE") {
            let frame = c.placement(c.reference(v, "POSITION")?)?;
            let (a, b) = if c.is(v, "CIRCLE") {
                let r = c.length(v, "RADIUS")?;
                (r, r)
            } else {
                (c.length(v, "SEMI_AXIS_1")?, c.length(v, "SEMI_AXIS_2")?)
            };
            let curve = c.checked(Curve::ellipse(
                frame,
                c.checked(Length::metres(a))?,
                c.checked(Length::metres(b))?,
            ))?;
            return Ok((
                CurveGeometry::Analytic(curve),
                CurveShape::Conic {
                    frame: Frame::new(frame),
                    a,
                    b,
                },
            ));
        }
        if c.is(v, "B_SPLINE_CURVE") {
            let degree = degree(c, v, "DEGREE")?;
            let controls = c.points(c.aggregate(v, "CONTROL_POINTS_LIST")?)?;
            c.charge(controls.len())?;
            let knots = knot_vector(c, v, "", degree, controls.len())?;
            let weights = if c.is(v, "RATIONAL_B_SPLINE_CURVE") {
                c.aggregate(v, "WEIGHTS_DATA")?
                    .iter()
                    .map(|w| c.number(w))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                vec![1.; controls.len()]
            };
            if !c.is(v, "B_SPLINE_CURVE_WITH_KNOTS") && !c.is(v, "QUASI_UNIFORM_CURVE") {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "B_SPLINE_CURVE needs explicit or quasi-uniform knots",
                ));
            }
            let curve =
                NurbsCurve::new(degree, &knots, &controls, &weights, SplineLimits::default())
                    .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))?;
            return Ok((CurveGeometry::Nurbs(curve), CurveShape::Nurbs));
        }
        Err(c.error(
            ErrorKind::Unsupported,
            "edge curve type is not supported by the B-rep import profile",
        ))
    }

    fn surface<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        v: &'d EntityView<'a>,
    ) -> Result<(Chart, SurfaceGeometry), Error> {
        c.charge(1)?;
        c.current = v.id;
        if c.is(v, "ELEMENTARY_SURFACE") {
            let frame = c.placement(c.reference(v, "POSITION")?)?;
            let f = Frame::new(frame);
            let shape = |e: tessstep_surfaces::Error| c.error(ErrorKind::InvalidGeometry, e);
            return if c.is(v, "PLANE") {
                Ok((Chart::Plane(f), Surface::plane(frame)))
            } else if c.is(v, "CYLINDRICAL_SURFACE") {
                let r = c.length(v, "RADIUS")?;
                Ok((
                    Chart::Cylinder(f, r),
                    Surface::cylinder(frame, c.checked(Length::metres(r))?).map_err(shape)?,
                ))
            } else if c.is(v, "CONICAL_SURFACE") {
                let r = c.checked(c.unit.to_metres(c.real(v, "RADIUS")?))?;
                let alpha = c
                    .checked(self.angle.to_angle(c.real(v, "SEMI_ANGLE")?))?
                    .as_radians();
                if r < 0. || !(alpha > 0. && alpha < std::f64::consts::FRAC_PI_2) {
                    return Err(c.error(
                        ErrorKind::InvalidGeometry,
                        "CONICAL_SURFACE needs a nonnegative radius and a semi-angle in (0, 90°)",
                    ));
                }
                // The kernel cone is placed at its apex, on the axis below the placement.
                let apex = sub(f.o, scale(f.z, r / alpha.tan()));
                let apex_frame = c.checked(PlaneFrame::new(
                    c.checked(Point::new(apex))?,
                    frame.x(),
                    frame.y(),
                    NumericalTolerance::default(),
                ))?;
                Ok((
                    Chart::Cone(Frame::new(apex_frame), alpha),
                    Surface::cone(apex_frame, c.checked(Angle::radians(alpha))?).map_err(shape)?,
                ))
            } else if c.is(v, "SPHERICAL_SURFACE") {
                let r = c.length(v, "RADIUS")?;
                Ok((
                    Chart::Sphere(f, r),
                    Surface::sphere(frame, c.checked(Length::metres(r))?).map_err(shape)?,
                ))
            } else if c.is(v, "TOROIDAL_SURFACE") {
                let major = c.length(v, "MAJOR_RADIUS")?;
                let minor = c.length(v, "MINOR_RADIUS")?;
                if major <= minor {
                    return Err(c.error(
                        ErrorKind::Unsupported,
                        "horn and spindle tori (major radius <= minor radius) are not supported",
                    ));
                }
                Ok((
                    Chart::Torus(f, major, minor),
                    Surface::torus(
                        frame,
                        c.checked(Length::metres(major))?,
                        c.checked(Length::metres(minor))?,
                    )
                    .map_err(shape)?,
                ))
            } else {
                Err(c.error(ErrorKind::Unsupported, "unsupported elementary surface"))
            }
            .map(|(chart, surface)| (chart, SurfaceGeometry::Analytic(surface)));
        }
        if c.is(v, "B_SPLINE_SURFACE") {
            if !c.is(v, "B_SPLINE_SURFACE_WITH_KNOTS") && !c.is(v, "QUASI_UNIFORM_SURFACE") {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "B_SPLINE_SURFACE needs explicit or quasi-uniform knots",
                ));
            }
            let degrees = [degree(c, v, "U_DEGREE")?, degree(c, v, "V_DEGREE")?];
            let rows = c.aggregate(v, "CONTROL_POINTS_LIST")?;
            let mut controls = Vec::new();
            let mut nv = None;
            for row in rows {
                let ValueKind::Aggregate(row) = &row.kind else {
                    return Err(c.error(ErrorKind::InvalidGeometry, "expected a control row"));
                };
                if nv.replace(row.len()).is_some_and(|n| n != row.len()) {
                    return Err(c.error(ErrorKind::InvalidGeometry, "ragged control net"));
                }
                c.charge(row.len())?;
                controls.extend(c.points(row)?);
            }
            let shape = [rows.len(), nv.unwrap_or(0)];
            let knots = [
                knot_vector(c, v, "U", degrees[0], shape[0])?,
                knot_vector(c, v, "V", degrees[1], shape[1])?,
            ];
            let weights = if c.is(v, "RATIONAL_B_SPLINE_SURFACE") {
                let mut weights = Vec::new();
                for row in c.aggregate(v, "WEIGHTS_DATA")? {
                    let ValueKind::Aggregate(row) = &row.kind else {
                        return Err(c.error(ErrorKind::InvalidGeometry, "expected a weight row"));
                    };
                    if row.len() != shape[1] {
                        return Err(c.error(ErrorKind::InvalidGeometry, "ragged weight net"));
                    }
                    for w in row {
                        weights.push(c.number(w)?);
                    }
                }
                weights
            } else {
                vec![1.; controls.len()]
            };
            let mut surface = NurbsSurface::new(
                degrees,
                [&knots[0], &knots[1]],
                shape,
                &controls,
                &weights,
                SplineLimits::default(),
            )
            .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))?;
            // A closed axis gets a periodic chart when its boundary curves agree within
            // the model tolerance; the STEP flag alone is advisory.
            let closed = [
                matches!(&c.attr(v, "U_CLOSED")?.kind, ValueKind::Enumeration(e) if e.as_ref() == "T"),
                matches!(&c.attr(v, "V_CLOSED")?.kind, ValueKind::Enumeration(e) if e.as_ref() == "T"),
            ];
            let mut periods = [None; 2];
            for axis in 0..2 {
                if !closed[axis] {
                    continue;
                }
                let mut axes = surface.periodic_axes();
                axes[axis] = true;
                if let Ok(periodic) = surface
                    .clone()
                    .with_periodic_axes(axes, c.tolerance.distance().as_metres())
                {
                    let [lo, hi] = periodic.knot_vectors()[axis].domain();
                    periods[axis] = Some(hi - lo);
                    surface = periodic;
                }
            }
            let singular = pcurve::nurbs_singular(&surface, c.tolerance.distance().as_metres());
            return Ok((
                Chart::Nurbs(periods, singular),
                SurfaceGeometry::Nurbs(surface),
            ));
        }
        Err(c.error(
            ErrorKind::Unsupported,
            "face surface type is not supported by the B-rep import profile",
        ))
    }

    /// Curve parameter of a vertex. Closed B-splines resolve their shared end point
    /// to the domain start or end as `at_end` requests.
    fn parameter(
        &self,
        c: &Context<'_, '_>,
        curve: &CurveGeometry<ModelSpace, 3>,
        shape: &CurveShape,
        p: V3,
        at_end: bool,
    ) -> Result<f64, Error> {
        let t = match shape {
            CurveShape::Line { origin, tangent } => {
                dot(sub(p, *origin), *tangent) / dot(*tangent, *tangent)
            }
            CurveShape::Conic { frame, a, b } => {
                let d = sub(p, frame.o);
                (dot(d, frame.y) / b).atan2(dot(d, frame.x) / a)
            }
            CurveShape::Nurbs => {
                let CurveGeometry::Nurbs(n) = curve else {
                    unreachable!("NURBS shape");
                };
                nurbs_parameter(c, n, p, at_end)?
            }
        };
        let q = c.checked(curve.evaluate(t))?.position.coordinates();
        if norm(sub(q, p)) > c.tolerance.distance().as_metres() {
            return Err(c.error(
                ErrorKind::InvalidGeometry,
                "EDGE_CURVE endpoint is off its declared curve",
            ));
        }
        Ok(t)
    }

    fn edge<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        edge: &'d EntityView<'a>,
    ) -> Result<(usize, bool), Error> {
        if let Some(&found) = self.edge_index.get(&edge.id) {
            return Ok(found);
        }
        let start = c.topological_vertex(c.reference(edge, "EDGE_START")?)?;
        let end = c.topological_vertex(c.reference(edge, "EDGE_END")?)?;
        c.current = edge.id;
        let same = c.boolean(edge, "SAME_SENSE")?;
        let geometry = c.reference(edge, "EDGE_GEOMETRY")?;
        let (curve, shape) = self.curve(c, geometry)?;
        c.current = edge.id;
        let position = |v: VertexId| c.raw.vertices[v.0].position.coordinates();
        let closed = start == end;
        let range = match (&curve, &shape) {
            (_, CurveShape::Conic { .. }) => {
                let ts = self.parameter(c, &curve, &shape, position(start), false)?;
                if closed {
                    [ts, ts + TAU]
                } else {
                    let te = self.parameter(c, &curve, &shape, position(end), true)?;
                    let (a, b) = if same { (ts, te) } else { (te, ts) };
                    let span = (b - a).rem_euclid(TAU);
                    if span == 0. {
                        return Err(c.error(
                            ErrorKind::InvalidGeometry,
                            "distinct EDGE_CURVE vertices coincide on a closed conic",
                        ));
                    }
                    [a, a + span]
                }
            }
            (CurveGeometry::Nurbs(n), _) if closed => {
                let domain = n.knot_vector().domain();
                self.parameter(c, &curve, &shape, position(start), false)?;
                let ends = [domain[0], domain[1]].map(|t| curve.evaluate(t));
                let [Ok(a), Ok(b)] = ends else {
                    return Err(c.error(ErrorKind::InvalidGeometry, "curve evaluation failed"));
                };
                let tol = c.tolerance.distance().as_metres();
                let p = c.raw.vertices[start.0].position;
                if c.checked(a.position.distance(p))? > tol
                    || c.checked(b.position.distance(p))? > tol
                {
                    return Err(c.error(
                        ErrorKind::Unsupported,
                        "closed B-spline edges must start and end at the curve domain ends",
                    ));
                }
                domain
            }
            _ => {
                if closed {
                    return Err(c.error(
                        ErrorKind::InvalidGeometry,
                        "an open curve cannot bound a closed edge",
                    ));
                }
                let ts = self.parameter(c, &curve, &shape, position(start), !same)?;
                let te = self.parameter(c, &curve, &shape, position(end), same)?;
                if (same && ts >= te) || (!same && ts <= te) {
                    return Err(c.error(
                        ErrorKind::InvalidGeometry,
                        "EDGE_CURVE same_sense contradicts curve parameter direction",
                    ));
                }
                [ts.min(te), ts.max(te)]
            }
        };
        c.record()?;
        let index = self.edges.len();
        self.edges.push(EdgeRecord {
            entity: Some(edge.id),
            curve,
            shape,
            range,
            vertices: if same { [start, end] } else { [end, start] },
        });
        self.edge_index.insert(edge.id, (index, same));
        Ok((index, same))
    }

    fn face<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        face: &'d EntityView<'a>,
    ) -> Result<(), Error> {
        c.current = face.id;
        let (chart, surface) = self.surface(c, c.reference(face, "FACE_GEOMETRY")?)?;
        c.current = face.id;
        let same = c.boolean(face, "SAME_SENSE")?;
        let mut loops = Vec::new();
        for bound_ref in c.aggregate(face, "BOUNDS")? {
            c.charge(1)?;
            let bound = c.target(bound_ref)?;
            c.current = bound.id;
            let orientation = c.boolean(bound, "ORIENTATION")?;
            let wire = c.reference(bound, "BOUND")?;
            if c.is(wire, "VERTEX_LOOP") {
                let vertex = c.topological_vertex(c.reference(wire, "LOOP_VERTEX")?)?;
                loops.push(LoopRecord {
                    uses: Vec::new(),
                    outer: c.is(bound, "FACE_OUTER_BOUND"),
                    vertex: Some(vertex),
                });
                continue;
            }
            let mut uses = Vec::new();
            for value in c.aggregate(wire, "EDGE_LIST")? {
                c.charge(1)?;
                let oriented = c.target(value)?;
                let forward = c.boolean(oriented, "ORIENTATION")?;
                let (edge, edge_same) = self.edge(c, c.reference(oriented, "EDGE_ELEMENT")?)?;
                uses.push(Use {
                    edge,
                    forward: forward == edge_same,
                });
            }
            // Store every loop oriented with the surface; the face orientation restores
            // STEP's same_sense in shell incidence and triangles.
            if orientation != same {
                uses.reverse();
                for u in &mut uses {
                    u.forward = !u.forward;
                }
            }
            loops.push(LoopRecord {
                uses,
                outer: c.is(bound, "FACE_OUTER_BOUND"),
                vertex: None,
            });
        }
        self.faces.push(FaceRecord {
            entity: face.id,
            chart,
            surface,
            same,
            loops,
        });
        Ok(())
    }

    fn pcurve_error(&self, c: &Context<'_, '_>, edge: usize, e: PcurveError) -> Error {
        let name = match self.edges[edge].entity {
            Some(id) => format!("EDGE_CURVE #{}", id.get()),
            None => "inserted seam".to_string(),
        };
        let (kind, message) = match e {
            PcurveError::OffSurface {
                parameter,
                distance,
            } => (
                ErrorKind::InvalidGeometry,
                format!(
                    "{name} lies {distance:.3e} m from the face surface at curve parameter {parameter}"
                ),
            ),
            PcurveError::Singular { parameter } => (
                ErrorKind::Unsupported,
                format!(
                    "{name} crosses a singular point of the face surface at curve parameter {parameter}"
                ),
            ),
            PcurveError::Unresolved => (
                ErrorKind::Unsupported,
                format!(
                    "{name} pcurve did not meet the model tolerance within the subdivision limit"
                ),
            ),
            PcurveError::Limit => (ErrorKind::ResourceLimit, "pcurve work budget".to_string()),
            PcurveError::Geometry => (
                ErrorKind::InvalidGeometry,
                format!("{name} could not be mapped to the face surface"),
            ),
        };
        c.error(kind, message)
    }

    fn build(
        mut self,
        c: &mut Context<'_, '_>,
        root: EntityId,
        options: ImportOptions,
    ) -> Result<ImportedSolid, Error> {
        let tolerance = c.tolerance.distance().as_metres();
        let mut faces = std::mem::take(&mut self.faces);
        // Plan every face; annular faces without aligned seam vertices request edge
        // splits, which are applied to all faces before planning again.
        let mut plans = Vec::new();
        for pass in 0.. {
            let saved = self.snapshot(c);
            plans.clear();
            let mut splits = BTreeMap::new();
            for face in &faces {
                c.current = face.entity;
                match self.plan(c, face, tolerance)? {
                    Planned::Face(plan) => plans.push(plan),
                    Planned::Split { edge, parameter } => {
                        splits.entry(edge).or_insert(parameter);
                    }
                }
            }
            if splits.is_empty() {
                break;
            }
            if pass >= 8 {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "annular faces still lack aligned seam vertices after edge splitting",
                ));
            }
            // Planning state is discarded; the splits below persist into the next pass.
            self.restore(c, saved);
            for (edge, parameter) in splits {
                self.split(c, &mut faces, edge, parameter)?;
            }
        }
        for e in &self.edges {
            c.raw.edges.push(Edge {
                vertices: e.vertices,
                curve: e.curve.clone(),
                range: e.range,
            });
        }
        let mut face_entities = Vec::new();
        for (face, plan) in faces.iter().zip(plans) {
            c.current = face.entity;
            let wire = |c: &mut Context<'_, '_>, uses: &[Use]| -> Result<WireId, Error> {
                let mut coedges = Vec::new();
                for u in uses {
                    c.record()?;
                    coedges.push(CoedgeId(c.raw.coedges.len()));
                    c.raw.coedges.push(Coedge {
                        edge: EdgeId(u.edge),
                        orientation: if u.forward {
                            Orientation::Forward
                        } else {
                            Orientation::Reversed
                        },
                        pcurve: Some(plan.pcurves[&u.edge].clone()),
                    });
                }
                c.record()?;
                c.raw.wires.push(Wire { coedges });
                Ok(WireId(c.raw.wires.len() - 1))
            };
            let outer = wire(c, &plan.outer)?;
            let mut holes = Vec::new();
            for hole in &plan.holes {
                holes.push(wire(c, hole)?);
            }
            c.record()?;
            c.raw.faces.push(Face {
                surface: plan.surface.clone().unwrap_or_else(|| face.surface.clone()),
                outer,
                holes,
                orientation: if face.same {
                    Orientation::Forward
                } else {
                    Orientation::Reversed
                },
            });
            face_entities.push(face.entity);
        }
        c.current = root;
        c.record()?;
        c.record()?;
        c.raw.shells.push(Shell {
            faces: (0..face_entities.len()).map(FaceId).collect(),
            closed: true,
        });
        c.raw.solids.push(Solid { shell: ShellId(0) });
        let raw = std::mem::take(&mut c.raw);
        let brep = raw
            .validate(
                c.tolerance,
                ValidationLimits {
                    max_records: options.max_records,
                    max_work: options.max_work,
                },
            )
            .map_err(|e| Error {
                kind: if e.defect == Defect::ResourceLimit {
                    ErrorKind::ResourceLimit
                } else {
                    ErrorKind::InvalidGeometry
                },
                stage: Stage::Topology,
                entity: Some(root),
                source: c.decoded.document().entities().get(root).map(|e| e.source),
                message: e.to_string(),
            })?
            .normalize();
        Ok(ImportedSolid {
            brep,
            faces: face_entities,
            root,
            adaptations: self.adaptations,
        })
    }
}

enum Planned {
    Face(Plan),
    Split { edge: usize, parameter: f64 },
}
enum SeamChoice {
    Seam(Box<EdgeRecord>, usize, usize, [f64; 2], bool),
    Split { edge: usize, parameter: f64 },
}
/// First interior edge parameter at which a pcurve crosses `value` modulo `period`
/// along axis `k`, with the UV point there. Sampled brackets are refined by bisection.
fn crossing(pcurve: &Pcurve, k: usize, value: f64, period: f64) -> Option<(f64, [f64; 2])> {
    let [t0, t1] = pcurve.range;
    let at = |t: f64| -> Option<[f64; 2]> {
        Some(pcurve.curve.evaluate(t).ok()?.position.coordinates())
    };
    let margin = 1e-9 * (t1 - t0).abs();
    let n = 64;
    let mut previous = (t0, at(t0)?);
    for i in 1..=n {
        let t = t0 + (t1 - t0) * i as f64 / n as f64;
        let uv = at(t)?;
        let (ta, ua) = previous;
        let (lo, hi) = (ua[k].min(uv[k]), ua[k].max(uv[k]));
        let first = ((lo - value) / period).ceil() as i64;
        let last = ((hi - value) / period).floor() as i64;
        for m in first..=last.min(first + 4) {
            let target = value + m as f64 * period;
            let (mut a, mut b) = (ta, t);
            let (fa, fb) = (ua[k] - target, uv[k] - target);
            if fb == 0. && (t - t0).abs() > margin && (t1 - t).abs() > margin {
                return Some((t, uv));
            }
            if fa == 0. || fb == 0. || fa.signum() == fb.signum() {
                continue;
            }
            for _ in 0..80 {
                let mid = 0.5 * (a + b);
                let fm = at(mid)?[k] - target;
                if fm.signum() == fa.signum() {
                    a = mid;
                } else {
                    b = mid;
                }
            }
            let root = 0.5 * (a + b);
            if (root - t0).abs() > margin && (t1 - root).abs() > margin {
                return Some((root, at(root)?));
            }
        }
        previous = (t, uv);
    }
    None
}
struct Plan {
    /// A re-charted surface replacing the face's STEP parameterization.
    surface: Option<SurfaceGeometry>,
    outer: Vec<Use>,
    holes: Vec<Vec<Use>>,
    pcurves: BTreeMap<usize, Pcurve>,
}
/// UV endpoints and sampled chart polygon of one loop.
struct LoopChart {
    /// Start UV of each use, in the chart of that use's own pcurve.
    starts: Vec<[f64; 2]>,
    winding: [i64; 2],
    polygon: Vec<[f64; 2]>,
}

impl Builder {
    fn chart_loop(
        &self,
        face: &FaceRecord,
        uses: &[Use],
        pcurves: &BTreeMap<usize, Pcurve>,
    ) -> Result<LoopChart, PcurveError> {
        let periods = face.chart.periods();
        let at = |u: &Use, s: f64| -> Result<[f64; 2], PcurveError> {
            let p = &pcurves[&u.edge];
            let s = if u.forward { s } else { 1. - s };
            let t = if s == 0. {
                p.range[0]
            } else if s == 1. {
                p.range[1]
            } else {
                p.range[0] + s * (p.range[1] - p.range[0])
            };
            Ok(p.curve
                .evaluate(t)
                .map_err(|_| PcurveError::Geometry)?
                .position
                .coordinates())
        };
        let mut starts = Vec::new();
        let mut displacement = [0.; 2];
        let mut polygon: Vec<[f64; 2]> = Vec::new();
        let mut previous: Option<[f64; 2]> = None;
        for u in uses {
            let first = at(u, 0.)?;
            let last = at(u, 1.)?;
            starts.push(first);
            for k in 0..2 {
                displacement[k] += last[k] - first[k];
            }
            // Translate each use by whole periods to continue from the previous end.
            let shift = previous.map_or([0.; 2], |end| {
                [0, 1].map(|k| match periods[k] {
                    Some(p) => ((end[k] - first[k]) / p).round() * p,
                    None => 0.,
                })
            });
            for i in 0..8 {
                let uv = at(u, i as f64 / 8.)?;
                polygon.push([uv[0] + shift[0], uv[1] + shift[1]]);
            }
            previous = Some([last[0] + shift[0], last[1] + shift[1]]);
        }
        let winding = [0, 1].map(|k| match periods[k] {
            Some(p) => (displacement[k] / p).round() as i64,
            None => 0,
        });
        Ok(LoopChart {
            starts,
            winding,
            polygon,
        })
    }

    /// Plan a face in its STEP chart. A spherical face that fails there because it
    /// touches or encloses a pole is retried with frames whose poles lie away from its
    /// boundary; the sphere, its orientation and all 3D geometry are unchanged. Pole
    /// and apex charts (collapsed edges) are used only when no frame avoids them.
    fn plan(
        &mut self,
        c: &mut Context<'_, '_>,
        face: &FaceRecord,
        tolerance: f64,
    ) -> Result<Planned, Error> {
        let Chart::Sphere(frame, radius) = face.chart else {
            return self.plan_chart(c, face, tolerance, true);
        };
        let axes = self.sphere_axes(c, face, frame)?;
        let mut native = None;
        for singular in [false, true] {
            let saved = self.snapshot(c);
            match self.plan_chart(c, face, tolerance, singular) {
                Err(e) if e.kind == ErrorKind::Unsupported => {
                    self.restore(c, saved);
                    if singular {
                        native = Some(e);
                    }
                }
                other => return other,
            }
            for &axis in &axes {
                c.charge(1)?;
                let Some(recharted) = sphere_chart(frame.o, axis, radius) else {
                    continue;
                };
                let mut candidate = face.clone();
                (candidate.chart, candidate.surface) = recharted;
                let saved = self.snapshot(c);
                match self.plan_chart(c, &candidate, tolerance, singular) {
                    Ok(Planned::Face(mut plan)) => {
                        plan.surface = Some(candidate.surface);
                        self.adaptations.recharted_spheres += 1;
                        return Ok(Planned::Face(plan));
                    }
                    Ok(split @ Planned::Split { .. }) => return Ok(split),
                    Err(e) if e.kind == ErrorKind::ResourceLimit => return Err(e),
                    Err(_) => self.restore(c, saved),
                }
            }
        }
        Err(native.expect("the singular native chart was planned"))
    }

    /// Planning state that failed attempts roll back: inserted edges and vertices.
    fn snapshot(&self, c: &Context<'_, '_>) -> (usize, usize, Adaptations) {
        (self.edges.len(), c.raw.vertices.len(), self.adaptations)
    }
    fn restore(
        &mut self,
        c: &mut Context<'_, '_>,
        (edges, vertices, adaptations): (usize, usize, Adaptations),
    ) {
        self.edges.truncate(edges);
        c.raw.vertices.truncate(vertices);
        c.point_entities.truncate(vertices);
        self.adaptations = adaptations;
    }

    /// Candidate sphere axes: directions to VERTEX_LOOP vertices (which then sit at a
    /// pole), perpendiculars to the mean boundary direction (which put both poles a
    /// quarter turn from a cap's centre), then 26 lattice directions, all at least
    /// 1e-3 rad from every sampled boundary direction.
    fn sphere_axes(
        &self,
        c: &Context<'_, '_>,
        face: &FaceRecord,
        frame: Frame,
    ) -> Result<Vec<V3>, Error> {
        let mut directions = Vec::new();
        let mut candidates = Vec::new();
        for l in &face.loops {
            if let Some(v) = l.vertex {
                let d = sub(c.raw.vertices[v.0].position.coordinates(), frame.o);
                if norm(d) > 0. {
                    candidates.push(scale(d, 1. / norm(d)));
                }
            }
            for u in &l.uses {
                let e = &self.edges[u.edge];
                for i in 0..=16 {
                    let t = e.range[0] + (e.range[1] - e.range[0]) * i as f64 / 16.;
                    if let Ok(jet) = e.curve.evaluate(t) {
                        let d = sub(jet.position.coordinates(), frame.o);
                        let n = norm(d);
                        if n > 0. {
                            directions.push(scale(d, 1. / n));
                        }
                    }
                }
            }
        }
        let mut mean = [0.; 3];
        for d in &directions {
            mean = add(mean, *d);
        }
        if norm(mean) > 1e-9 {
            let m = scale(mean, 1. / norm(mean));
            let helper = if m[0].abs() < 0.9 {
                [1., 0., 0.]
            } else {
                [0., 1., 0.]
            };
            let p = crate::pcurve::cross(m, helper);
            let p = scale(p, 1. / norm(p));
            let q = crate::pcurve::cross(m, p);
            for i in 0..6 {
                let a = std::f64::consts::PI * i as f64 / 6.;
                candidates.push(add(scale(p, a.cos()), scale(q, a.sin())));
            }
        }
        for x in [-1., 0., 1.] {
            for y in [-1., 0., 1.] {
                for z in [-1., 0., 1.] {
                    let d: V3 = [x, y, z];
                    if d != [0.; 3] {
                        candidates.push(scale(d, 1. / norm(d)));
                    }
                }
            }
        }
        Ok(candidates
            .into_iter()
            .filter(|axis| {
                directions
                    .iter()
                    .all(|d| norm(crate::pcurve::cross(*d, *axis)) > 1e-3)
            })
            .collect())
    }

    /// A collapsed edge at a singular vertex, with a pcurve from `start` along the
    /// singular line (periodic axis `k`) by `delta`.
    fn collapsed_edge(
        &mut self,
        c: &mut Context<'_, '_>,
        vertex: VertexId,
        start: [f64; 2],
        k: usize,
        delta: f64,
    ) -> Result<(usize, Pcurve), Error> {
        c.record()?;
        let span = delta.abs();
        let at = c.checked(Point::<ModelSpace, 3>::new(
            c.raw.vertices[vertex.0].position.coordinates(),
        ))?;
        let q = span / 4.;
        // Quarter-span knots seed the singular line's samples.
        let curve = NurbsCurve::new(
            1,
            &[0., 0., q, 2. * q, 3. * q, span, span],
            &[at; 5],
            &[1.; 5],
            SplineLimits::default(),
        )
        .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))?;
        let mut tangent = [0.; 2];
        tangent[k] = delta.signum();
        let pcurve = Pcurve {
            curve: CurveGeometry::Analytic(c.checked(Curve::line(
                c.checked(Point::<ParameterSpace, 2>::new(start))?,
                c.checked(Vector::new(tangent))?,
            ))?),
            range: [0., span],
        };
        self.edges.push(EdgeRecord {
            entity: None,
            curve: CurveGeometry::Nurbs(curve),
            shape: CurveShape::Nurbs,
            range: [0., span],
            vertices: [vertex; 2],
        });
        self.adaptations.collapsed_edges += 1;
        Ok((self.edges.len() - 1, pcurve))
    }

    /// An isoparametric seam at coordinate `at` of periodic axis `k`, between `from`
    /// (at `jf` along the other axis) and `to` (at `jt`), with its pcurve. Returns
    /// the edge and whether the use from `from` to `to` is forward.
    #[allow(clippy::too_many_arguments)]
    fn iso_seam(
        &mut self,
        c: &mut Context<'_, '_>,
        chart: &Chart,
        k: usize,
        at: f64,
        (from, jf): (VertexId, f64),
        (to, jt): (VertexId, f64),
        tolerance: f64,
    ) -> Result<Option<(usize, bool, Pcurve)>, Error> {
        let Some((curve, shape)) = seam_curve(chart, k, at) else {
            return Ok(None);
        };
        let position = |v: VertexId| c.raw.vertices[v.0].position.coordinates();
        let point = |t: f64| curve.evaluate(t).map(|e| e.position.coordinates());
        let (Ok(pf), Ok(pt)) = (point(jf), point(jt)) else {
            return Ok(None);
        };
        if norm(sub(pf, position(from))) > tolerance || norm(sub(pt, position(to))) > tolerance {
            return Ok(None);
        }
        let forward = jt > jf;
        let range = [jf.min(jt), jf.max(jt)];
        let (origin, tangent) = if k == 0 {
            ([at, 0.], [0., 1.])
        } else {
            ([0., at], [1., 0.])
        };
        let pcurve = Pcurve {
            curve: CurveGeometry::Analytic(c.checked(Curve::line(
                c.checked(Point::<ParameterSpace, 2>::new(origin))?,
                c.checked(Vector::new(tangent))?,
            ))?),
            range,
        };
        c.record()?;
        self.edges.push(EdgeRecord {
            entity: None,
            curve,
            shape,
            range,
            vertices: if forward { [from, to] } else { [to, from] },
        });
        self.adaptations.inserted_seams += 1;
        Ok(Some((self.edges.len() - 1, forward, pcurve)))
    }

    /// The vertex at a singular point: a VERTEX_LOOP's vertex, or an inserted one.
    fn pole_vertex(
        c: &mut Context<'_, '_>,
        face: &FaceRecord,
        poles: &mut Vec<(Singular, VertexId)>,
        s: &Singular,
    ) -> Result<VertexId, Error> {
        if let Some(i) = poles
            .iter()
            .position(|(p, _)| p.k == s.k && p.value == s.value)
        {
            return Ok(poles.remove(i).1);
        }
        c.record()?;
        c.raw.vertices.push(Vertex {
            position: c.checked(Point::new(s.point))?,
        });
        c.point_entities.push(face.entity);
        Ok(VertexId(c.raw.vertices.len() - 1))
    }

    fn plan_chart(
        &mut self,
        c: &mut Context<'_, '_>,
        face: &FaceRecord,
        tolerance: f64,
        allow_singular: bool,
    ) -> Result<Planned, Error> {
        if !allow_singular && face.loops.iter().any(|l| l.vertex.is_some()) {
            return Err(c.error(
                ErrorKind::Unsupported,
                "VERTEX_LOOP bounds need a pole or apex chart",
            ));
        }
        let inverter = Inverter {
            chart: &face.chart,
            surface: &face.surface,
            tolerance,
        };
        let mut pcurves = BTreeMap::new();
        for l in &face.loops {
            for u in &l.uses {
                if pcurves.contains_key(&u.edge) {
                    continue;
                }
                let e = &self.edges[u.edge];
                let p = pcurve::pcurve(&inverter, &e.shape, &e.curve, e.range, &mut self.work)
                    .map_err(|err| self.pcurve_error(c, u.edge, err))?;
                pcurves.insert(u.edge, p);
            }
        }
        let singular = face.chart.singular();
        let periods = face.chart.periods();
        let position =
            |c: &Context<'_, '_>, v: VertexId| c.raw.vertices[v.0].position.coordinates();
        let at_singular = |c: &Context<'_, '_>, v: VertexId| {
            singular
                .iter()
                .copied()
                .find(|s| norm(sub(position(c, v), s.point)) <= tolerance)
        };
        // Apex joins: where consecutive uses meet at a singular point, a collapsed edge
        // runs along the singular line between their chart ends, in the direction that
        // keeps the face on its left.
        let mut loops = face.loops.clone();
        for l in loops
            .iter_mut()
            .filter(|l| l.vertex.is_none() && !singular.is_empty())
        {
            let n = l.uses.len();
            let mut joined = Vec::with_capacity(n);
            for i in 0..n {
                c.charge(1)?;
                let (u, next) = (l.uses[i], l.uses[(i + 1) % n]);
                joined.push(u);
                let e = &self.edges[u.edge];
                let v = if u.forward {
                    e.vertices[1]
                } else {
                    e.vertices[0]
                };
                let Some(s) = at_singular(c, v) else {
                    continue;
                };
                if !allow_singular {
                    return Err(c.error(
                        ErrorKind::Unsupported,
                        "face loop meets a pole or apex of the surface chart",
                    ));
                }
                let end = use_uv(&pcurves[&u.edge], u, 1.)
                    .map_err(|e| self.pcurve_error(c, u.edge, e))?;
                let start = use_uv(&pcurves[&next.edge], next, 0.)
                    .map_err(|e| self.pcurve_error(c, next.edge, e))?;
                let (k, j) = (s.k, 1 - s.k);
                let near = |x: f64| (x - s.value).abs() <= 1e-7 * (1. + s.value.abs());
                if !near(end[j]) || !near(start[j]) {
                    continue;
                }
                // The face lies on the singular line's side: the join runs along it in
                // the direction that keeps the face on its left.
                let sign = if k == 0 { s.side } else { -s.side };
                let delta = match periods[k] {
                    Some(period) => {
                        let delta = (sign * (start[k] - end[k])).rem_euclid(period);
                        // Meeting at the same chart point: the loop runs up a seam and back.
                        if delta <= 1e-9 * period {
                            period
                        } else {
                            delta
                        }
                    }
                    None => sign * (start[k] - end[k]),
                };
                if delta <= 0. {
                    return Err(c.error(
                        ErrorKind::InvalidGeometry,
                        "face loop runs against its surface along a pole or apex line",
                    ));
                }
                let mut from = end;
                from[j] = s.value;
                let (index, pcurve) = self.collapsed_edge(c, v, from, k, sign * delta)?;
                pcurves.insert(index, pcurve);
                joined.push(Use {
                    edge: index,
                    forward: true,
                });
            }
            l.uses = joined;
        }
        // A loop that runs along an edge and straight back is a slit unless a collapsed
        // edge separates the two uses at a pole or apex.
        for l in &loops {
            let n = l.uses.len();
            for i in 0..n {
                let (u, next) = (l.uses[i], l.uses[(i + 1) % n]);
                if n > 1 && u.edge == next.edge && u.forward != next.forward {
                    return Err(c.error(
                        // Only a chart with singular points can make it valid.
                        if singular.is_empty() {
                            ErrorKind::InvalidGeometry
                        } else {
                            ErrorKind::Unsupported
                        },
                        "face loop doubles back along an edge away from a pole or apex of the surface chart",
                    ));
                }
            }
        }
        // VERTEX_LOOP vertices must lie at singular points of the chart.
        let mut poles = Vec::new();
        for l in &loops {
            if let Some(v) = l.vertex {
                match at_singular(c, v) {
                    Some(s) => poles.push((s, v)),
                    None => {
                        return Err(c.error(
                            ErrorKind::Unsupported,
                            "VERTEX_LOOP vertex is not at a pole or apex of the face chart",
                        ));
                    }
                }
            }
        }
        let edge_loops: Vec<usize> = (0..loops.len())
            .filter(|&i| loops[i].vertex.is_none())
            .collect();
        let mut charts = BTreeMap::new();
        for &i in &edge_loops {
            let chart = self
                .chart_loop(face, &loops[i].uses, &pcurves)
                .map_err(|e| self.pcurve_error(c, loops[i].uses[0].edge, e))?;
            charts.insert(i, chart);
        }
        let wrapping: Vec<usize> = edge_loops
            .iter()
            .copied()
            .filter(|i| charts[i].winding != [0, 0])
            .collect();
        if wrapping.is_empty() && poles.is_empty() {
            let explicit: Vec<usize> = edge_loops
                .iter()
                .copied()
                .filter(|&i| loops[i].outer)
                .collect();
            let outer = match explicit.len() {
                1 => explicit[0],
                0 if edge_loops.len() == 1 => edge_loops[0],
                0 => {
                    let ccw: Vec<usize> = edge_loops
                        .iter()
                        .copied()
                        .filter(|i| signed_area(&charts[i].polygon) > 0.)
                        .collect();
                    if ccw.len() != 1 {
                        return Err(c.error(
                            ErrorKind::InvalidGeometry,
                            format!(
                                "ambiguous outer bound: no FACE_OUTER_BOUND and {} of {} loops are counterclockwise in the surface chart",
                                ccw.len(),
                                edge_loops.len()
                            ),
                        ));
                    }
                    self.adaptations.inferred_outer_bounds += 1;
                    ccw[0]
                }
                _ => return Err(c.error(ErrorKind::InvalidGeometry, "multiple outer bounds")),
            };
            return Ok(Planned::Face(Plan {
                surface: None,
                outer: loops[outer].uses.clone(),
                holes: edge_loops
                    .iter()
                    .filter(|&&i| i != outer)
                    .map(|&i| loops[i].uses.clone())
                    .collect(),
                pcurves,
            }));
        }
        let rotate = |uses: &[Use], start: usize| -> Vec<Use> {
            uses[start..]
                .iter()
                .chain(&uses[..start])
                .copied()
                .collect()
        };
        // Is the chart point `uv` covered by a hole's sampled chart box along `k`
        // (modulo its period) and, along the other axis, within `[lo, hi]`?
        let hole_blocks = |holes: &[usize], k: usize, at: f64, lo: f64, hi: f64| {
            let j = 1 - k;
            holes.iter().any(|h| {
                let polygon = &charts[h].polygon;
                let (kmin, kmax) = minmax(&polygon.iter().map(|p| p[k]).collect::<Vec<_>>());
                let (jmin, jmax) = minmax(&polygon.iter().map(|p| p[j]).collect::<Vec<_>>());
                let at = periods[k].map_or(at, |p| nearest(at, 0.5 * (kmin + kmax), p));
                at >= kmin && at <= kmax && hi >= jmin && lo <= jmax
            })
        };
        if allow_singular
            && wrapping.len() == 1
            && charts[&wrapping[0]]
                .winding
                .iter()
                .filter(|&&w| w != 0)
                .count()
                == 1
        {
            // One loop around a periodic axis encloses the pole or apex on its left:
            // a seam from one of its vertices to that singular point, and a collapsed
            // edge along the singular line, close the chart.
            let a = wrapping[0];
            let winding = charts[&a].winding;
            let k = (0..2).find(|&k| winding[k] != 0).expect("wrapping axis");
            let (j, w) = (1 - k, winding[k]);
            let period = periods[k].expect("wrapping axis is periodic");
            let direction = if k == 0 { 1. } else { -1. };
            let up = w as f64 * direction > 0.;
            let (jmin, jmax) = minmax(&charts[&a].polygon.iter().map(|p| p[j]).collect::<Vec<_>>());
            let enclosed = singular
                .iter()
                .copied()
                .filter(|s| {
                    s.k == k
                        && if up {
                            s.side < 0. && s.value >= jmax
                        } else {
                            s.side > 0. && s.value <= jmin
                        }
                })
                .min_by(|x, y| {
                    (x.value - 0.5 * (jmin + jmax))
                        .abs()
                        .total_cmp(&(y.value - 0.5 * (jmin + jmax)).abs())
                });
            let Some(s) = enclosed.filter(|_| w.abs() == 1) else {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "a face loop winds around a periodic surface direction without enclosing a pole or apex, or winds more than once",
                ));
            };
            let pole = Self::pole_vertex(c, face, &mut poles, &s)?;
            if !poles.is_empty() {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "VERTEX_LOOP vertex is not the pole or apex enclosed by the face",
                ));
            }
            let holes: Vec<usize> = edge_loops.iter().copied().filter(|&i| i != a).collect();
            for (ia, ua) in charts[&a].starts.clone().into_iter().enumerate() {
                c.charge(1)?;
                if hole_blocks(&holes, k, ua[k], ua[j].min(s.value), ua[j].max(s.value)) {
                    continue;
                }
                let u = loops[a].uses[ia];
                let e = &self.edges[u.edge];
                let va = if u.forward {
                    e.vertices[0]
                } else {
                    e.vertices[1]
                };
                let Some((seam, forward, seam_pcurve)) = self.iso_seam(
                    c,
                    &face.chart,
                    k,
                    ua[k],
                    (va, ua[j]),
                    (pole, s.value),
                    tolerance,
                )?
                else {
                    continue;
                };
                let mut from = [0.; 2];
                from[k] = ua[k] + w as f64 * period;
                from[j] = s.value;
                let (collapsed, collapsed_pcurve) =
                    self.collapsed_edge(c, pole, from, k, -(w as f64) * period)?;
                pcurves.insert(seam, seam_pcurve);
                pcurves.insert(collapsed, collapsed_pcurve);
                let mut outer = rotate(&loops[a].uses, ia);
                outer.extend([
                    Use {
                        edge: seam,
                        forward,
                    },
                    Use {
                        edge: collapsed,
                        forward: true,
                    },
                    Use {
                        edge: seam,
                        forward: !forward,
                    },
                ]);
                return Ok(Planned::Face(Plan {
                    surface: None,
                    outer,
                    holes: holes.iter().map(|&i| loops[i].uses.clone()).collect(),
                    pcurves,
                }));
            }
            return Err(c.error(
                ErrorKind::Unsupported,
                "no seam from the face loop to its enclosed pole or apex avoids the face's holes",
            ));
        }
        if wrapping.is_empty() {
            // A face bounded only by VERTEX_LOOPs (and holes) covers the whole periodic
            // band between two singular lines, such as a full sphere.
            let k = poles[0].0.k;
            if periods[k].is_none() {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "VERTEX_LOOP bounds on a non-periodic chart are not supported",
                ));
            }
            let line = |side: f64| {
                singular
                    .iter()
                    .copied()
                    .find(|s| s.k == k && s.side == side)
            };
            let (Some(bottom), Some(top)) = (line(1.), line(-1.)) else {
                return Err(c.error(
                    ErrorKind::Unsupported,
                    "a face bounded by VERTEX_LOOPs needs a chart between two poles",
                ));
            };
            let b = Self::pole_vertex(c, face, &mut poles, &bottom)?;
            let t = Self::pole_vertex(c, face, &mut poles, &top)?;
            if !poles.is_empty() {
                return Err(c.error(ErrorKind::Unsupported, "repeated VERTEX_LOOP at one pole"));
            }
            let period = periods[k].expect("singular lines run along periodic axes");
            let sign = if k == 0 { 1. } else { -1. };
            let mut candidates = vec![0., 0.25 * period, 0.5 * period, 0.75 * period];
            for i in &edge_loops {
                let (_, kmax) = minmax(&charts[i].polygon.iter().map(|p| p[k]).collect::<Vec<_>>());
                candidates.push(kmax + 1e-3 * period);
            }
            for at in candidates {
                c.charge(1)?;
                if hole_blocks(&edge_loops, k, at, bottom.value, top.value) {
                    continue;
                }
                let Some((seam, forward, seam_pcurve)) = self.iso_seam(
                    c,
                    &face.chart,
                    k,
                    at,
                    (b, bottom.value),
                    (t, top.value),
                    tolerance,
                )?
                else {
                    continue;
                };
                let mut from = [0.; 2];
                from[k] = at;
                from[1 - k] = bottom.value;
                let (low, low_pcurve) = self.collapsed_edge(c, b, from, k, sign * period)?;
                from[k] = at + sign * period;
                from[1 - k] = top.value;
                let (high, high_pcurve) = self.collapsed_edge(c, t, from, k, -sign * period)?;
                pcurves.extend([(seam, seam_pcurve), (low, low_pcurve), (high, high_pcurve)]);
                return Ok(Planned::Face(Plan {
                    surface: None,
                    outer: vec![
                        Use {
                            edge: low,
                            forward: true,
                        },
                        Use {
                            edge: seam,
                            forward,
                        },
                        Use {
                            edge: high,
                            forward: true,
                        },
                        Use {
                            edge: seam,
                            forward: !forward,
                        },
                    ],
                    holes: edge_loops.iter().map(|&i| loops[i].uses.clone()).collect(),
                    pcurves,
                }));
            }
            return Err(c.error(
                ErrorKind::Unsupported,
                "no pole-to-pole seam avoids the face's holes",
            ));
        }
        if !poles.is_empty() {
            return Err(c.error(
                ErrorKind::Unsupported,
                "VERTEX_LOOP bounds on an annular face are not supported",
            ));
        }
        let periodic = |k: usize| periods[k].is_some();
        let pair = match wrapping.as_slice() {
            &[a, b] => {
                let (wa, wb) = (charts[&a].winding, charts[&b].winding);
                let axis = (0..2).find(|&k| wa[k] != 0);
                match axis {
                    Some(k)
                        if wa[1 - k] == 0
                            && wb[1 - k] == 0
                            && wa[k].abs() == 1
                            && wb[k] == -wa[k] =>
                    {
                        Some(if wa[k] == 1 { (a, b, k) } else { (b, a, k) })
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let Some((a, b, k)) = pair else {
            return Err(c.error(
                ErrorKind::Unsupported,
                format!(
                    "{} face loops wind around a periodic surface direction; faces that wrap more than once are not supported",
                    wrapping.len()
                ),
            ));
        };
        let holes: Vec<usize> = edge_loops
            .iter()
            .copied()
            .filter(|&i| i != a && i != b)
            .collect();
        let annulus = FaceRecord {
            loops: loops.clone(),
            ..face.clone()
        };
        let chart_list: Vec<LoopChart> = (0..loops.len())
            .map(|i| {
                charts.remove(&i).unwrap_or(LoopChart {
                    starts: Vec::new(),
                    winding: [0, 0],
                    polygon: Vec::new(),
                })
            })
            .collect();
        let (seam, ia, ib, range_j, forward) = match self.seam(
            c,
            &annulus,
            &chart_list,
            &pcurves,
            (a, b, k),
            &holes,
            periodic(1 - k),
            tolerance,
        )? {
            SeamChoice::Seam(seam, ia, ib, range, forward) => (seam, ia, ib, range, forward),
            SeamChoice::Split { edge, parameter } => {
                return Ok(Planned::Split { edge, parameter });
            }
        };
        let index = self.edges.len();
        self.edges.push(*seam);
        let mut outer = rotate(&loops[a].uses, ia);
        outer.push(Use {
            edge: index,
            forward,
        });
        outer.extend(rotate(&loops[b].uses, ib));
        outer.push(Use {
            edge: index,
            forward: !forward,
        });
        let origin = if k == 0 {
            [chart_list[a].starts[ia][0], 0.]
        } else {
            [0., chart_list[a].starts[ia][1]]
        };
        let tangent = if k == 0 { [0., 1.] } else { [1., 0.] };
        pcurves.insert(
            index,
            Pcurve {
                curve: CurveGeometry::Analytic(c.checked(Curve::line(
                    c.checked(Point::<ParameterSpace, 2>::new(origin))?,
                    c.checked(Vector::new(tangent))?,
                ))?),
                range: range_j,
            },
        );
        self.adaptations.inserted_seams += 1;
        Ok(Planned::Face(Plan {
            surface: None,
            outer,
            holes: holes.iter().map(|&i| loops[i].uses.clone()).collect(),
            pcurves,
        }))
    }

    /// An isoparametric seam from a vertex of loop `a` (winding +1 along axis `k`) to an
    /// aligned vertex of loop `b`, on the side where the face lies (left of `a`).
    /// Without such a pair, it requests a split of the loop `b` edge that the
    /// isoparametric line through a vertex of `a` crosses.
    #[allow(clippy::too_many_arguments)]
    fn seam(
        &mut self,
        c: &mut Context<'_, '_>,
        face: &FaceRecord,
        charts: &[LoopChart],
        pcurves: &BTreeMap<usize, Pcurve>,
        (a, b, k): (usize, usize, usize),
        holes: &[usize],
        j_periodic: bool,
        tolerance: f64,
    ) -> Result<SeamChoice, Error> {
        let j = 1 - k;
        let direction = if k == 0 { 1. } else { -1. };
        let period = face.chart.periods()[k].expect("wrapping axis is periodic");
        let vertex = |l: usize, i: usize| {
            let u = face.loops[l].uses[i];
            let e = &self.edges[u.edge];
            if u.forward {
                e.vertices[0]
            } else {
                e.vertices[1]
            }
        };
        let target_j = |ua: [f64; 2], uj: f64| {
            if j_periodic {
                ua[j] + direction * (direction * (uj - ua[j])).rem_euclid(TAU)
            } else {
                uj
            }
        };
        let blocked = |ua: [f64; 2], bj: f64| {
            direction * (bj - ua[j]) <= 0.
                || holes.iter().any(|&h| {
                    let hk: Vec<f64> = charts[h].polygon.iter().map(|p| p[k]).collect();
                    let hj: Vec<f64> = charts[h].polygon.iter().map(|p| p[j]).collect();
                    let (kmin, kmax) = minmax(&hk);
                    let (jmin, jmax) = minmax(&hj);
                    let seam_k = nearest(ua[k], 0.5 * (kmin + kmax), period);
                    let (lo, hi) = (ua[j].min(bj), ua[j].max(bj));
                    seam_k >= kmin && seam_k <= kmax && hi >= jmin && lo <= jmax
                })
        };
        for (ia, ua) in charts[a].starts.iter().enumerate() {
            for (ib, ub) in charts[b].starts.iter().enumerate() {
                c.charge(1)?;
                let bj = target_j(*ua, ub[j]);
                if blocked(*ua, bj) {
                    continue;
                }
                let Some((curve, shape)) = seam_curve(&face.chart, k, ua[k]) else {
                    continue;
                };
                let (va, vb) = (vertex(a, ia), vertex(b, ib));
                let at = |t: f64| curve.evaluate(t).map(|e| e.position.coordinates());
                let (Ok(pa), Ok(pb)) = (at(ua[j]), at(bj)) else {
                    continue;
                };
                let position = |v: VertexId| c.raw.vertices[v.0].position.coordinates();
                if norm(sub(pa, position(va))) > tolerance
                    || norm(sub(pb, position(vb))) > tolerance
                {
                    continue;
                }
                let forward = direction > 0.;
                let range = [ua[j].min(bj), ua[j].max(bj)];
                c.record()?;
                return Ok(SeamChoice::Seam(
                    Box::new(EdgeRecord {
                        entity: None,
                        curve,
                        shape,
                        range,
                        vertices: if forward { [va, vb] } else { [vb, va] },
                    }),
                    ia,
                    ib,
                    range,
                    forward,
                ));
            }
        }
        // No aligned pair: split the loop-b edge crossed by the isoparametric line
        // through a loop-a vertex, choosing the first crossing whose seam avoids holes.
        for ua in &charts[a].starts {
            for u in &face.loops[b].uses {
                c.charge(1)?;
                let Some((parameter, uv)) = crossing(&pcurves[&u.edge], k, ua[k], period) else {
                    continue;
                };
                if !blocked(*ua, target_j(*ua, uv[j])) {
                    return Ok(SeamChoice::Split {
                        edge: u.edge,
                        parameter,
                    });
                }
            }
        }
        Err(c.error(
            ErrorKind::Unsupported,
            "annular face on a periodic surface has no seam between its loops that avoids its holes",
        ))
    }

    /// Split an edge at an interior curve parameter. Both halves keep the curve; every
    /// use in every face is replaced by the two halves in traversal order.
    fn split(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &mut [FaceRecord],
        edge: usize,
        parameter: f64,
    ) -> Result<(), Error> {
        let [a, b] = self.edges[edge].range;
        if !(parameter > a && parameter < b) {
            return Err(c.error(
                ErrorKind::InvalidGeometry,
                "edge split parameter outside the edge range",
            ));
        }
        c.record()?;
        let position = c
            .checked(self.edges[edge].curve.evaluate(parameter))?
            .position;
        let vertex = VertexId(c.raw.vertices.len());
        c.raw.vertices.push(Vertex { position });
        c.point_entities
            .push(self.edges[edge].entity.unwrap_or(c.current));
        let [start, end] = self.edges[edge].vertices;
        let mut second = self.edges[edge].clone();
        second.range = [parameter, b];
        second.vertices = [vertex, end];
        let first = &mut self.edges[edge];
        first.range = [a, parameter];
        first.vertices = [start, vertex];
        let index = self.edges.len();
        self.edges.push(second);
        for face in faces.iter_mut() {
            for l in &mut face.loops {
                let mut uses = Vec::with_capacity(l.uses.len() + 1);
                for u in &l.uses {
                    c.charge(1)?;
                    if u.edge != edge {
                        uses.push(*u);
                    } else if u.forward {
                        uses.extend([
                            *u,
                            Use {
                                edge: index,
                                forward: true,
                            },
                        ]);
                    } else {
                        uses.extend([
                            Use {
                                edge: index,
                                forward: false,
                            },
                            *u,
                        ]);
                    }
                }
                l.uses = uses;
            }
        }
        self.adaptations.split_edges += 1;
        Ok(())
    }
}

/// Chart point at fraction `s` along a use, in its pcurve's own chart.
fn use_uv(pcurve: &Pcurve, u: Use, s: f64) -> Result<[f64; 2], PcurveError> {
    let s = if u.forward { s } else { 1. - s };
    let t = if s == 0. {
        pcurve.range[0]
    } else if s == 1. {
        pcurve.range[1]
    } else {
        pcurve.range[0] + s * (pcurve.range[1] - pcurve.range[0])
    };
    Ok(pcurve
        .curve
        .evaluate(t)
        .map_err(|_| PcurveError::Geometry)?
        .position
        .coordinates())
}
fn minmax(values: &[f64]) -> (f64, f64) {
    values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
            (lo.min(x), hi.max(x))
        })
}
fn signed_area(polygon: &[[f64; 2]]) -> f64 {
    let n = polygon.len();
    (0..n)
        .map(|i| {
            let (p, q) = (polygon[i], polygon[(i + 1) % n]);
            p[0] * q[1] - q[0] * p[1]
        })
        .sum::<f64>()
        * 0.5
}

/// A sphere chart with the given centre, unit axis and radius.
fn sphere_chart(center: V3, axis: V3, radius: f64) -> Option<(Chart, SurfaceGeometry)> {
    let helper = if axis[0].abs() < 0.9 {
        [1., 0., 0.]
    } else {
        [0., 1., 0.]
    };
    let x = crate::pcurve::cross(helper, axis);
    let frame = PlaneFrame::new(
        Point::new(center).ok()?,
        Vector::new(scale(x, 1. / norm(x))).ok()?,
        Vector::new(crate::pcurve::cross(axis, x)).ok()?,
        NumericalTolerance::default(),
    )
    .ok()?;
    let surface = Surface::sphere(frame, Length::metres(radius).ok()?).ok()?;
    Some((
        Chart::Sphere(Frame::new(frame), radius),
        SurfaceGeometry::Analytic(surface),
    ))
}

/// The isoparametric curve with fixed coordinate `k` = `value`, parameterized by the
/// other surface coordinate, so its pcurve is an axis-aligned UV line.
fn seam_curve(
    chart: &Chart,
    k: usize,
    value: f64,
) -> Option<(CurveGeometry<ModelSpace, 3>, CurveShape)> {
    let point = |p: V3| Point::<ModelSpace, 3>::new(p).ok();
    let vector = |v: V3| Vector::<ModelSpace, 3>::new(v).ok();
    let circle =
        |o: V3, x: V3, y: V3, r: f64| -> Option<(CurveGeometry<ModelSpace, 3>, CurveShape)> {
            let frame = PlaneFrame::new(
                point(o)?,
                vector(x)?,
                vector(y)?,
                NumericalTolerance::default(),
            )
            .ok()?;
            let curve = Curve::circle(frame, Length::metres(r).ok()?).ok()?;
            Some((
                CurveGeometry::Analytic(curve),
                CurveShape::Conic {
                    frame: Frame::new(frame),
                    a: r,
                    b: r,
                },
            ))
        };
    let line = |o: V3, t: V3| -> Option<(CurveGeometry<ModelSpace, 3>, CurveShape)> {
        Some((
            CurveGeometry::Analytic(Curve::line(point(o)?, vector(t)?).ok()?),
            CurveShape::Line {
                origin: o,
                tangent: t,
            },
        ))
    };
    match (chart, k) {
        (Chart::Cylinder(f, r), 0) => {
            let radial = add(scale(f.x, value.cos()), scale(f.y, value.sin()));
            line(add(f.o, scale(radial, *r)), f.z)
        }
        (Chart::Cone(f, alpha), 0) => {
            let radial = add(scale(f.x, value.cos()), scale(f.y, value.sin()));
            line(
                f.o,
                add(scale(radial, alpha.sin()), scale(f.z, alpha.cos())),
            )
        }
        (Chart::Sphere(f, r), 0) => {
            let radial = add(scale(f.x, value.cos()), scale(f.y, value.sin()));
            circle(f.o, radial, f.z, *r)
        }
        (Chart::Torus(f, major, minor), 0) => {
            let radial = add(scale(f.x, value.cos()), scale(f.y, value.sin()));
            circle(add(f.o, scale(radial, *major)), radial, f.z, *minor)
        }
        (Chart::Torus(f, major, minor), 1) => circle(
            add(f.o, scale(f.z, minor * value.sin())),
            f.x,
            f.y,
            major + minor * value.cos(),
        ),
        _ => None,
    }
}

/// Closest-parameter inversion on a NURBS curve: sampled seed plus Newton iteration.
fn nurbs_parameter(
    c: &Context<'_, '_>,
    n: &NurbsCurve<ModelSpace, 3>,
    p: V3,
    at_end: bool,
) -> Result<f64, Error> {
    let [lo, hi] = n.knot_vector().domain();
    let mut breaks: Vec<f64> = n
        .knot_vector()
        .knots()
        .iter()
        .copied()
        .filter(|&t| t >= lo && t <= hi)
        .collect();
    breaks.dedup();
    let eval = |t: f64| {
        n.evaluate(t)
            .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))
    };
    let mut best = (f64::INFINITY, lo);
    for w in breaks.windows(2) {
        for i in 0..=8 {
            let t = w[0] + (w[1] - w[0]) * i as f64 / 8.;
            let d = norm(sub(eval(t)?.position.coordinates(), p));
            // Prefer the requested end on closed curves: ties go to the later sample.
            if d < best.0 || (at_end && d <= best.0) {
                best = (d, t);
            }
        }
    }
    let mut t = best.1;
    for _ in 0..32 {
        let e = eval(t)?;
        let r = sub(e.position.coordinates(), p);
        let d1 = e.first.components();
        let d2 = e.second.components();
        let f = dot(r, d1);
        let df = dot(d1, d1) + dot(r, d2);
        if df.is_nan() || df <= 0. {
            break;
        }
        let next = (t - f / df).clamp(lo, hi);
        if next == t {
            break;
        }
        let closer = norm(sub(eval(next)?.position.coordinates(), p)) <= norm(r);
        if !closer {
            break;
        }
        t = next;
    }
    Ok(t)
}
