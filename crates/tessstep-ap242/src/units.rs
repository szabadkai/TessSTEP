use super::*;
use std::collections::BTreeSet;

impl<'d, 'a> Context<'d, 'a> {
    pub(super) fn context_units(&mut self, context: &'d EntityView<'a>) -> Result<Units, Error> {
        self.set_entity(context.id);
        self.expect(context, "geometric_representation_context")?;
        self.expect(context, "global_unit_assigned_context")?;
        let dimension = self.attr(context, "coordinate_space_dimension")?;
        if !matches!(dimension.kind, ValueKind::Integer(3)) {
            return Err(self.error(
                ErrorKind::Unsupported,
                "only 3D representation contexts are supported",
            ));
        }
        let mut length = None;
        let mut plane = None;
        let mut solid = None;
        for value in self.aggregate(context, "units")? {
            self.tick()?;
            let view = self.ref_value(value)?;
            let unit = self.unit(view)?;
            let slot = match unit.dimension() {
                Dimension::Length => &mut length,
                Dimension::PlaneAngle => &mut plane,
                Dimension::SolidAngle => &mut solid,
            };
            if slot.replace(unit).is_some() {
                return Err(self.error(ErrorKind::Units, "duplicate unit dimension in context"));
            }
        }
        self.set_entity(context.id);
        Ok(Units {
            length: length.ok_or_else(|| self.error(ErrorKind::Units, "missing length unit"))?,
            plane_angle: plane
                .ok_or_else(|| self.error(ErrorKind::Units, "missing plane angle unit"))?,
            solid_angle: solid
                .ok_or_else(|| self.error(ErrorKind::Units, "missing solid angle unit"))?,
        })
    }
    fn dimension(&mut self, view: &EntityView<'a>) -> Result<Dimension, Error> {
        let mut result = None;
        for (role, dimension) in [
            ("length_unit", Dimension::Length),
            ("plane_angle_unit", Dimension::PlaneAngle),
            ("solid_angle_unit", Dimension::SolidAngle),
        ] {
            if self.is(view, role)? && result.replace(dimension).is_some() {
                return Err(self.error(ErrorKind::Units, "unit has conflicting dimensions"));
            }
        }
        result.ok_or_else(|| self.error(ErrorKind::Unsupported, "unit dimension not supported"))
    }
    pub(super) fn unit(&mut self, mut view: &'d EntityView<'a>) -> Result<Unit, Error> {
        self.set_entity(view.id);
        let dimension = self.dimension(view)?;
        let mut seen = BTreeSet::new();
        let mut factors = Vec::new();
        let base = loop {
            self.set_entity(view.id);
            self.tick()?;
            if seen.len() >= self.limits.max_unit_depth {
                return Err(self.error(ErrorKind::ResourceLimit, "unit chain depth budget"));
            }
            if !seen.insert(view.id) {
                return Err(self.error(ErrorKind::Units, "cyclic unit conversion"));
            }
            if self.dimension(view)? != dimension {
                return Err(self.error(ErrorKind::Units, "conversion changes unit dimension"));
            }
            if self.is(view, "conversion_based_unit_with_offset")? {
                return Err(self.error(ErrorKind::Unsupported, "offset units are not supported"));
            }
            let si = self.is(view, "si_unit")?;
            let conversion = self.is(view, "conversion_based_unit")?;
            if si && conversion {
                return Err(self.error(ErrorKind::Units, "ambiguous unit definition"));
            }
            if si {
                let name = self.attr(view, "name")?;
                let expected = match dimension {
                    Dimension::Length => "METRE",
                    Dimension::PlaneAngle => "RADIAN",
                    Dimension::SolidAngle => "STERADIAN",
                };
                if !matches!(&name.kind, ValueKind::Enumeration(s) if s.eq_ignore_ascii_case(expected))
                {
                    return Err(
                        self.error(ErrorKind::Units, "SI name conflicts with unit dimension")
                    );
                }
                let prefix = self.attr(view, "prefix")?;
                let scale = match &prefix.kind {
                    ValueKind::Null => 1.0,
                    ValueKind::Enumeration(s) => prefix_scale(s).ok_or_else(|| {
                        self.error(ErrorKind::Unsupported, "unsupported SI prefix")
                    })?,
                    _ => return Err(self.error(ErrorKind::TypeMismatch, "expected SI prefix")),
                };
                break scale;
            } else if conversion {
                let exponents = self.target(view, "dimensions", "dimensional_exponents")?;
                for (i, name) in [
                    "length_exponent",
                    "mass_exponent",
                    "time_exponent",
                    "electric_current_exponent",
                    "thermodynamic_temperature_exponent",
                    "amount_of_substance_exponent",
                    "luminous_intensity_exponent",
                ]
                .into_iter()
                .enumerate()
                {
                    let value = self.attr(exponents, name)?;
                    let n = self.number(value)?;
                    let expected = if i == 0 && dimension == Dimension::Length {
                        1.0
                    } else {
                        0.0
                    };
                    if n != expected {
                        return Err(self.error(
                            ErrorKind::Units,
                            "conversion dimensions disagree with unit role",
                        ));
                    }
                }
                let measure = self.target(view, "conversion_factor", "measure_with_unit")?;
                let value = self.attr(measure, "value_component")?;
                let n = self.number(value)?;
                if !n.is_finite() || n <= 0.0 {
                    return Err(self.error(
                        ErrorKind::Units,
                        "conversion factor must be positive and finite",
                    ));
                }
                factors.push(n);
                view = self.reference(measure, "unit_component")?;
            } else {
                return Err(self.error(
                    ErrorKind::Unsupported,
                    "unit is neither SI nor conversion based",
                ));
            }
        };
        let mut scale = base;
        for factor in factors.into_iter().rev() {
            self.tick()?;
            scale *= factor;
            if !scale.is_finite() || scale <= 0.0 {
                return Err(
                    self.error(ErrorKind::Units, "conversion scale overflows or underflows")
                );
            }
        }
        Unit::new(dimension, scale)
            .map_err(|_| self.error(ErrorKind::Units, "invalid SI conversion scale"))
    }
}
fn prefix_scale(s: &str) -> Option<f64> {
    [
        ("EXA", 1e18),
        ("PETA", 1e15),
        ("TERA", 1e12),
        ("GIGA", 1e9),
        ("MEGA", 1e6),
        ("KILO", 1e3),
        ("HECTO", 1e2),
        ("DECA", 1e1),
        ("DECI", 1e-1),
        ("CENTI", 1e-2),
        ("MILLI", 1e-3),
        ("MICRO", 1e-6),
        ("NANO", 1e-9),
        ("PICO", 1e-12),
        ("FEMTO", 1e-15),
        ("ATTO", 1e-18),
    ]
    .into_iter()
    .find(|(name, _)| s.eq_ignore_ascii_case(name))
    .map(|(_, scale)| scale)
}
