use super::*;
impl Context<'_> {
    /// Value equality ignores physical source spans and nominal numeric aliases.
    /// Entity equality is identity equality; referenced graphs are never expanded.
    pub(super) fn equal(
        &mut self,
        domain: &Domain,
        left: &StepValue,
        right: &StepValue,
        depth: usize,
    ) -> Result<bool, Error> {
        self.tick()?;
        if depth >= self.limits.max_depth.min(128) {
            return Err(self.error(ErrorKind::ResourceLimit, "aggregate equality depth budget"));
        }
        if matches!(left.kind, ValueKind::Null) || matches!(right.kind, ValueKind::Null) {
            return Ok(false);
        }
        match domain {
            Domain::Named(id) => match self.declaration(*id)? {
                DeclarationKind::Type { domain, .. } => self.equal(&domain, left, right, depth + 1),
                DeclarationKind::Entity { .. } => Ok(
                    matches!((&left.kind,&right.kind), (ValueKind::Reference(a),ValueKind::Reference(b)) if a==b),
                ),
                _ => Err(self.error(ErrorKind::InvalidMetadata, "invalid equality domain")),
            },
            Domain::Select(_) => {
                let (a, av, at) = self.selected_domain(left, depth + 1)?;
                let (b, bv, bt) = self.selected_domain(right, depth + 1)?;
                // Different enumeration declarations are distinct domains even if
                // their enumerator spellings happen to coincide.
                if matches!(a, Domain::Enumeration(_)) && at != bt {
                    return Ok(false);
                }
                match (a, b) {
                    (Domain::Builtin { kind: ka, .. }, Domain::Builtin { kind: kb, .. })
                        if ka == kb
                            || (matches!(ka, Builtin::Boolean | Builtin::Logical)
                                && matches!(kb, Builtin::Boolean | Builtin::Logical))
                            || (matches!(
                                ka,
                                Builtin::Integer | Builtin::Real | Builtin::Number
                            ) && matches!(
                                kb,
                                Builtin::Integer | Builtin::Real | Builtin::Number
                            )) =>
                    {
                        self.equal(&a, av, bv, depth + 1)
                    }
                    (Domain::Named(_), Domain::Named(_)) => self.equal(&a, av, bv, depth + 1),
                    (Domain::Enumeration(_), Domain::Enumeration(_)) => {
                        self.equal(&a, av, bv, depth + 1)
                    }
                    (Domain::Aggregate { .. }, Domain::Aggregate { .. }) if at == bt => {
                        self.equal(&a, av, bv, depth + 1)
                    }
                    (Domain::Aggregate { .. }, Domain::Aggregate { .. }) => Err(self.error(
                        ErrorKind::Unsupported,
                        "SELECT aggregate equality across distinct defined types",
                    )),
                    _ => Ok(false),
                }
            }
            Domain::Aggregate { kind, element, .. } => {
                let (ValueKind::Aggregate(a), ValueKind::Aggregate(b)) = (&left.kind, &right.kind)
                else {
                    return Ok(false);
                };
                if a.len() != b.len() {
                    return Ok(false);
                }
                if matches!(kind, AggregateKind::Array | AggregateKind::List) {
                    for (a, b) in a.iter().zip(b) {
                        if !self.equal(element, a, b, depth + 1)? {
                            return Ok(false);
                        }
                    }
                } else {
                    let mut used = Vec::new();
                    for _ in b {
                        self.tick()?;
                        used.push(false);
                    }
                    for a in a {
                        let mut found = false;
                        for (i, b) in b.iter().enumerate() {
                            self.tick()?;
                            if !used[i] && self.equal(element, a, b, depth + 1)? {
                                used[i] = true;
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
            _ => Ok(match (&left.kind, &right.kind) {
                (ValueKind::Integer(a), ValueKind::Integer(b)) => a == b,
                (ValueKind::Real(a), ValueKind::Real(b)) => a == b,
                (ValueKind::Integer(a), ValueKind::Real(b))
                | (ValueKind::Real(b), ValueKind::Integer(a)) => {
                    b.fract() == 0.0
                        && *b >= i64::MIN as f64
                        && *b < -(i64::MIN as f64)
                        && (*b as i64) == *a
                }
                (ValueKind::String(a), ValueKind::String(b)) => {
                    for _ in a.bytes().zip(b.bytes()) {
                        self.tick()?;
                    }
                    a == b
                }
                (ValueKind::Binary(a), ValueKind::Binary(b)) => {
                    for _ in a.bytes.iter().zip(&b.bytes) {
                        self.tick()?;
                    }
                    a == b
                }
                (ValueKind::Enumeration(a), ValueKind::Enumeration(b)) => a.eq_ignore_ascii_case(b),
                _ => false,
            }),
        }
    }
    fn selected_domain<'v>(
        &mut self,
        value: &'v StepValue,
        depth: usize,
    ) -> Result<(Domain, &'v StepValue, Option<DeclarationId>), Error> {
        if depth >= self.limits.max_depth.min(128) {
            return Err(self.error(ErrorKind::ResourceLimit, "SELECT equality depth budget"));
        }
        if let ValueKind::Reference(id) = value.kind {
            let declaration = self
                .types
                .get(&id)
                .and_then(|ids| ids.first())
                .copied()
                .ok_or_else(|| {
                    self.error(ErrorKind::MissingReference, "missing SELECT reference")
                })?;
            return Ok((Domain::Named(declaration), value, None));
        }
        let ValueKind::Typed { type_name, value } = &value.kind else {
            return Err(self.error(ErrorKind::TypeMismatch, "expected SELECT tag"));
        };
        let mut id = self
            .lookup(type_name)?
            .ok_or_else(|| self.error(ErrorKind::TypeMismatch, "unknown SELECT tag"))?;
        for _ in depth..self.limits.max_depth.min(128) {
            match self.declaration(id)? {
                DeclarationKind::Type {
                    domain: Domain::Named(next),
                    ..
                } => id = next,
                DeclarationKind::Type {
                    domain: Domain::Select(_),
                    ..
                } => return self.selected_domain(value, depth + 1),
                DeclarationKind::Type { domain, .. } => return Ok((domain, value, Some(id))),
                _ => {
                    return Err(
                        self.error(ErrorKind::InvalidMetadata, "invalid SELECT equality type")
                    );
                }
            }
        }
        Err(self.error(ErrorKind::ResourceLimit, "SELECT alias depth budget"))
    }
}
