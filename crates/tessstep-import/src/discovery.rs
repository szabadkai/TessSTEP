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
    let mut work = Work(options.max_work);
    let mut roots = Vec::new();
    for e in document.entities().iter() {
        work.charge(1)?;
        let kind = if has(e, "BREP_WITH_VOIDS") {
            SolidKind::BrepWithVoids
        } else if has(e, "FACETED_BREP") {
            SolidKind::FacetedBrep
        } else if has(e, "MANIFOLD_SOLID_BREP") {
            SolidKind::ManifoldSolidBrep
        } else {
            continue;
        };
        roots.push((e.id, kind));
    }
    let ids: Vec<EntityId> = roots.iter().map(|r| r.0).collect();
    let uses = containing(document, options, &mut work, &ids, &BTreeMap::new())?;
    Ok(roots
        .into_iter()
        .zip(uses)
        .map(|((entity, kind), uses)| {
            let (representations, units) = resolve(document, entity, uses);
            SolidRoot {
                entity,
                kind,
                representations,
                units,
            }
        })
        .collect())
}

/// An existing shape tessellation and the units of the representation containing it.
#[derive(Clone, Debug, PartialEq)]
pub struct TessellatedRoot {
    pub entity: EntityId,
    pub kind: TessellatedKind,
    /// The solid's GEOMETRIC_LINK or the shell's TOPOLOGICAL_LINK, read from the
    /// physical record without decoding its target.
    pub link: Option<EntityId>,
    pub representations: Vec<EntityId>,
    pub units: Result<ContextUnits, Error>,
}

/// Find every TESSELLATED_SOLID and TESSELLATED_SHELL, and every triangulated surface
/// set that is directly an item of a supported representation, with their context
/// units as `discover_solids` finds them. Surface sets reached only through geometric
/// sets are presentation graphics (`discover_presentations`), not shape roots.
pub fn discover_tessellations(
    document: &Document,
    options: ImportOptions,
) -> Result<Vec<TessellatedRoot>, Error> {
    let mut work = Work(options.max_work);
    let mut roots = Vec::new();
    for e in document.entities().iter() {
        work.charge(1)?;
        let kind = if has(e, "TESSELLATED_SOLID") {
            TessellatedKind::Solid
        } else if has(e, "TESSELLATED_SHELL") {
            TessellatedKind::Shell
        } else if has(e, "TRIANGULATED_SURFACE_SET") || has(e, "COMPLEX_TRIANGULATED_SURFACE_SET") {
            TessellatedKind::SurfaceSet
        } else {
            continue;
        };
        let link = match e.kind.records() {
            [record] if kind != TessellatedKind::SurfaceSet => match record.parameters.get(2) {
                Some(StepValue {
                    kind: ValueKind::Reference(id),
                    ..
                }) => Some(*id),
                _ => None,
            },
            _ => None,
        };
        roots.push((e.id, kind, link));
    }
    let ids: Vec<EntityId> = roots.iter().map(|r| r.0).collect();
    let uses = containing(document, options, &mut work, &ids, &BTreeMap::new())?;
    Ok(roots
        .into_iter()
        .zip(uses)
        .filter(|((_, kind, _), uses)| *kind != TessellatedKind::SurfaceSet || !uses.is_empty())
        .map(|((entity, kind, link), uses)| {
            let (representations, units) = resolve(document, entity, uses);
            TessellatedRoot {
                entity,
                kind,
                link,
                representations,
                units,
            }
        })
        .collect())
}

/// A tessellated annotation occurrence and the units of the representations (for
/// example draughting models) that contain it, directly or through containers.
#[derive(Clone, Debug, PartialEq)]
pub struct PresentationRoot {
    pub entity: EntityId,
    /// `*_CALLOUT` and ANNOTATION_PLANE records that contain it, directly or nested.
    pub containers: Vec<EntityId>,
    pub representations: Vec<EntityId>,
    pub units: Result<ContextUnits, Error>,
}

/// Find every TESSELLATED_ANNOTATION_OCCURRENCE, the callouts and annotation planes
/// that contain it (a plane may contain callouts), and the representations whose
/// items include the occurrence or one of those containers. Units follow the
/// `discover_solids` rules. Containment is read from physical references; nothing
/// is decoded until `import_presentation`.
pub fn discover_presentations(
    document: &Document,
    options: ImportOptions,
) -> Result<Vec<PresentationRoot>, Error> {
    let mut work = Work(options.max_work);
    let mut roots = Vec::new();
    let mut index = BTreeMap::new();
    for e in document.entities().iter() {
        work.charge(1)?;
        if has(e, "TESSELLATED_ANNOTATION_OCCURRENCE") {
            index.insert(e.id, roots.len());
            roots.push(e.id);
        }
    }
    // Container records and the occurrences or containers they reference directly.
    let mut direct: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    if !roots.is_empty() {
        let mut candidates = Vec::new();
        for e in document.entities().iter() {
            work.charge(1)?;
            let container = e.kind.records().iter().any(|r| {
                let name = r.name.as_ref().to_ascii_uppercase();
                name.ends_with("_CALLOUT") || name == "ANNOTATION_PLANE"
            });
            if container && !index.contains_key(&e.id) {
                candidates.push(e);
            }
        }
        let ids: BTreeMap<EntityId, usize> = candidates
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id, i))
            .collect();
        for e in &candidates {
            let mut refs: Vec<EntityId> = references(e, &index, &mut work)?
                .into_iter()
                .map(|i| roots[i])
                .collect();
            refs.extend(
                references(e, &ids, &mut work)?
                    .into_iter()
                    .map(|i| candidates[i].id),
            );
            if !refs.is_empty() {
                direct.insert(e.id, refs);
            }
        }
    }
    // Transitive members, bounded by the work budget and safe against cycles.
    let mut containers: BTreeMap<EntityId, Vec<usize>> = BTreeMap::new();
    let mut within = vec![Vec::new(); roots.len()];
    for &container in direct.keys() {
        let mut found = BTreeMap::new();
        let mut seen = std::collections::BTreeSet::from([container]);
        let mut stack = vec![container];
        while let Some(c) = stack.pop() {
            for &r in &direct[&c] {
                work.charge(1)?;
                if let Some(&i) = index.get(&r) {
                    found.insert(i, ());
                } else if direct.contains_key(&r) && seen.insert(r) {
                    stack.push(r);
                }
            }
        }
        let found: Vec<usize> = found.into_keys().collect();
        for &i in &found {
            within[i].push(container);
        }
        if !found.is_empty() {
            containers.insert(container, found);
        }
    }
    let uses = containing(document, options, &mut work, &roots, &containers)?;
    Ok(roots
        .into_iter()
        .zip(within)
        .zip(uses)
        .map(|((entity, containers), uses)| {
            let (representations, units) = resolve(document, entity, uses);
            PresentationRoot {
                entity,
                containers,
                representations,
                units,
            }
        })
        .collect())
}

/// Which representation `select_representations` prefers when a shape has both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationPreference {
    /// The B-rep solid; tessellations become alternatives.
    Exact,
    /// The first tessellation in document order; the solid becomes an alternative.
    Tessellated,
}
/// A discovered root, by index into the slices given to `select_representations`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootRef {
    Solid(usize),
    Tessellation(usize),
}
/// How alternative representations were recognized as the same shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pairing {
    /// A tessellated solid's GEOMETRIC_LINK names the solid, or a tessellated shell's
    /// TOPOLOGICAL_LINK names its outer shell.
    Link,
    /// A (SHAPE_)REPRESENTATION_RELATIONSHIP without transformation relates a
    /// representation holding only that tessellation to one holding only that solid.
    Relationship,
}
/// One shape: the root to import and the other representations of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationChoice {
    pub selected: RootRef,
    pub alternatives: Vec<RootRef>,
    /// None when the shape has a single representation.
    pub pairing: Option<Pairing>,
}

/// Group discovered solid and tessellated roots that represent the same shape and
/// select one of each group. Links pair first; a representation relationship pairs a
/// remaining tessellation only when each related representation holds exactly one
/// root of the respective kind, so ambiguous files are never guessed. Every input
/// root appears once, as a selection or an alternative: solids in input order, then
/// unpaired tessellations. Physical records are read without decoding, and nothing
/// is imported; a caller may fall back to an alternative when the selected root fails.
pub fn select_representations(
    document: &Document,
    solids: &[SolidRoot],
    tessellations: &[TessellatedRoot],
    preference: RepresentationPreference,
    options: ImportOptions,
) -> Result<Vec<RepresentationChoice>, Error> {
    let mut work = Work(options.max_work);
    let mut by_solid: BTreeMap<EntityId, usize> = BTreeMap::new();
    let mut by_shell: BTreeMap<EntityId, usize> = BTreeMap::new();
    for (i, solid) in solids.iter().enumerate() {
        work.charge(1)?;
        by_solid.insert(solid.entity, i);
        let outer = document
            .entities()
            .get(solid.entity)
            .and_then(|e| match e.kind.records() {
                [record] => match record.parameters.get(1) {
                    Some(StepValue {
                        kind: ValueKind::Reference(id),
                        ..
                    }) => Some(*id),
                    _ => None,
                },
                _ => None,
            });
        if let Some(shell) = outer {
            by_shell.insert(shell, i);
        }
    }
    let mut pairs: Vec<Option<(usize, Pairing)>> = vec![None; tessellations.len()];
    for (t, root) in tessellations.iter().enumerate() {
        work.charge(1)?;
        let target = match root.kind {
            TessellatedKind::Solid => root.link.and_then(|l| by_solid.get(&l)),
            TessellatedKind::Shell => root.link.and_then(|l| by_shell.get(&l)),
            TessellatedKind::SurfaceSet => None,
        };
        pairs[t] = target.map(|&s| (s, Pairing::Link));
    }
    // Representations holding exactly one root, by kind.
    let mut held: BTreeMap<EntityId, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (i, solid) in solids.iter().enumerate() {
        for &r in &solid.representations {
            work.charge(1)?;
            held.entry(r).or_default().0.push(i);
        }
    }
    for (i, root) in tessellations.iter().enumerate() {
        for &r in &root.representations {
            work.charge(1)?;
            held.entry(r).or_default().1.push(i);
        }
    }
    let only = |r: &EntityId, solid: bool| -> Option<usize> {
        match held.get(r) {
            Some((s, t)) if solid && s.len() == 1 && t.is_empty() => Some(s[0]),
            Some((s, t)) if !solid && t.len() == 1 && s.is_empty() => Some(t[0]),
            _ => None,
        }
    };
    for e in document.entities().iter() {
        work.charge(1)?;
        let [record] = e.kind.records() else {
            continue;
        };
        let name = record.name.as_ref();
        if !name.eq_ignore_ascii_case("SHAPE_REPRESENTATION_RELATIONSHIP")
            && !name.eq_ignore_ascii_case("REPRESENTATION_RELATIONSHIP")
        {
            continue;
        }
        let rep = |k: usize| match record.parameters.get(k) {
            Some(StepValue {
                kind: ValueKind::Reference(id),
                ..
            }) => Some(*id),
            _ => None,
        };
        let (Some(a), Some(b)) = (rep(2), rep(3)) else {
            continue;
        };
        for (x, y) in [(a, b), (b, a)] {
            if let (Some(t), Some(s)) = (only(&x, false), only(&y, true)) {
                if pairs[t].is_none() {
                    pairs[t] = Some((s, Pairing::Relationship));
                }
            }
        }
    }
    let mut choices = Vec::new();
    for s in 0..solids.len() {
        work.charge(1)?;
        let paired: Vec<(usize, Pairing)> = pairs
            .iter()
            .enumerate()
            .filter_map(|(t, p)| p.filter(|(solid, _)| *solid == s).map(|(_, how)| (t, how)))
            .collect();
        let Some(&(_, how)) = paired.first() else {
            choices.push(RepresentationChoice {
                selected: RootRef::Solid(s),
                alternatives: Vec::new(),
                pairing: None,
            });
            continue;
        };
        let mut members: Vec<RootRef> = paired
            .iter()
            .map(|(t, _)| RootRef::Tessellation(*t))
            .collect();
        let selected = match preference {
            RepresentationPreference::Exact => RootRef::Solid(s),
            RepresentationPreference::Tessellated => {
                let first = members.remove(0);
                members.insert(0, RootRef::Solid(s));
                first
            }
        };
        choices.push(RepresentationChoice {
            selected,
            alternatives: members,
            // Link pairing is reported when any member was linked explicitly.
            pairing: Some(if paired.iter().any(|(_, p)| *p == Pairing::Link) {
                Pairing::Link
            } else {
                how
            }),
        });
    }
    for (t, pair) in pairs.iter().enumerate() {
        if pair.is_none() {
            choices.push(RepresentationChoice {
                selected: RootRef::Tessellation(t),
                alternatives: Vec::new(),
                pairing: None,
            });
        }
    }
    Ok(choices)
}

struct Work(usize);
impl Work {
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        self.0 = self.0.checked_sub(n).ok_or_else(|| Error {
            kind: ErrorKind::ResourceLimit,
            stage: Stage::Profile,
            entity: None,
            source: None,
            message: "root discovery work budget".into(),
        })?;
        Ok(())
    }
}

fn has(e: &tessstep_part21::EntityInstance, name: &str) -> bool {
    e.kind
        .records()
        .iter()
        .any(|r| r.name.as_ref().eq_ignore_ascii_case(name))
}

/// Indices of `index` entries referenced anywhere in a record's parameters.
fn references(
    e: &tessstep_part21::EntityInstance,
    index: &BTreeMap<EntityId, usize>,
    work: &mut Work,
) -> Result<Vec<usize>, Error> {
    let mut found = Vec::new();
    let mut stack: Vec<&StepValue> = e
        .kind
        .records()
        .iter()
        .flat_map(|r| &r.parameters)
        .collect();
    while let Some(v) = stack.pop() {
        work.charge(1)?;
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
    Ok(found)
}

type Use = (EntityId, Result<ContextUnits, Error>);

/// For each root, the supported representations whose decoded items contain it, or
/// a container record holding it, with their context units, in representation order.
fn containing(
    document: &Document,
    options: ImportOptions,
    work: &mut Work,
    roots: &[EntityId],
    containers: &BTreeMap<EntityId, Vec<usize>>,
) -> Result<Vec<Vec<Use>>, Error> {
    // Physical users of each root: records that reference it anywhere.
    const REPRESENTATIONS: [&str; 7] = [
        "REPRESENTATION",
        "SHAPE_REPRESENTATION",
        "ADVANCED_BREP_SHAPE_REPRESENTATION",
        "FACETED_BREP_SHAPE_REPRESENTATION",
        "MANIFOLD_SURFACE_SHAPE_REPRESENTATION",
        "TESSELLATED_SHAPE_REPRESENTATION",
        "DRAUGHTING_MODEL",
    ];
    const CHARACTERIZED: [&str; 2] = ["CHARACTERIZED_OBJECT", "CHARACTERIZED_REPRESENTATION"];
    let mut uses: Vec<Vec<Use>> = vec![Vec::new(); roots.len()];
    if roots.is_empty() {
        return Ok(uses);
    }
    let mut index: BTreeMap<EntityId, usize> = BTreeMap::new();
    for (i, &root) in roots.iter().enumerate() {
        index.insert(root, i);
    }
    // Indexed records: roots first, then containers, each with the roots it holds.
    let mut keys: Vec<EntityId> = roots.to_vec();
    let mut members: Vec<Vec<usize>> = (0..roots.len()).map(|i| vec![i]).collect();
    for (&container, found) in containers {
        if let std::collections::btree_map::Entry::Vacant(slot) = index.entry(container) {
            slot.insert(keys.len());
            keys.push(container);
            members.push(found.clone());
        }
    }
    let mut users: BTreeMap<EntityId, Vec<usize>> = BTreeMap::new();
    for e in document.entities().iter() {
        work.charge(1)?;
        let records = e.kind.records();
        let named = |names: &[&str], r: &tessstep_part21::Record| {
            names
                .iter()
                .any(|n| r.name.as_ref().eq_ignore_ascii_case(n))
        };
        let representation = records
            .iter()
            .all(|r| named(&REPRESENTATIONS, r) || named(&CHARACTERIZED, r))
            && records.iter().any(|r| named(&REPRESENTATIONS, r));
        if !representation {
            continue;
        }
        let found = references(e, &index, work)?;
        if !found.is_empty() {
            users.insert(e.id, found);
        }
    }
    let mut contexts: BTreeMap<EntityId, Result<ContextUnits, Error>> = BTreeMap::new();
    for (&representation, found) in &users {
        let (items, context) = match decode_representation(document, representation, options) {
            Ok(decoded) => decoded,
            Err(e) => {
                for &i in found {
                    for &root in &members[i] {
                        uses[root].push((representation, Err(e.clone())));
                    }
                }
                continue;
            }
        };
        let units = contexts
            .entry(context)
            .or_insert_with(|| context_units(document, context, options))
            .clone();
        let mut added = BTreeMap::new();
        for &i in found {
            if !items.contains(&keys[i]) {
                continue;
            }
            for &root in &members[i] {
                work.charge(1)?;
                if added.insert(root, ()).is_none() {
                    uses[root].push((representation, units.clone()));
                }
            }
        }
    }
    Ok(uses)
}

/// Representations of one root and their unique units, or why none apply.
fn resolve(
    document: &Document,
    entity: EntityId,
    uses: Vec<Use>,
) -> (Vec<EntityId>, Result<ContextUnits, Error>) {
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
            "no supported representation contains the root".into(),
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
    (representations, units)
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
        &[
            decode::OmittedSlot {
                entity: s::Entity_SI_UNIT::DECLARATION,
                attribute: "DIMENSIONS",
                allow_unset: !options.strict,
                allow_value: false,
            },
            // Derived from the representation by the standard; many exporters write
            // the values instead of `*`, which the tolerant default accepts.
            decode::OmittedSlot {
                entity: s::Entity_CHARACTERIZED_OBJECT::DECLARATION,
                attribute: "OBJECT_NAME",
                allow_unset: false,
                allow_value: !options.strict,
            },
            decode::OmittedSlot {
                entity: s::Entity_CHARACTERIZED_OBJECT::DECLARATION,
                attribute: "OBJECT_DESCRIPTION",
                allow_unset: false,
                allow_value: !options.strict,
            },
        ],
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
            allow_value: false,
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
