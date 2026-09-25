//! STEP-independent product and representation graphs, with explicit SI scales.
//!
//! Explicit placements use checked metre-based affine maps. Graph validity does
//! not establish geometry, topology or AP conformance.
#![forbid(unsafe_code)]

mod assembly;
pub use assembly::*;
pub type Transform = tessstep_math::Affine3<tessstep_math::ModelSpace, tessstep_math::ModelSpace>;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

macro_rules! ids {
    ($($name:ident),*) => {$(
        /// Caller-assigned identity, local to one product model; never an allocation size.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(pub u64);
    )*};
}
ids!(
    ProductId,
    FormationId,
    DefinitionId,
    ContextId,
    RepresentationId,
    ShapeId,
    OccurrenceId,
    RelationshipId,
    MapId,
    ItemId
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Length,
    PlaneAngle,
    SolidAngle,
}

/// Positive finite conversion to metres, radians or steradians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    dimension: Dimension,
    scale: f64,
}
impl Unit {
    pub fn new(dimension: Dimension, scale: f64) -> Result<Self, Error> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(Error::InvalidUnit);
        }
        Ok(Self { dimension, scale })
    }
    pub fn dimension(self) -> Dimension {
        self.dimension
    }
    pub fn scale(self) -> f64 {
        self.scale
    }
    /// Reject overflow and nonzero underflow rather than silently losing a length.
    pub fn to_si(self, value: f64) -> Result<f64, Error> {
        let result = value * self.scale;
        if !value.is_finite() || !result.is_finite() || (value != 0.0 && result == 0.0) {
            return Err(Error::NumericRange);
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Units {
    pub length: Unit,
    pub plane_angle: Unit,
    pub solid_angle: Unit,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    pub id: ProductId,
    pub identifier: String,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Formation {
    pub id: FormationId,
    pub product: ProductId,
    pub identifier: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub id: DefinitionId,
    pub formation: FormationId,
    pub identifier: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    pub id: ContextId,
    pub units: Units,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Representation {
    pub id: RepresentationId,
    pub name: String,
    pub context: ContextId,
    /// Reusable opaque item identities. These are not evaluated geometry assets.
    pub items: Vec<ItemId>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeTarget {
    Definition(DefinitionId),
    Occurrence(OccurrenceId),
}
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub id: ShapeId,
    pub target: ShapeTarget,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeBinding {
    pub shape: ShapeId,
    pub representation: RepresentationId,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub id: OccurrenceId,
    pub identifier: String,
    pub parent: DefinitionId,
    pub child: DefinitionId,
}
/// An unevaluated placement pair. Item 1 belongs to rep_1, item 2 to rep_2.
/// Keeping this order avoids guessing a child-to-parent matrix direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemTransform {
    pub item_1: ItemId,
    pub item_2: ItemId,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub id: RelationshipId,
    pub rep_1: RepresentationId,
    pub rep_2: RepresentationId,
    /// None means no transformation was specified, not an identity matrix.
    pub transform: Option<ItemTransform>,
    /// Alternative Cartesian operator identity; mutually exclusive with the pair.
    pub operator: Option<ItemId>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct OccurrencePlacement {
    pub occurrence: OccurrenceId,
    pub relationship: RelationshipId,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RepresentationMap {
    pub id: MapId,
    pub representation: RepresentationId,
    pub origin: ItemId,
}
#[derive(Debug, Clone, PartialEq)]
pub struct MappedItem {
    pub id: ItemId,
    pub map: MapId,
    pub target: ItemId,
}

/// Unvalidated input. Use `Model::new` to obtain an immutable checked graph.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Parts {
    pub products: Vec<Product>,
    pub formations: Vec<Formation>,
    pub definitions: Vec<Definition>,
    pub contexts: Vec<Context>,
    pub representations: Vec<Representation>,
    pub shapes: Vec<Shape>,
    pub shape_bindings: Vec<ShapeBinding>,
    pub occurrences: Vec<Occurrence>,
    pub relationships: Vec<Relationship>,
    pub placements: Vec<OccurrencePlacement>,
    pub maps: Vec<RepresentationMap>,
    pub mapped_items: Vec<MappedItem>,
    /// Contextual references between representation items, excluding map-source crossings.
    pub item_dependencies: Vec<(ItemId, ItemId)>,
    /// Untransformed shape relationships used for definition ownership discovery.
    pub associations: Vec<RelationshipId>,
    pub relationship_transforms: Vec<ResolvedRelationship>,
    pub mapped_transforms: Vec<ResolvedMapping>,
    pub uncertainties: Vec<Uncertainty>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    DuplicateId,
    MissingLink,
    Cycle,
    InvalidUnit,
    NumericRange,
    ResourceLimit,
    InvalidTransform,
    AmbiguousPlacement,
    MissingPlacement,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    parts: Parts,
    roots: Vec<DefinitionId>,
    definition_reps: BTreeMap<DefinitionId, BTreeSet<RepresentationId>>,
    context_items: BTreeMap<ContextId, BTreeSet<ItemId>>,
}
impl Model {
    /// Validate references, unique identities, unit dimensions and acyclic assembly
    /// and mapped-representation graphs. Work is bounded; no instance tree is expanded.
    pub fn new(parts: Parts, max_work: usize) -> Result<Self, Error> {
        let mut work = Budget(max_work);
        let products = index(parts.products.iter().map(|p| p.id), &mut work)?;
        let formations = index(parts.formations.iter().map(|p| p.id), &mut work)?;
        let definitions = index(parts.definitions.iter().map(|p| p.id), &mut work)?;
        let contexts = index(parts.contexts.iter().map(|p| p.id), &mut work)?;
        let representations = index(parts.representations.iter().map(|p| p.id), &mut work)?;
        let shapes = index(parts.shapes.iter().map(|p| p.id), &mut work)?;
        let occurrences = index(parts.occurrences.iter().map(|p| p.id), &mut work)?;
        let relationships = index(parts.relationships.iter().map(|p| p.id), &mut work)?;
        let maps = index(parts.maps.iter().map(|p| p.id), &mut work)?;
        let mapped = index(parts.mapped_items.iter().map(|p| p.id), &mut work)?;
        for p in &parts.formations {
            work.link(&products, p.product)?;
        }
        for p in &parts.definitions {
            work.link(&formations, p.formation)?;
        }
        for p in &parts.contexts {
            work.tick()?;
            if p.units.length.dimension() != Dimension::Length
                || p.units.plane_angle.dimension() != Dimension::PlaneAngle
                || p.units.solid_angle.dimension() != Dimension::SolidAngle
            {
                return Err(Error::InvalidUnit);
            }
        }
        let mut dependencies: BTreeMap<ItemId, Vec<ItemId>> = BTreeMap::new();
        for &(a, b) in &parts.item_dependencies {
            work.tick()?;
            dependencies.entry(a).or_default().push(b);
        }
        let mut context_items: BTreeMap<ContextId, BTreeSet<ItemId>> = BTreeMap::new();
        let mut representation_items = BTreeMap::new();
        for p in &parts.representations {
            work.link(&contexts, p.context)?;
            context_items.entry(p.context).or_default();
            let local = index(p.items.iter().copied(), &mut work)?;
            let mut found = BTreeSet::new();
            let mut stack: Vec<_> = local.keys().copied().collect();
            while let Some(item) = stack.pop() {
                work.tick()?;
                if found.insert(item) {
                    context_items.entry(p.context).or_default().insert(item);
                    if let Some(children) = dependencies.get(&item) {
                        for &child in children {
                            work.tick()?;
                            stack.push(child);
                        }
                    }
                }
            }
            representation_items.insert(p.id, found);
        }
        for p in &parts.shapes {
            match p.target {
                ShapeTarget::Definition(id) => work.link(&definitions, id)?,
                ShapeTarget::Occurrence(id) => work.link(&occurrences, id)?,
            }
        }
        for p in &parts.shape_bindings {
            work.link(&shapes, p.shape)?;
            work.link(&representations, p.representation)?;
        }
        let mut definition_reps: BTreeMap<DefinitionId, BTreeSet<RepresentationId>> =
            BTreeMap::new();
        for p in &parts.shape_bindings {
            work.tick()?;
            if let ShapeTarget::Definition(id) = parts.shapes[shapes[&p.shape]].target {
                definition_reps
                    .entry(id)
                    .or_default()
                    .insert(p.representation);
            }
        }
        let mut associated: BTreeMap<RepresentationId, Vec<RepresentationId>> = BTreeMap::new();
        for &id in &parts.associations {
            work.link(&relationships, id)?;
            let r = &parts.relationships[relationships[&id]];
            if r.transform.is_some() || r.operator.is_some() {
                return Err(Error::InvalidTransform);
            }
            work.link(&representations, r.rep_1)?;
            work.link(&representations, r.rep_2)?;
            associated.entry(r.rep_1).or_default().push(r.rep_2);
            associated.entry(r.rep_2).or_default().push(r.rep_1);
        }
        for reps in definition_reps.values_mut() {
            let mut stack = Vec::new();
            for &rep in reps.iter() {
                work.tick()?;
                stack.push(rep);
            }
            let mut seen = BTreeSet::new();
            while let Some(rep) = stack.pop() {
                work.tick()?;
                if seen.insert(rep) {
                    reps.insert(rep);
                    if let Some(next) = associated.get(&rep) {
                        for &r in next {
                            work.tick()?;
                            stack.push(r);
                        }
                    }
                }
            }
        }
        let mut assembly = Vec::new();
        let mut children = BTreeSet::new();
        for p in &parts.occurrences {
            work.link(&definitions, p.parent)?;
            work.link(&definitions, p.child)?;
            assembly.push((p.parent, p.child));
            children.insert(p.child);
        }
        acyclic(&definitions, &assembly, &mut work)?;
        for p in &parts.relationships {
            work.link(&representations, p.rep_1)?;
            work.link(&representations, p.rep_2)?;
            if p.transform.is_some() && p.operator.is_some() {
                return Err(Error::InvalidTransform);
            }
            if let Some(t) = p.transform {
                for (rep, item) in [(p.rep_1, t.item_1), (p.rep_2, t.item_2)] {
                    work.tick()?;
                    let context = parts.representations[representations[&rep]].context;
                    if !context_items[&context].contains(&item) {
                        return Err(Error::MissingLink);
                    }
                }
            }
        }
        for p in &parts.placements {
            work.link(&occurrences, p.occurrence)?;
            work.link(&relationships, p.relationship)?;
            let occurrence = &parts.occurrences[occurrences[&p.occurrence]];
            let relation = &parts.relationships[relationships[&p.relationship]];
            let parent = definition_reps.get(&occurrence.parent);
            let child = definition_reps.get(&occurrence.child);
            let connects = |a, b| {
                parent.is_some_and(|s| s.contains(&a)) && child.is_some_and(|s| s.contains(&b))
            };
            if !(connects(relation.rep_1, relation.rep_2)
                || connects(relation.rep_2, relation.rep_1))
            {
                return Err(Error::MissingLink);
            }
        }
        for p in &parts.maps {
            work.link(&representations, p.representation)?;
            work.tick()?;
            let context = parts.representations[representations[&p.representation]].context;
            if !context_items[&context].contains(&p.origin) {
                return Err(Error::MissingLink);
            }
        }
        for p in &parts.mapped_items {
            work.link(&maps, p.map)?;
        }
        let mut mapping_edges = Vec::new();
        let mut used = BTreeSet::new();
        for p in &parts.representations {
            for item in &representation_items[&p.id] {
                work.tick()?;
                if let Some(&i) = mapped.get(item) {
                    let m = &parts.mapped_items[i];
                    if !context_items[&p.context].contains(&m.target) {
                        return Err(Error::MissingLink);
                    }
                    used.insert(m.id);
                    mapping_edges.push((p.id, parts.maps[maps[&m.map]].representation));
                }
            }
        }
        if used.len() != mapped.len() {
            return Err(Error::MissingLink);
        }
        index(
            parts.relationship_transforms.iter().map(|p| p.relationship),
            &mut work,
        )?;
        for p in &parts.relationship_transforms {
            work.link(&relationships, p.relationship)?;
            let r = &parts.relationships[relationships[&p.relationship]];
            if r.transform.is_none() && r.operator.is_none() {
                return Err(Error::InvalidTransform);
            }
            p.rep_1_to_rep_2
                .inverse(Default::default())
                .map_err(|_| Error::InvalidTransform)?;
        }
        let mut mappings = BTreeSet::new();
        for p in &parts.mapped_transforms {
            work.link(&mapped, p.item)?;
            work.link(&representations, p.using_representation)?;
            if !representation_items[&p.using_representation].contains(&p.item) {
                return Err(Error::MissingLink);
            }
            if !mappings.insert((p.item, p.using_representation)) {
                return Err(Error::DuplicateId);
            }
            p.source_to_using
                .inverse(Default::default())
                .map_err(|_| Error::InvalidTransform)?;
        }
        for p in &parts.uncertainties {
            work.link(&contexts, p.context)?;
            if !p.value_si.is_finite() || p.value_si <= 0.0 {
                return Err(Error::NumericRange);
            }
        }
        acyclic(&representations, &mapping_edges, &mut work)?;
        let mut roots = Vec::new();
        for p in &parts.definitions {
            work.tick()?;
            if !children.contains(&p.id) {
                roots.push(p.id);
            }
        }
        Ok(Self {
            parts,
            roots,
            definition_reps,
            context_items,
        })
    }
    pub fn parts(&self) -> &Parts {
        &self.parts
    }
    /// Definitions with no incoming assembly occurrence, in input order.
    pub fn roots(&self) -> &[DefinitionId] {
        &self.roots
    }
}
struct Budget(usize);
impl Budget {
    fn tick(&mut self) -> Result<(), Error> {
        self.0 = self.0.checked_sub(1).ok_or(Error::ResourceLimit)?;
        Ok(())
    }
    fn link<K: Ord>(&mut self, index: &BTreeMap<K, usize>, key: K) -> Result<(), Error> {
        self.tick()?;
        if index.contains_key(&key) {
            Ok(())
        } else {
            Err(Error::MissingLink)
        }
    }
}
fn index<K: Ord>(
    keys: impl Iterator<Item = K>,
    work: &mut Budget,
) -> Result<BTreeMap<K, usize>, Error> {
    let mut result = BTreeMap::new();
    for (i, k) in keys.enumerate() {
        work.tick()?;
        if result.insert(k, i).is_some() {
            return Err(Error::DuplicateId);
        }
    }
    Ok(result)
}
fn acyclic<K: Ord + Copy>(
    nodes: &BTreeMap<K, usize>,
    edges: &[(K, K)],
    work: &mut Budget,
) -> Result<(), Error> {
    let mut degree = vec![0usize; nodes.len()];
    let mut next = vec![Vec::new(); nodes.len()];
    for &(a, b) in edges {
        work.tick()?;
        let (a, b) = (nodes[&a], nodes[&b]);
        degree[b] = degree[b].checked_add(1).ok_or(Error::ResourceLimit)?;
        next[a].push(b);
    }
    let mut queue = VecDeque::new();
    for (i, &d) in degree.iter().enumerate() {
        work.tick()?;
        if d == 0 {
            queue.push_back(i);
        }
    }
    let mut visited = 0;
    while let Some(i) = queue.pop_front() {
        work.tick()?;
        visited += 1;
        for &j in &next[i] {
            work.tick()?;
            degree[j] -= 1;
            if degree[j] == 0 {
                queue.push_back(j);
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        Err(Error::Cycle)
    }
}
