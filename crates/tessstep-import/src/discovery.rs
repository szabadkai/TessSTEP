//! Solid root discovery and representation-context units.
use super::*;
use tessstep_math::AngleUnit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolidKind {
    ManifoldSolidBrep,
    BrepWithVoids,
    FacetedBrep,
}
impl SolidKind {
    pub fn step_name(self) -> &'static str {
        match self {
            Self::ManifoldSolidBrep => "MANIFOLD_SOLID_BREP",
            Self::BrepWithVoids => "BREP_WITH_VOIDS",
            Self::FacetedBrep => "FACETED_BREP",
        }
    }
}

/// Units declared by one geometric representation context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextUnits {
    pub context: EntityId,
    pub length: LengthUnit,
    pub plane_angle: AngleUnit,
    /// The context's length uncertainty in metres, if it assigns one. A measure
    /// named `DISTANCE_ACCURACY_VALUE` wins; otherwise the smallest length measure.
    pub distance_uncertainty: Option<f64>,
}

/// A solid root and the units of the shape representation that contains it.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidRoot {
    pub entity: EntityId,
    pub kind: SolidKind,
    /// Shape representations whose items include the root, in document order.
    pub representations: Vec<EntityId>,
    /// Units of the unique context of those representations, or why none applies.
    pub units: Result<ContextUnits, Error>,
}

/// Find every MANIFOLD_SOLID_BREP, BREP_WITH_VOIDS and FACETED_BREP instance and the
/// length/plane-angle units and length uncertainty of the representation context
/// that contains it. Candidate records are selected by physical record name; each
/// containing representation is then decoded with the bounded context profile.
/// A root outside every supported shape representation, or used in representations
/// with different units, receives a unit error instead of a default.
pub fn discover_solids(
    document: &Document,
    options: ImportOptions,
) -> Result<Vec<SolidRoot>, Error> {
    let mut work = options.max_work;
    let mut charge = |n: usize| -> Result<(), Error> {
        work = work.checked_sub(n).ok_or_else(|| Error {
            kind: ErrorKind::ResourceLimit,
            stage: Stage::Profile,
            entity: None,
            source: None,
            message: "root discovery work budget".into(),
        })?;
        Ok(())
    };
    let mut roots = Vec::new();
    let mut index = BTreeMap::new();
    for e in document.entities().iter() {
        charge(1)?;
        let has = |name: &str| {
            e.kind
                .records()
                .iter()
                .any(|r| r.name.as_ref().eq_ignore_ascii_case(name))
        };
        let kind = if has("BREP_WITH_VOIDS") {
            SolidKind::BrepWithVoids
        } else if has("FACETED_BREP") {
            SolidKind::FacetedBrep
        } else if has("MANIFOLD_SOLID_BREP") {
            SolidKind::ManifoldSolidBrep
        } else {
            continue;
        };
        index.insert(e.id, roots.len());
        roots.push((e.id, kind, Vec::new()));
    }
    // Physical users of each root: records that reference it anywhere.
    const REPRESENTATIONS: [&str; 6] = [
        "REPRESENTATION",
        "SHAPE_REPRESENTATION",
        "ADVANCED_BREP_SHAPE_REPRESENTATION",
        "FACETED_BREP_SHAPE_REPRESENTATION",
        "MANIFOLD_SURFACE_SHAPE_REPRESENTATION",
        "TESSELLATED_SHAPE_REPRESENTATION",
    ];
    let mut users: BTreeMap<EntityId, Vec<usize>> = BTreeMap::new();
    if !roots.is_empty() {
        for e in document.entities().iter() {
            charge(1)?;
            let records = e.kind.records();
            let representation = records.iter().all(|r| {
                REPRESENTATIONS
                    .iter()
                    .any(|n| r.name.as_ref().eq_ignore_ascii_case(n))
            });
            if !representation {
                continue;
            }
            let mut found = Vec::new();
            let mut stack: Vec<&StepValue> = records.iter().flat_map(|r| &r.parameters).collect();
            while let Some(v) = stack.pop() {
                charge(1)?;
                match &v.kind {
                    ValueKind::Reference(id) => {
                        if let Some(&i) = index.get(id) {
                            found.push(i);
                        }
                    }
                    ValueKind::Aggregate(items) => stack.extend(items),
                    _ => {}
                }
            }
            if !found.is_empty() {
                users.insert(e.id, found);
            }
        }
    }
    let mut contexts: BTreeMap<EntityId, Result<ContextUnits, Error>> = BTreeMap::new();
    for (&representation, found) in &users {
        let decoded = match decode_representation(document, representation, options) {
            Ok(units) => units,
            Err(e) => {
                for &i in found {
                    roots[i].2.push((representation, Err(e.clone())));
                }
                continue;
            }
        };
        let (items, context) = decoded;
        let units = contexts
            .entry(context)
            .or_insert_with(|| context_units(document, context, options))
            .clone();
        for &i in found {
            if items.contains(&roots[i].0) {
                roots[i].2.push((representation, units.clone()));
            }
        }
    }
    Ok(roots
        .into_iter()
        .map(|(entity, kind, uses)| {
            let representations: Vec<EntityId> = uses.iter().map(|(r, _)| *r).collect();
            let unit_error = |message: String| Error {
                kind: ErrorKind::Unsupported,
                stage: Stage::Profile,
                entity: Some(entity),
                source: document.entities().get(entity).map(|e| e.source),
                message,
            };
            let units = match uses.first() {
                None => Err(unit_error(
                    "no supported shape representation contains the root".into(),
                )),
                Some((_, first)) => {
                    let first = first.clone();
                    let conflicting = uses.iter().any(|(_, u)| match (u, &first) {
                        (Ok(a), Ok(b)) => {
                            a.length != b.length
                                || a.plane_angle != b.plane_angle
                                || a.distance_uncertainty != b.distance_uncertainty
                        }
                        (Err(_), _) | (_, Err(_)) => false,
                    });
                    if conflicting {
                        Err(unit_error(
                            "the root is used in representations with different units".into(),
                        ))
                    } else {
                        uses.iter()
                            .find_map(|(_, u)| u.clone().ok())
                            .ok_or(())
                            .or(first)
                    }
                }
            };
            SolidRoot {
                entity,
                kind,
                representations,
                units,
            }
        })
        .collect())
}

fn decode_representation(
    document: &Document,
    representation: EntityId,
    options: ImportOptions,
) -> Result<(Vec<EntityId>, EntityId), Error> {
    use context_profile::schema_tessstep_context as s;
    use tessstep_schema::EntityBinding;
    let schema = &context_profile::SCHEMA_SET;
    let decoded = decode::decode_reachable_profile_with_links(
        document,
        schema,
        "tessstep_context",
        &[representation],
        &[decode::OmittedSlot {
            entity: s::Entity_SI_UNIT::DECLARATION,
            attribute: "DIMENSIONS",
            allow_unset: !options.strict,
        }],
        &[decode::LinkSlot {
            entity: s::Entity_REPRESENTATION::DECLARATION,
            attribute: "ITEMS",
        }],
        decode::Limits {
            max_work: options.max_work,
            ..decode::Limits::default()
        },
    )
    .map_err(|e| context_error(document, e))?;
    let view = decoded.get(representation).expect("decoded root");
    let value = |name: &str| {
        view.attributes
            .iter()
            .find(|a| a.declaration.name == name)
            .map(|a| a.value)
    };
    let mut items = Vec::new();
    if let Some(ValueKind::Aggregate(values)) = value("ITEMS").map(|v| &v.kind) {
        for v in values {
            if let ValueKind::Reference(id) = v.kind {
                items.push(id);
            }
        }
    }
    let Some(ValueKind::Reference(context)) = value("CONTEXT_OF_ITEMS").map(|v| v.kind.clone())
    else {
        unreachable!("decoded required reference");
    };
    Ok((items, context))
}

fn context_error(document: &Document, e: decode::Error) -> Error {
    Error {
        kind: match e.kind {
            decode::ErrorKind::ResourceLimit => ErrorKind::ResourceLimit,
            decode::ErrorKind::MissingReference => ErrorKind::MissingEntity,
            decode::ErrorKind::UnknownEntity | decode::ErrorKind::Unsupported => {
                ErrorKind::Unsupported
            }
            _ => ErrorKind::InvalidGeometry,
        },
        stage: Stage::Profile,
        entity: e.entity,
        source: e.source,
        message: if e.kind == decode::ErrorKind::UnknownEntity {
            outside_profile(
                document,
                &context_profile::SCHEMA_SET,
                "tessstep_context",
                e.entity,
            )
        } else {
            e.to_string()
        },
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Dimension {
    Length,
    PlaneAngle,
    SolidAngle,
}

struct Units<'d, 'a> {
    decoded: &'d decode::DecodedDocument<'a>,
    current: EntityId,
}
impl<'d, 'a> Units<'d, 'a> {
    fn error(&self, message: &str) -> Error {
        Error {
            kind: ErrorKind::Unsupported,
            stage: Stage::Profile,
            entity: Some(self.current),
            source: self
                .decoded
                .document()
                .entities()
                .get(self.current)
                .map(|e| e.source),
            message: message.into(),
        }
    }
    fn view(&self, id: EntityId) -> &'d EntityView<'a> {
        self.decoded
            .get(id)
            .expect("reference inside the decoded closure")
    }
    fn is(&self, v: &EntityView<'_>, name: &str) -> bool {
        v.types.iter().any(|id| {
            self.decoded
                .schemas()
                .declaration(*id)
                .is_some_and(|d| d.name == name)
        })
    }
    fn attr(&self, v: &'d EntityView<'a>, name: &str) -> &'a StepValue {
        v.attributes
            .iter()
            .find(|a| a.declaration.name == name)
            .map(|a| a.value)
            .expect("profile attribute")
    }
    fn reference(&self, v: &'d EntityView<'a>, name: &str) -> &'d EntityView<'a> {
        match self.attr(v, name).kind {
            ValueKind::Reference(id) => self.view(id),
            _ => unreachable!("decoded entity reference"),
        }
    }
    fn number(value: &StepValue) -> Option<f64> {
        match &value.kind {
            ValueKind::Real(x) => Some(*x),
            ValueKind::Integer(x) => Some(*x as f64),
            ValueKind::Typed { value, .. } => Self::number(value),
            _ => None,
        }
    }
    fn dimension(&self, v: &EntityView<'_>) -> Result<Dimension, Error> {
        let found: Vec<Dimension> = [
            ("LENGTH_UNIT", Dimension::Length),
            ("PLANE_ANGLE_UNIT", Dimension::PlaneAngle),
            ("SOLID_ANGLE_UNIT", Dimension::SolidAngle),
        ]
        .into_iter()
        .filter(|(name, _)| self.is(v, name))
        .map(|(_, d)| d)
        .collect();
        match found.as_slice() {
            [d] => Ok(*d),
            [] => Err(self.error("unit has no length, plane angle or solid angle role")),
            _ => Err(self.error("unit has conflicting dimensions")),
        }
    }
    /// SI scale of a unit: metres, radians or steradians per unit.
    fn scale(&mut self, mut v: &'d EntityView<'a>) -> Result<(Dimension, f64), Error> {
        let dimension = self.dimension(v)?;
        let mut scale = 1.;
        for _ in 0..16 {
            self.current = v.id;
            if self.dimension(v)? != dimension {
                return Err(self.error("unit conversion changes dimension"));
            }
            if self.is(v, "SI_UNIT") {
                let expected = match dimension {
                    Dimension::Length => "METRE",
                    Dimension::PlaneAngle => "RADIAN",
                    Dimension::SolidAngle => "STERADIAN",
                };
                let ValueKind::Enumeration(name) = &self.attr(v, "NAME").kind else {
                    unreachable!("decoded enumeration");
                };
                if !name.eq_ignore_ascii_case(expected) {
                    return Err(self.error("SI unit name conflicts with its dimension"));
                }
                if let ValueKind::Enumeration(prefix) = &self.attr(v, "PREFIX").kind {
                    scale *= prefix_scale(prefix).expect("decoded SI prefix");
                }
                return Ok((dimension, scale));
            }
            if !self.is(v, "CONVERSION_BASED_UNIT") {
                return Err(self.error("unit is neither SI nor conversion based"));
            }
            let exponents = self.reference(v, "DIMENSIONS");
            let expected_length = if dimension == Dimension::Length {
                1.
            } else {
                0.
            };
            for (i, a) in exponents.attributes.iter().enumerate() {
                let n = Self::number(a.value).unwrap_or(f64::NAN);
                if n != if i == 0 { expected_length } else { 0. } {
                    return Err(self.error("conversion dimensions disagree with the unit role"));
                }
            }
            let measure = self.reference(v, "CONVERSION_FACTOR");
            let factor = Self::number(self.attr(measure, "VALUE_COMPONENT")).unwrap_or(f64::NAN);
            if !factor.is_finite() || factor <= 0. {
                return Err(self.error("conversion factor must be positive and finite"));
            }
            scale *= factor;
            v = self.reference(measure, "UNIT_COMPONENT");
        }
        Err(self.error("unit conversion chain is too deep"))
    }
}

fn context_units(
    document: &Document,
    context: EntityId,
    options: ImportOptions,
) -> Result<ContextUnits, Error> {
    use context_profile::schema_tessstep_context as s;
    use tessstep_schema::EntityBinding;
    let decoded = decode::decode_reachable_profile(
        document,
        &context_profile::SCHEMA_SET,
        "tessstep_context",
        &[context],
        &[decode::OmittedSlot {
            entity: s::Entity_SI_UNIT::DECLARATION,
            attribute: "DIMENSIONS",
            allow_unset: !options.strict,
        }],
        decode::Limits {
            max_work: options.max_work,
            ..decode::Limits::default()
        },
    )
    .map_err(|e| context_error(document, e))?;
    let mut u = Units {
        decoded: &decoded,
        current: context,
    };
    let view = u.view(context);
    if !u.is(view, "GEOMETRIC_REPRESENTATION_CONTEXT")
        || !u.is(view, "GLOBAL_UNIT_ASSIGNED_CONTEXT")
    {
        return Err(u.error("representation context assigns no geometric units"));
    }
    if !matches!(
        u.attr(view, "COORDINATE_SPACE_DIMENSION").kind,
        ValueKind::Integer(3)
    ) {
        return Err(u.error("only 3D representation contexts are supported"));
    }
    let mut length = None;
    let mut angle = None;
    let ValueKind::Aggregate(units) = &u.attr(view, "UNITS").kind else {
        unreachable!("decoded aggregate");
    };
    for value in units {
        let ValueKind::Reference(id) = value.kind else {
            unreachable!("decoded reference");
        };
        let (dimension, scale) = u.scale(u.view(id))?;
        let slot = match dimension {
            Dimension::Length => &mut length,
            Dimension::PlaneAngle => &mut angle,
            Dimension::SolidAngle => continue,
        };
        if slot.replace(scale).is_some() {
            u.current = context;
            return Err(u.error("duplicate unit dimension in representation context"));
        }
    }
    u.current = context;
    let length = length.ok_or_else(|| u.error("representation context has no length unit"))?;
    let angle = angle.ok_or_else(|| u.error("representation context has no plane angle unit"))?;
    let mut uncertainty: Option<(bool, f64)> = None;
    if u.is(view, "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT") {
        let ValueKind::Aggregate(measures) = &u.attr(view, "UNCERTAINTY").kind else {
            unreachable!("decoded aggregate");
        };
        for value in measures {
            let ValueKind::Reference(id) = value.kind else {
                unreachable!("decoded reference");
            };
            let measure = u.view(id);
            let (dimension, scale) = u.scale(u.reference(measure, "UNIT_COMPONENT"))?;
            u.current = measure.id;
            if dimension != Dimension::Length {
                continue;
            }
            let value = Units::number(u.attr(measure, "VALUE_COMPONENT")).unwrap_or(f64::NAN);
            let metres = value * scale;
            if !metres.is_finite() || metres <= 0. {
                return Err(u.error("length uncertainty must be positive and finite"));
            }
            let named = matches!(&u.attr(measure, "NAME").kind,
                ValueKind::String(s) if s.eq_ignore_ascii_case("DISTANCE_ACCURACY_VALUE"));
            uncertainty = Some(match uncertainty {
                None => (named, metres),
                Some((true, m)) if !named => (true, m),
                Some((was_named, m)) if was_named == named => (named, m.min(metres)),
                Some(_) => (named, metres),
            });
        }
    }
    u.current = context;
    Ok(ContextUnits {
        context,
        length: LengthUnit::metres_per_unit(length)
            .map_err(|_| u.error("invalid length unit scale"))?,
        plane_angle: AngleUnit::radians_per_unit(angle)
            .map_err(|_| u.error("invalid plane angle unit scale"))?,
        distance_uncertainty: uncertainty.map(|(_, m)| m),
    })
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
