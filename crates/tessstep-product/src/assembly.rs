use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRelationship {
    pub relationship: RelationshipId,
    /// Acts on SI coordinates: representation 1 -> representation 2.
    pub rep_1_to_rep_2: Transform,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMapping {
    pub item: ItemId,
    pub using_representation: RepresentationId,
    /// Source representation SI coordinates -> using representation SI coordinates.
    pub source_to_using: Transform,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Uncertainty {
    pub source: u64,
    pub context: ContextId,
    pub dimension: Dimension,
    pub value_si: f64,
    pub name: String,
    pub description: Option<String>,
}
#[derive(Debug, Clone, Copy)]
pub struct ExpansionLimits {
    pub max_work: usize,
    pub max_instances: usize,
    pub max_depth: usize,
}
impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            max_work: 5_000_000,
            max_instances: 100_000,
            max_depth: 1024,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    /// Index of the parent in this expansion; root has no parent or occurrence.
    pub parent: Option<usize>,
    pub occurrence: Option<OccurrenceId>,
    pub definition: DefinitionId,
    pub representation: RepresentationId,
    pub local_to_parent: Transform,
    pub local_to_world: Transform,
}
impl Model {
    pub fn representations_of(
        &self,
        definition: DefinitionId,
    ) -> Option<&BTreeSet<RepresentationId>> {
        self.definition_reps.get(&definition)
    }
    pub fn items_in_context(&self, context: ContextId) -> Option<&BTreeSet<ItemId>> {
        self.context_items.get(&context)
    }
    /// Explicit, bounded expansion of a definition DAG. Representation alternatives
    /// are never guessed: select a root representation and disambiguate occurrences
    /// with a relationship ID when needed. Missing placement is not identity.
    pub fn expand(
        &self,
        root: DefinitionId,
        representation: RepresentationId,
        selections: &BTreeMap<OccurrenceId, RelationshipId>,
        limits: ExpansionLimits,
    ) -> Result<Vec<Instance>, Error> {
        if !self
            .definition_reps
            .get(&root)
            .is_some_and(|r| r.contains(&representation))
        {
            return Err(Error::MissingLink);
        }
        let mut work = Budget(limits.max_work);
        let mut children: BTreeMap<DefinitionId, Vec<&Occurrence>> = BTreeMap::new();
        for o in &self.parts.occurrences {
            work.tick()?;
            children.entry(o.parent).or_default().push(o);
        }
        let relationships = index(self.parts.relationships.iter().map(|r| r.id), &mut work)?;
        let resolved = index(
            self.parts
                .relationship_transforms
                .iter()
                .map(|r| r.relationship),
            &mut work,
        )?;
        let representations = index(self.parts.representations.iter().map(|r| r.id), &mut work)?;
        let occurrences = index(self.parts.occurrences.iter().map(|o| o.id), &mut work)?;
        let mut placements: BTreeMap<OccurrenceId, Vec<RelationshipId>> = BTreeMap::new();
        for p in &self.parts.placements {
            work.tick()?;
            placements
                .entry(p.occurrence)
                .or_default()
                .push(p.relationship);
        }
        for (&id, &rel) in selections {
            work.link(&occurrences, id)?;
            work.tick()?;
            if !placements.get(&id).is_some_and(|v| v.contains(&rel)) {
                return Err(Error::MissingLink);
            }
        }
        let mapped = MappedPlacements::new(&self.parts, &mut work)?;
        let mut associated: BTreeMap<RepresentationId, Vec<RepresentationId>> = BTreeMap::new();
        for id in &self.parts.associations {
            work.tick()?;
            let r = &self.parts.relationships[relationships[id]];
            // Ownership association is broader than a known common coordinate frame.
            if self.parts.representations[representations[&r.rep_1]].context
                == self.parts.representations[representations[&r.rep_2]].context
            {
                associated.entry(r.rep_1).or_default().push(r.rep_2);
                associated.entry(r.rep_2).or_default().push(r.rep_1);
            }
        }
        let mut out = Vec::new();
        let mut stack = vec![(
            Instance {
                parent: None,
                occurrence: None,
                definition: root,
                representation,
                local_to_parent: Transform::identity(),
                local_to_world: Transform::identity(),
            },
            1,
        )];
        while let Some((instance, depth)) = stack.pop() {
            work.tick()?;
            if out.len() >= limits.max_instances || depth > limits.max_depth {
                return Err(Error::ResourceLimit);
            }
            instance
                .local_to_world
                .inverse(Default::default())
                .map_err(|_| Error::InvalidTransform)?;
            let mut same_frame = BTreeSet::new();
            let mut search = vec![instance.representation];
            while let Some(r) = search.pop() {
                work.tick()?;
                if same_frame.insert(r) {
                    if let Some(next) = associated.get(&r) {
                        for &r in next {
                            work.tick()?;
                            search.push(r);
                        }
                    }
                }
            }
            let parent_index = out.len();
            if let Some(children) = children.get(&instance.definition) {
                for o in children.iter().rev() {
                    work.tick()?;
                    let mut choice = None;
                    let candidates = match placements.get(&o.id) {
                        Some(candidates) => candidates.as_slice(),
                        // Without a context dependent placement, a mapped item may
                        // place the occurrence; see `MappedPlacements::place`.
                        None => {
                            let child_reps = self
                                .definition_reps
                                .get(&o.child)
                                .ok_or(Error::MissingLink)?;
                            choice = mapped.place(o.id, child_reps, &same_frame, &mut work)?;
                            &[]
                        }
                    };
                    for id in candidates {
                        work.tick()?;
                        if selections.get(&o.id).is_some_and(|selected| selected != id) {
                            continue;
                        }
                        let r = &self.parts.relationships[relationships[id]];
                        let child_reps = self
                            .definition_reps
                            .get(&o.child)
                            .ok_or(Error::MissingLink)?;
                        let forward =
                            same_frame.contains(&r.rep_2) && child_reps.contains(&r.rep_1);
                        let reverse =
                            same_frame.contains(&r.rep_1) && child_reps.contains(&r.rep_2);
                        if forward && reverse {
                            return Err(Error::AmbiguousPlacement);
                        }
                        if !forward && !reverse {
                            continue;
                        }
                        if choice.is_some() {
                            return Err(Error::AmbiguousPlacement);
                        }
                        let transform = self.parts.relationship_transforms
                            [*resolved.get(id).ok_or(Error::MissingPlacement)?]
                        .rep_1_to_rep_2;
                        let (rep, local) = if forward {
                            (r.rep_1, transform)
                        } else {
                            (
                                r.rep_2,
                                transform
                                    .inverse(Default::default())
                                    .map_err(|_| Error::InvalidTransform)?,
                            )
                        };
                        choice = Some((rep, local));
                    }
                    let (rep, local) = choice.ok_or(Error::MissingPlacement)?;
                    if stack.len().saturating_add(out.len()).saturating_add(1)
                        >= limits.max_instances
                    {
                        return Err(Error::ResourceLimit);
                    }
                    stack.push((
                        Instance {
                            parent: Some(parent_index),
                            occurrence: Some(o.id),
                            definition: o.child,
                            representation: rep,
                            local_to_parent: local,
                            local_to_world: local
                                .then(instance.local_to_world)
                                .map_err(|_| Error::InvalidTransform)?,
                        },
                        depth.checked_add(1).ok_or(Error::ResourceLimit)?,
                    ));
                }
            }
            out.push(instance);
        }
        Ok(out)
    }
}

/// Occurrence placements written as mapped items, the alternative to a context
/// dependent shape representation that CATIA- and Creo-style exporters write: the
/// occurrence's own shape representation holds a MAPPED_ITEM whose map's source is
/// a representation of the child, and a representation in the parent's frame holds
/// the same item. That holder's evaluated source-to-using map is the child-to-parent
/// placement. A mapped item that only the occurrence's representation holds places
/// nothing, since that representation's frame is not known to be the parent's.
struct MappedPlacements<'p> {
    own: BTreeMap<OccurrenceId, Vec<RepresentationId>>,
    items: BTreeMap<RepresentationId, &'p [ItemId]>,
    sources: BTreeMap<ItemId, RepresentationId>,
    transforms: BTreeMap<(ItemId, RepresentationId), Transform>,
}
impl<'p> MappedPlacements<'p> {
    fn new(parts: &'p Parts, work: &mut Budget) -> Result<Self, Error> {
        let mut shapes = BTreeMap::new();
        for s in &parts.shapes {
            work.tick()?;
            if let ShapeTarget::Occurrence(o) = s.target {
                shapes.insert(s.id, o);
            }
        }
        let mut own: BTreeMap<OccurrenceId, Vec<RepresentationId>> = BTreeMap::new();
        for b in &parts.shape_bindings {
            work.tick()?;
            if let Some(&o) = shapes.get(&b.shape) {
                own.entry(o).or_default().push(b.representation);
            }
        }
        let mut items = BTreeMap::new();
        for r in &parts.representations {
            work.tick()?;
            items.insert(r.id, r.items.as_slice());
        }
        let mut maps = BTreeMap::new();
        for m in &parts.maps {
            work.tick()?;
            maps.insert(m.id, m.representation);
        }
        let mut sources = BTreeMap::new();
        for m in &parts.mapped_items {
            work.tick()?;
            if let Some(&source) = maps.get(&m.map) {
                sources.insert(m.id, source);
            }
        }
        let mut transforms = BTreeMap::new();
        for t in &parts.mapped_transforms {
            work.tick()?;
            transforms.insert((t.item, t.using_representation), t.source_to_using);
        }
        Ok(Self {
            own,
            items,
            sources,
            transforms,
        })
    }
    /// The child representation and child-to-parent map of an occurrence placed by
    /// a mapped item; two distinct items that both qualify are ambiguous.
    fn place(
        &self,
        occurrence: OccurrenceId,
        child_reps: &BTreeSet<RepresentationId>,
        parent_frame: &BTreeSet<RepresentationId>,
        work: &mut Budget,
    ) -> Result<Option<(RepresentationId, Transform)>, Error> {
        let mut choice: Option<(ItemId, RepresentationId, Transform)> = None;
        for rep in self.own.get(&occurrence).into_iter().flatten() {
            work.tick()?;
            for &item in self.items.get(rep).copied().unwrap_or(&[]) {
                work.tick()?;
                let Some(&source) = self.sources.get(&item) else {
                    continue;
                };
                if !child_reps.contains(&source) {
                    continue;
                }
                let mut holder = None;
                for &r in parent_frame {
                    work.tick()?;
                    if let Some(&t) = self.transforms.get(&(item, r)) {
                        holder = Some(t);
                        break;
                    }
                }
                let Some(t) = holder else {
                    continue;
                };
                if choice.is_some_and(|(other, ..)| other != item) {
                    return Err(Error::AmbiguousPlacement);
                }
                choice = Some((item, source, t));
            }
        }
        Ok(choice.map(|(_, rep, t)| (rep, t)))
    }
}
