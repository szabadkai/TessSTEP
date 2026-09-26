use super::*;
use std::collections::BTreeSet;
/// A record's declaration and its `(owner, attribute)` list in parameter order.
type RecordAttributes<'a> = (DeclarationId, Vec<(DeclarationId, &'a Attribute)>);
impl<'a> Context<'a> {
    /// Postorder in SUBTYPE OF order; diamonds emit each ancestor once.
    pub(super) fn hierarchy(&mut self, root: DeclarationId) -> Result<Vec<DeclarationId>, Error> {
        let mut stack = vec![(root, false, 0)];
        let mut active = BTreeSet::new();
        let mut done = BTreeSet::new();
        let mut result = Vec::new();
        while let Some((id, exit, depth)) = stack.pop() {
            self.tick()?;
            if exit {
                active.remove(&id);
                done.insert(id);
                result.push(id);
                continue;
            }
            if active.contains(&id) {
                return Err(self.error(ErrorKind::InvalidMetadata, "cyclic entity inheritance"));
            }
            if done.contains(&id) {
                continue;
            }
            if depth >= self.limits.max_depth.min(128) {
                return Err(self.error(ErrorKind::ResourceLimit, "inheritance depth budget"));
            }
            let DeclarationKind::Entity { supertypes, .. } = self.declaration(id)? else {
                return Err(self.error(ErrorKind::InvalidMetadata, "supertype is not an entity"));
            };
            self.local_attributes(id)?;
            active.insert(id);
            stack.push((id, true, depth));
            for parent in supertypes.iter().rev() {
                self.tick()?;
                stack.push((*parent, false, depth + 1));
            }
        }
        Ok(result)
    }
    pub(super) fn local_attributes(&mut self, id: DeclarationId) -> Result<&'a [Attribute], Error> {
        let DeclarationKind::Entity {
            attributes,
            unique,
            where_rules,
            supertype_constraint,
            ..
        } = self.declaration(id)?
        else {
            return Err(self.error(ErrorKind::InvalidMetadata, "expected an entity declaration"));
        };
        if !unique.is_empty() || !where_rules.is_empty() || supertype_constraint.is_some() {
            return Err(self.error(
                ErrorKind::Unsupported,
                "entity rules or supertype constraints",
            ));
        }
        for attribute in attributes {
            self.tick()?;
            if !matches!(attribute.kind, AttributeKind::Explicit) {
                return Err(self.error(ErrorKind::Unsupported, "DERIVE or INVERSE attribute"));
            }
        }
        Ok(attributes)
    }
    /// A record's declaration and explicit attributes in physical parameter order:
    /// the full inherited list for internal mapping, local attributes for external.
    pub(super) fn record_attributes(
        &mut self,
        kind: &EntityKind,
        record: &Record,
    ) -> Result<RecordAttributes<'a>, Error> {
        self.source = Some(record.source);
        let id = self
            .lookup(&record.name)?
            .ok_or_else(|| self.error(ErrorKind::UnknownEntity, "unknown entity name"))?;
        let owners = if matches!(kind, EntityKind::Simple(_)) {
            self.hierarchy(id)?
        } else {
            vec![id]
        };
        let mut attributes = Vec::new();
        for owner in owners {
            for attribute in self.local_attributes(owner)? {
                self.tick()?;
                attributes.push((owner, attribute));
            }
        }
        Ok((id, attributes))
    }
    /// Whether an instance attribute is a resolved link slot `(entity, owner, name)`.
    pub(super) fn is_link(
        &mut self,
        links: &[(DeclarationId, DeclarationId, &'static str)],
        types: &[DeclarationId],
        owner: DeclarationId,
        attribute: &Attribute,
    ) -> Result<bool, Error> {
        for &(entity, declared, name) in links {
            self.tick()?;
            if declared == owner && name == attribute.name && types.contains(&entity) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(super) fn instance_types(
        &mut self,
        kind: &EntityKind,
    ) -> Result<Vec<DeclarationId>, Error> {
        let mut ids = Vec::new();
        let mut included = BTreeSet::new();
        let mut inherited = BTreeSet::new();
        let mut previous: Option<&str> = None;
        for record in kind.records() {
            self.source = Some(record.source);
            let id = self
                .lookup(&record.name)?
                .ok_or_else(|| self.error(ErrorKind::UnknownEntity, "unknown entity name"))?;
            let DeclarationKind::Entity { .. } = self.declaration(id)? else {
                return Err(self.error(ErrorKind::UnknownEntity, "record name is not an entity"));
            };
            let name = self
                .schemas
                .declaration(id)
                .expect("checked declaration")
                .name;
            if matches!(kind, EntityKind::Complex(_)) {
                if previous.is_some_and(|p| {
                    p.bytes()
                        .map(|c| c.to_ascii_uppercase())
                        .cmp(name.bytes().map(|c| c.to_ascii_uppercase()))
                        .is_ge()
                }) {
                    return Err(self.error(
                        ErrorKind::ComplexMapping,
                        "complex components must follow canonical entity name order",
                    ));
                }
                previous = Some(name);
            }
            if !included.insert(id) {
                return Err(self.error(
                    ErrorKind::ComplexMapping,
                    "duplicate complex component identity",
                ));
            }
            ids.push(id);
            let hierarchy = self.hierarchy(id)?;
            for ancestor in hierarchy.iter().filter(|&&ancestor| ancestor != id) {
                self.tick()?;
                inherited.insert(*ancestor);
            }
            if matches!(kind, EntityKind::Simple(_)) {
                self.concrete_leaf(id)?;
                return Ok(hierarchy);
            }
        }
        for id in &inherited {
            self.tick()?;
            if !included.contains(id) {
                return Err(self.error(
                    ErrorKind::ComplexMapping,
                    "complex mapping omits a required supertype component",
                ));
            }
        }
        let mut leaves = 0;
        for id in &ids {
            self.tick()?;
            if !inherited.contains(id) {
                self.concrete_leaf(*id)?;
                leaves += 1;
            }
        }
        if leaves < 2 {
            return Err(self.error(
                ErrorKind::ComplexMapping,
                "a single-leaf instance requires internal mapping",
            ));
        }
        // A complex instance must form one connected inheritance graph.
        let mut connected = BTreeSet::new();
        if let Some(id) = ids.first() {
            connected.insert(*id);
        }
        loop {
            let before = connected.len();
            for id in &ids {
                let DeclarationKind::Entity { supertypes, .. } = self.declaration(*id)? else {
                    unreachable!()
                };
                for parent in supertypes {
                    self.tick()?;
                    if connected.contains(id) || connected.contains(parent) {
                        connected.insert(*id);
                        connected.insert(*parent);
                    }
                }
            }
            if before == connected.len() {
                break;
            }
        }
        if connected.len() != ids.len() {
            return Err(self.error(ErrorKind::ComplexMapping, "unrelated complex components"));
        }
        Ok(ids)
    }
    fn concrete_leaf(&mut self, id: DeclarationId) -> Result<(), Error> {
        if matches!(
            self.declaration(id)?,
            DeclarationKind::Entity {
                abstract_entity: true,
                ..
            }
        ) {
            return Err(self.error(
                ErrorKind::AbstractEntity,
                "abstract entity cannot be a leaf instance",
            ));
        }
        Ok(())
    }
}
