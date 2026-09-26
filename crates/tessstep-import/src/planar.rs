use super::*;

/// Import one selected MANIFOLD_SOLID_BREP with planar faces and straight LINE
/// EDGE_CURVEs. Shared vertices/edges retain VERTEX_POINT/EDGE_CURVE identity.
/// One plain FACE_BOUND is an unambiguous outer bound; multiple bounds require
/// one explicit FACE_OUTER_BOUND. No coordinate welding, healing or unit inference.
/// ORIENTED_EDGE endpoint slots must be `*`; their values come from EDGE_ELEMENT.
pub fn import_planar_solid(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    tolerance: ModelTolerance,
    limits: ImportLimits,
) -> Result<ImportedSolid, Error> {
    import_solid(document, root, unit, tolerance, limits, true)
}
impl<'d, 'a> Context<'d, 'a> {
    fn topological_vertex(&mut self, v: &'d EntityView<'a>) -> Result<VertexId, Error> {
        if let Some(&id) = self.points.get(&v.id) {
            return Ok(id);
        }
        self.current = v.id;
        self.record()?;
        let point = self.point(self.reference(v, "VERTEX_GEOMETRY")?)?;
        let id = VertexId(self.raw.vertices.len());
        self.raw.vertices.push(Vertex { position: point });
        self.points.insert(v.id, id);
        self.point_entities.push(v.id);
        Ok(id)
    }
    fn line_edge(&mut self, edge: &'d EntityView<'a>) -> Result<(EdgeId, bool), Error> {
        if let Some(&found) = self.edge_entities.get(&edge.id) {
            return Ok(found);
        }
        let start = self.topological_vertex(self.reference(edge, "EDGE_START")?)?;
        let end = self.topological_vertex(self.reference(edge, "EDGE_END")?)?;
        self.current = edge.id;
        let same = self.boolean(edge, "SAME_SENSE")?;
        let line = self.reference(edge, "EDGE_GEOMETRY")?;
        let origin = self.point(self.reference(line, "PNT")?)?;
        let vector = self.reference(line, "DIR")?;
        let direction = self.direction(self.reference(vector, "ORIENTATION")?)?;
        let magnitude = match self.attr(vector, "MAGNITUDE")?.kind {
            ValueKind::Real(v) => v,
            ValueKind::Integer(v) => v as f64,
            _ => return Err(self.error(ErrorKind::InvalidGeometry, "expected vector magnitude")),
        };
        if magnitude <= 0. {
            return Err(self.error(
                ErrorKind::InvalidGeometry,
                "LINE requires positive vector magnitude",
            ));
        }
        let magnitude = self.checked(self.unit.to_metres(magnitude))?;
        let curve = self.checked(Curve::line(
            origin,
            self.checked(direction.scaled(magnitude))?,
        ))?;
        let mut parameters = [0.; 2];
        for (u, vertex) in parameters.iter_mut().zip([start, end]) {
            let point = self.raw.vertices[vertex.0].position;
            *u = self.checked(self.checked(point.difference(origin))?.dot(direction))? / magnitude;
            let evaluated = self.checked(curve.evaluate(*u))?.position;
            if self.checked(evaluated.distance(point))? > self.tolerance.distance().as_metres() {
                return Err(self.error(
                    ErrorKind::InvalidGeometry,
                    "EDGE_CURVE endpoint is off its declared LINE",
                ));
            }
        }
        let [a, b] = parameters;
        if (same && a >= b) || (!same && a <= b) {
            return Err(self.error(
                ErrorKind::InvalidGeometry,
                "EDGE_CURVE same_sense contradicts LINE parameter direction",
            ));
        }
        self.record()?;
        let id = EdgeId(self.raw.edges.len());
        self.raw.edges.push(Edge {
            vertices: if same { [start, end] } else { [end, start] },
            curve: CurveGeometry::Analytic(curve),
            range: [a.min(b), a.max(b)],
        });
        self.edge_entities.insert(edge.id, (id, same));
        Ok((id, same))
    }
    pub(super) fn edge_wire(
        &mut self,
        wire: &'d EntityView<'a>,
        frame: PlaneFrame<ModelSpace, 3>,
        reverse: bool,
    ) -> Result<WireId, Error> {
        let mut uses = Vec::new();
        for value in self.aggregate(wire, "EDGE_LIST")? {
            self.charge(1)?;
            let oriented = self.target(value)?;
            let forward = self.boolean(oriented, "ORIENTATION")?;
            let (edge, same) = self.line_edge(self.reference(oriented, "EDGE_ELEMENT")?)?;
            let source = &self.raw.edges[edge.0];
            let [a, b] = source.range;
            self.current = oriented.id;
            let pa = self.checked(source.curve.evaluate(a))?.position;
            let pb = self.checked(source.curve.evaluate(b))?.position;
            let ua = self.uv(pa, frame)?;
            let ub = self.uv(pb, frame)?;
            // Pcurve range 0..1 maps affinely to the trimmed LINE range a..b.
            let curve = self.checked(Curve::line(
                self.checked(Point::new(ua))?,
                self.checked(Vector::new([ub[0] - ua[0], ub[1] - ua[1]]))?,
            ))?;
            self.record()?;
            uses.push(CoedgeId(self.raw.coedges.len()));
            self.raw.coedges.push(Coedge {
                edge,
                orientation: if (forward == same) != reverse {
                    Orientation::Forward
                } else {
                    Orientation::Reversed
                },
                pcurve: Some(Pcurve {
                    curve: CurveGeometry::Analytic(curve),
                    range: [0., 1.],
                }),
            });
        }
        if reverse {
            uses.reverse();
        }
        self.record()?;
        let id = WireId(self.raw.wires.len());
        self.raw.wires.push(Wire { coedges: uses });
        Ok(id)
    }
}
