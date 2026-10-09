use super::*;
impl Context<'_> {
    pub(super) fn select_value(
        &mut self,
        alternatives: &[DeclarationId],
        value: &StepValue,
        depth: usize,
    ) -> Result<(), Error> {
        let typed_id =
            if let ValueKind::Typed { type_name, .. } = &value.kind {
                Some(self.lookup(type_name)?.ok_or_else(|| {
                    self.error(ErrorKind::TypeMismatch, "unknown SELECT type tag")
                })?)
            } else {
                None
            };
        // A bare simple value where the SELECT requires a type tag (policy only).
        let untyped = typed_id.is_none()
            && self.policy.accept_untyped_select
            && matches!(
                value.kind,
                ValueKind::Integer(_)
                    | ValueKind::Real(_)
                    | ValueKind::String(_)
                    | ValueKind::Enumeration(_)
                    | ValueKind::Binary(_)
            );
        let mut stack = Vec::new();
        for id in alternatives.iter().rev() {
            self.tick()?;
            stack.push((*id, depth));
        }
        while let Some((id, depth)) = stack.pop() {
            self.tick()?;
            if depth >= self.limits.max_depth.min(128) {
                return Err(self.error(ErrorKind::ResourceLimit, "SELECT traversal depth budget"));
            }
            match self.declaration(id)? {
                DeclarationKind::Entity { .. } if matches!(value.kind, ValueKind::Reference(_)) => {
                    match self.reference(id, value) {
                        Ok(()) => return Ok(()),
                        Err(e) if e.kind == ErrorKind::ReferenceType => (),
                        Err(e) => return Err(e),
                    }
                }
                DeclarationKind::Type {
                    domain,
                    where_rules,
                } => {
                    if !where_rules.is_empty() {
                        if !self.policy.retain_unevaluated {
                            return Err(
                                self.error(ErrorKind::Unsupported, "SELECT type WHERE constraint")
                            );
                        }
                        self.retained.rules += where_rules.len();
                    }
                    if let Domain::Select(nested) = domain {
                        for nested in nested.iter().rev() {
                            self.tick()?;
                            stack.push((*nested, depth + 1));
                        }
                    } else if typed_id == Some(id) {
                        let ValueKind::Typed { value: inner, .. } = &value.kind else {
                            unreachable!()
                        };
                        return self.value(&domain, inner, false, depth + 1);
                    } else if untyped {
                        let retained = self.retained;
                        match self.value(&domain, value, false, depth + 1) {
                            Ok(()) => {
                                self.retained.untyped_select += 1;
                                return Ok(());
                            }
                            Err(e)
                                if matches!(e.kind, ErrorKind::TypeMismatch | ErrorKind::Width) =>
                            {
                                self.retained = retained;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
                DeclarationKind::Entity { .. } => (),
                _ => {
                    return Err(
                        self.error(ErrorKind::InvalidMetadata, "invalid SELECT alternative")
                    );
                }
            }
        }
        Err(self.error(
            ErrorKind::TypeMismatch,
            "value is not a permitted SELECT alternative",
        ))
    }
}
