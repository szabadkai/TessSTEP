//! STEP-independent product and representation graphs, with explicit SI scales.
//!
//! Placements remain descriptions until the independent math layer can evaluate
//! them. Graph validity does not establish geometry, topology or AP conformance.
#![forbid(unsafe_code)]

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
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    DuplicateId,
    MissingLink,
    Cycle,
    InvalidUnit,
    NumericRange,
    ResourceLimit,
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
        let mut items = BTreeSet::new();
        for p in &parts.representations {
            work.link(&contexts, p.context)?;
            let mut local = BTreeSet::new();
            for &item in &p.items {
                work.tick()?;
                if !local.insert(item) {
                    return Err(Error::DuplicateId);
                }
                items.insert(item);
            }
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
            if let Some(t) = p.transform {
                for (rep, item) in [(p.rep_1, t.item_1), (p.rep_2, t.item_2)] {
                    work.member(&parts.representations[representations[&rep]].items, item)?;
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
            work.member(
                &parts.representations[representations[&p.representation]].items,
                p.origin,
            )?;
        }
        for p in &parts.mapped_items {
            work.link(&maps, p.map)?;
            work.tick()?;
            if !items.contains(&p.id) || !items.contains(&p.target) {
                return Err(Error::MissingLink);
            }
        }
        let mut mapping_edges = Vec::new();
        for p in &parts.representations {
            for item in &p.items {
                work.tick()?;
                if let Some(&i) = mapped.get(item) {
                    let m = &parts.mapped_items[i];
                    work.member(&p.items, m.target)?;
                    mapping_edges.push((p.id, parts.maps[maps[&m.map]].representation));
                }
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
        Ok(Self { parts, roots })
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
    fn member<K: PartialEq>(&mut self, items: &[K], key: K) -> Result<(), Error> {
        for item in items {
            self.tick()?;
            if *item == key {
                return Ok(());
            }
        }
        Err(Error::MissingLink)
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
