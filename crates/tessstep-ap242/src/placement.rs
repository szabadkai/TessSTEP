use super::*;
use std::collections::BTreeSet;
use tessstep_math::{Direction3, ModelSpace, Point3};

type Direction = Direction3<ModelSpace>;
impl<'d, 'a> Context<'d, 'a> {
    fn math<T>(&self, value: Result<T, tessstep_math::Error>) -> Result<T, Error> {
        value.map_err(|math_error| Error {
            math_error: Some(math_error),
            ..self.error(ErrorKind::Placement, "invalid placement arithmetic")
        })
    }
    fn view(&mut self, id: u64) -> Result<&'d EntityView<'a>, Error> {
        self.tick()?;
        EntityId::new(id)
            .and_then(|id| self.decoded.get(id))
            .ok_or_else(|| self.error(ErrorKind::TypeMismatch, "missing placement entity"))
    }
    fn optional_ref(
        &mut self,
        view: &'d EntityView<'a>,
        name: &'static str,
    ) -> Result<Option<&'d EntityView<'a>>, Error> {
        let value = self.attr(view, name)?;
        if matches!(value.kind, ValueKind::Null) {
            Ok(None)
        } else {
            self.ref_value(value).map(Some)
        }
    }
    fn triple(&mut self, view: &'d EntityView<'a>, name: &'static str) -> Result<[f64; 3], Error> {
        let values = self.aggregate(view, name)?;
        if values.len() != 3 {
            return Err(self.error(ErrorKind::Placement, "placement needs three coordinates"));
        }
        Ok([
            self.number(&values[0])?,
            self.number(&values[1])?,
            self.number(&values[2])?,
        ])
    }
    fn direction(&mut self, view: &'d EntityView<'a>) -> Result<Direction, Error> {
        self.expect(view, "direction")?;
        let values = self.triple(view, "direction_ratios")?;
        self.math(Direction::new(values))
    }
    fn axis(
        &mut self,
        view: &'d EntityView<'a>,
        name: &'static str,
    ) -> Result<Option<Direction>, Error> {
        self.optional_ref(view, name)?
            .map(|v| self.direction(v))
            .transpose()
    }
    fn point(&mut self, view: &'d EntityView<'a>, unit: Unit) -> Result<Point3<ModelSpace>, Error> {
        self.expect(view, "cartesian_point")?;
        let mut xyz = self.triple(view, "coordinates")?;
        for v in &mut xyz {
            *v = unit
                .to_si(*v)
                .map_err(|_| self.error(ErrorKind::Units, "placement length conversion range"))?;
        }
        self.math(Point3::new(xyz))
    }
    // STEP first_proj_axis: choose Y only when the normalized Z is exactly +/-X.
    // Near-parallel explicit/default axes are rejected by the math numerical policy.
    fn frame(&mut self, view: &'d EntityView<'a>, unit: Unit) -> Result<Transform, Error> {
        self.expect(view, "axis2_placement_3d")?;
        let point = self.reference(view, "location")?;
        let origin = self.point(point, unit)?;
        let z = self
            .axis(view, "axis")?
            .unwrap_or(self.math(Direction::new([0., 0., 1.]))?);
        let default_x = if z.vector().components()[1..] == [0., 0.] {
            [0., 1., 0.]
        } else {
            [1., 0., 0.]
        };
        let x = self
            .axis(view, "ref_direction")?
            .unwrap_or(self.math(Direction::new(default_x))?);
        self.math(Transform::from_frame(origin, z, x, Default::default()))
    }
    fn scale(
        &mut self,
        view: &'d EntityView<'a>,
        name: &'static str,
        default: f64,
    ) -> Result<f64, Error> {
        let v = self.attr(view, name)?;
        let n = if matches!(v.kind, ValueKind::Null) {
            default
        } else {
            self.number(v)?
        };
        if !n.is_finite() || n <= 0. {
            return Err(self.error(
                ErrorKind::Placement,
                "operator scale must be positive and finite",
            ));
        }
        Ok(n)
    }
    fn operator(&mut self, view: &'d EntityView<'a>, unit: Unit) -> Result<Transform, Error> {
        self.expect(view, "cartesian_transformation_operator_3d")?;
        let origin = self.reference(view, "local_origin")?;
        let origin = self.point(origin, unit)?;
        let z = self
            .axis(view, "axis3")?
            .unwrap_or(self.math(Direction::new([0., 0., 1.]))?);
        let default_x = if z.vector().components()[1..] == [0., 0.] {
            [0., 1., 0.]
        } else {
            [1., 0., 0.]
        };
        let x = self
            .axis(view, "axis1")?
            .unwrap_or(self.math(Direction::new(default_x))?);
        let frame = self.math(Transform::from_frame(origin, z, x, Default::default()))?;
        let x = self.math(Direction::new(frame.linear().map(|row| row[0])))?;
        let y_arg = self
            .axis(view, "axis2")?
            .unwrap_or(self.math(Direction::new([0., 1., 0.]))?);
        // second_proj_axis removes the Z and X components; this preserves reflection.
        let v = y_arg.vector();
        let projected =
            self.math(v.subtracted(self.math(z.vector().scaled(self.math(v.dot(z.vector()))?))?))?;
        let projected = self.math(
            projected.subtracted(self.math(x.vector().scaled(self.math(v.dot(x.vector()))?))?),
        )?;
        if self.math(projected.norm())? <= tessstep_math::NumericalTolerance::default().relative() {
            return Err(self.error(ErrorKind::Placement, "operator second axis is degenerate"));
        }
        let y = self.math(projected.normalized())?;
        let s1 = self.scale(view, "scale", 1.)?;
        let (s2, s3) = if self.is(view, "cartesian_transformation_operator_3d_non_uniform")? {
            (
                self.scale(view, "scale2", s1)?,
                self.scale(view, "scale3", s1)?,
            )
        } else {
            (s1, s1)
        };
        let columns = [
            self.math(x.vector().scaled(s1))?.components(),
            self.math(y.vector().scaled(s2))?.components(),
            self.math(z.vector().scaled(s3))?.components(),
        ];
        let t = self.math(Transform::new(
            std::array::from_fn(|i| std::array::from_fn(|j| columns[j][i])),
            origin.coordinates(),
        ))?;
        self.math(t.inverse(Default::default()))?;
        Ok(t)
    }
    pub(super) fn uncertainties(
        &mut self,
        context: &'d EntityView<'a>,
        id: ContextId,
        parts: &mut Parts,
    ) -> Result<(), Error> {
        if !self.is(context, "global_uncertainty_assigned_context")? {
            return Ok(());
        }
        for v in self.aggregate(context, "uncertainty")? {
            self.tick()?;
            let measure = self.ref_value(v)?;
            self.expect(measure, "uncertainty_measure_with_unit")?;
            let unit = self.reference(measure, "unit_component")?;
            let unit = self.unit(unit)?;
            let value = self.attr(measure, "value_component")?;
            let n = self.number(value)?;
            if n <= 0. {
                return Err(self.error(ErrorKind::Units, "uncertainty must be positive"));
            }
            let value_si = unit
                .to_si(n)
                .map_err(|_| self.error(ErrorKind::Units, "uncertainty conversion range"))?;
            let value = self.attr(measure, "description")?;
            let description = if matches!(value.kind, ValueKind::Null) {
                None
            } else {
                Some(self.string(measure, "description")?)
            };
            parts.uncertainties.push(Uncertainty {
                source: measure.id.get(),
                context: id,
                dimension: unit.dimension(),
                value_si,
                name: self.string(measure, "name")?,
                description,
            });
        }
        Ok(())
    }
    pub(super) fn finish_placements(&mut self, parts: &mut Parts) -> Result<(), Error> {
        // Only references between representation items propagate context. A reference
        // to a representation_map must not leak its source coordinates into the user.
        let mut dependencies: BTreeMap<ItemId, Vec<ItemId>> = BTreeMap::new();
        for view in self.decoded.entities() {
            if !self.is(view, "representation_item")? {
                continue;
            }
            for a in &view.attributes {
                let mut stack = vec![a.value];
                while let Some(value) = stack.pop() {
                    self.tick()?;
                    match &value.kind {
                        ValueKind::Reference(id) => {
                            let target = self.view(id.get())?;
                            if self.is(target, "representation_item")? {
                                let edge = (ItemId(view.id.get()), ItemId(id.get()));
                                parts.item_dependencies.push(edge);
                                dependencies.entry(edge.0).or_default().push(edge.1);
                            }
                        }
                        ValueKind::Aggregate(values) => {
                            for v in values {
                                self.tick()?;
                                stack.push(v);
                            }
                        }
                        ValueKind::Typed { value, .. } => stack.push(value),
                        _ => (),
                    }
                }
            }
        }
        let mut units = BTreeMap::new();
        for context in &parts.contexts {
            self.tick()?;
            units.insert(context.id, context.units.length);
        }
        let mut rep_units = BTreeMap::new();
        for rep in &parts.representations {
            self.tick()?;
            rep_units.insert(rep.id, units[&rep.context]);
        }
        for r in &parts.relationships {
            self.tick()?;
            self.set_entity(EntityId::new(r.id.0).expect("adapter source IDs are nonzero"));
            let from = *rep_units
                .get(&r.rep_1)
                .ok_or_else(|| self.error(ErrorKind::Graph, "missing source representation"))?;
            let to = *rep_units
                .get(&r.rep_2)
                .ok_or_else(|| self.error(ErrorKind::Graph, "missing target representation"))?;
            let t = if let Some(pair) = r.transform {
                let a = self.view(pair.item_1.0)?;
                let b = self.view(pair.item_2.0)?;
                let a = self.frame(a, from)?;
                let b = self.frame(b, to)?;
                Some(self.math(self.math(a.inverse(Default::default()))?.then(b))?)
            } else if let Some(id) = r.operator {
                let v = self.view(id.0)?;
                let t = self.operator(v, to)?;
                // The operator scale is dimensionless; only its destination
                // origin needs unit conversion. Source coordinates are already SI.
                Some(t)
            } else {
                None
            };
            if let Some(t) = t {
                parts.relationship_transforms.push(ResolvedRelationship {
                    relationship: r.id,
                    rep_1_to_rep_2: t,
                });
            }
        }
        let mut maps = BTreeMap::new();
        for m in &parts.maps {
            self.tick()?;
            maps.insert(m.id, m);
        }
        let mut mapped = BTreeMap::new();
        for m in &parts.mapped_items {
            self.tick()?;
            mapped.insert(m.id, m);
        }
        for rep in &parts.representations {
            let mut seen = BTreeSet::new();
            let mut stack = Vec::new();
            for &item in &rep.items {
                self.tick()?;
                stack.push(item);
            }
            while let Some(item) = stack.pop() {
                self.tick()?;
                if !seen.insert(item) {
                    continue;
                }
                if let Some(next) = dependencies.get(&item) {
                    for &id in next {
                        self.tick()?;
                        stack.push(id);
                    }
                }
                if let Some(m) = mapped.get(&item) {
                    let map = maps.get(&m.map).ok_or_else(|| {
                        self.error(ErrorKind::Graph, "missing representation map")
                    })?;
                    let from = *rep_units.get(&map.representation).ok_or_else(|| {
                        self.error(ErrorKind::Graph, "missing mapped representation")
                    })?;
                    let to = rep_units[&rep.id];
                    let origin = self.view(map.origin.0)?;
                    let a = self.frame(origin, from)?;
                    let target = self.view(m.target.0)?;
                    let b = if self.is(target, "axis2_placement_3d")? {
                        self.frame(target, to)?
                    } else {
                        self.operator(target, to)?
                    };
                    // Both frame origins are already SI. For a mapped operator its
                    // scale is relative to the mapped frame, so don't double-convert units.
                    let t = self.math(self.math(a.inverse(Default::default()))?.then(b))?;
                    parts.mapped_transforms.push(ResolvedMapping {
                        item: m.id,
                        using_representation: rep.id,
                        source_to_using: t,
                    });
                }
            }
        }
        Ok(())
    }
}
