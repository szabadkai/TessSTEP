use crate::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug)]
pub struct Source<'a> {
    pub name: &'a str,
    pub text: &'a str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclarationId(pub usize);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compilation {
    pub sources: Vec<String>,
    pub schemas: Vec<Schema>,
    /// Present only when parsing and all implemented semantic checks succeeded.
    /// Unsupported diagnostics still require downstream policy decisions.
    pub ir: Option<SchemaIr>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaIr {
    pub schemas: Vec<IrSchema>,
    pub declarations: Vec<IrDeclaration>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrSchema {
    pub name: String,
    pub span: Span,
    pub symbols: BTreeMap<String, DeclarationId>,
    pub exports: BTreeMap<String, DeclarationId>,
    pub dependencies: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrDeclaration {
    pub name: String,
    pub span: Span,
    /// Indices into Compilation.schemas and its declarations preserve the complete AST.
    pub schema: usize,
    pub ast_declaration: usize,
    pub kind: IrKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrKind {
    Entity {
        supertypes: Vec<DeclarationId>,
        attributes: Vec<IrAttribute>,
    },
    Type(IrType),
    Constant(IrType),
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrAttribute {
    pub name: String,
    pub span: Span,
    pub ty: IrType,
    pub optional: bool,
    pub kind: AttributeKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrType {
    Builtin {
        name: String,
        width: Option<Expression>,
        fixed: bool,
    },
    Named(DeclarationId),
    Aggregate {
        kind: AggregateKind,
        bounds: Option<(Expression, Expression)>,
        optional: bool,
        unique: bool,
        element: Box<IrType>,
    },
    Enumeration(Vec<Name>),
    Select(Vec<DeclarationId>),
}

/// Compile only explicitly supplied sources. No filesystem/network search or schema guessing.
/// Budgets cover the complete source set. Diagnostics sort by source and byte position.
pub fn compile(sources: &[Source<'_>], limits: Limits) -> Compilation {
    let mut result = Compilation {
        sources: Vec::new(),
        schemas: Vec::new(),
        ir: None,
        diagnostics: Vec::new(),
    };
    let bytes = sources
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.text.len()));
    if sources.len() > limits.max_schemas || bytes.is_none_or(|n| n > limits.max_input_bytes) {
        result
            .diagnostics
            .push(limit(Span::default(), "source set"));
        return result;
    }
    result.sources = sources.iter().map(|s| s.name.to_string()).collect();
    let mut remaining = limits;
    for (source, input) in sources.iter().enumerate() {
        match crate::parser::parse_counted(source, input.text, remaining) {
            Ok((schemas, tokens, clone_work)) => {
                remaining.max_tokens -= tokens;
                remaining.max_work -= clone_work;
                remaining.max_schemas -= schemas.len();
                remaining.max_declarations -=
                    schemas.iter().map(|s| s.declarations.len()).sum::<usize>();
                result.schemas.extend(schemas);
            }
            Err(d) => result.diagnostics.push(d),
        }
    }
    if sources.is_empty() {
        result
            .diagnostics
            .push(Diagnostic::error("EX1003", Span::default(), "no sources"));
    }
    if result.diagnostics.is_empty() {
        let mut validator = Validator {
            schemas: &result.schemas,
            limits: remaining,
            work: 0,
            diagnostics: Vec::new(),
            declarations: Vec::new(),
            symbols: Vec::new(),
            exports: Vec::new(),
        };
        match validator.run() {
            Ok(ir) => {
                if !validator
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == Severity::Error)
                {
                    result.ir = Some(ir);
                }
            }
            Err(d) => validator.diagnostics.push(d),
        }
        result.diagnostics = validator.diagnostics;
    }
    result.diagnostics.sort_by(|a, b| {
        (a.span.source, a.span.start, a.code, &a.message).cmp(&(
            b.span.source,
            b.span.start,
            b.code,
            &b.message,
        ))
    });
    result
}
type Symbols = BTreeMap<String, BTreeSet<DeclarationId>>;
struct Validator<'a> {
    schemas: &'a [Schema],
    limits: Limits,
    work: usize,
    diagnostics: Vec<Diagnostic>,
    declarations: Vec<(usize, usize)>,
    symbols: Vec<Symbols>,
    exports: Vec<Symbols>,
}
impl Validator<'_> {
    fn charge(&mut self, span: Span) -> Result<(), Diagnostic> {
        self.charge_amount(span, 1)
    }
    fn charge_amount(&mut self, span: Span, amount: usize) -> Result<(), Diagnostic> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or_else(|| limit(span, "semantic work"))?;
        if self.work > self.limits.max_work {
            return Err(limit(span, "semantic work"));
        }
        Ok(())
    }
    fn error(&mut self, code: &'static str, span: Span, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(code, span, message));
    }
    fn opaque(&mut self, expr: &Expression, context: &str) {
        self.diagnostics.push(Diagnostic::unsupported(
            expr.span,
            format!("{context} retained; expression semantics not validated"),
        ));
    }
    fn decl(&self, id: DeclarationId) -> &Declaration {
        let (s, d) = self.declarations[id.0];
        &self.schemas[s].declarations[d]
    }
    fn is_type(&self, id: DeclarationId) -> bool {
        matches!(
            self.decl(id).kind,
            DeclarationKind::Entity(_) | DeclarationKind::Type { .. }
        )
    }
    fn run(&mut self) -> Result<SchemaIr, Diagnostic> {
        let mut schema_names = BTreeMap::new();
        for (s, schema) in self.schemas.iter().enumerate() {
            self.charge(schema.span)?;
            if schema_names.insert(schema.name.text.clone(), s).is_some() {
                self.error("EX2002", schema.name.span, "duplicate schema name");
            }
            let mut symbols = Symbols::new();
            for (d, decl) in schema.declarations.iter().enumerate() {
                self.charge_amount(decl.span, decl.name.text.len().saturating_add(1))?;
                let id = DeclarationId(self.declarations.len());
                self.declarations.push((s, d));
                let set = symbols.entry(decl.name.text.clone()).or_default();
                if !set.is_empty() {
                    self.error(
                        "EX2002",
                        decl.name.span,
                        format!("duplicate declaration {}", decl.name.text),
                    );
                }
                set.insert(id);
            }
            self.symbols.push(symbols.clone());
            self.exports.push(symbols);
        }
        let mut dependencies = vec![BTreeSet::new(); self.schemas.len()];
        for (s, schema) in self.schemas.iter().enumerate() {
            for import in &schema.imports {
                self.charge(import.span)?;
                match schema_names.get(&import.schema.text) {
                    Some(&target) if target != s => {
                        dependencies[s].insert(target);
                    }
                    Some(_) => {
                        self.error("EX2003", import.schema.span, "schema cannot import itself")
                    }
                    None => self.error(
                        "EX2003",
                        import.schema.span,
                        format!("missing supplied schema {}", import.schema.text),
                    ),
                }
            }
        }
        // Monotone fixed point permits cyclic schema dependencies without recursive traversal.
        loop {
            let mut changed = false;
            for (s, schema) in self.schemas.iter().enumerate() {
                for import in &schema.imports {
                    self.charge(import.span)?;
                    let Some(&target) = schema_names.get(&import.schema.text) else {
                        continue;
                    };
                    let mut pending = Vec::new();
                    if let Some(items) = &import.items {
                        for item in items {
                            self.charge(item.name.span)?;
                            if let Some(ids) = self.exports[target].get(&item.name.text) {
                                let ids: Vec<_> = ids.iter().copied().collect();
                                for id in ids {
                                    self.charge_amount(
                                        item.name.span,
                                        item.alias
                                            .as_ref()
                                            .unwrap_or(&item.name)
                                            .text
                                            .len()
                                            .saturating_add(1),
                                    )?;
                                    if import.kind == ImportKind::Reference || self.is_type(id) {
                                        pending.push((
                                            item.alias.as_ref().unwrap_or(&item.name).text.clone(),
                                            id,
                                        ));
                                    }
                                }
                            }
                        }
                    } else {
                        // Charge copies before snapshotting to bound wildcard expansion.
                        let cost = self.exports[target]
                            .iter()
                            .fold(0usize, |cost, (name, ids)| {
                                cost.saturating_add(name.len()).saturating_add(ids.len())
                            });
                        self.charge_amount(import.span, cost)?;
                        let entries: Vec<_> = self.exports[target]
                            .iter()
                            .map(|(n, ids)| (n.clone(), ids.clone()))
                            .collect();
                        for (name, ids) in entries {
                            for id in ids {
                                self.charge_amount(import.span, name.len().saturating_add(1))?;
                                if import.kind == ImportKind::Reference || self.is_type(id) {
                                    pending.push((name.clone(), id));
                                }
                            }
                        }
                    }
                    for (name, id) in pending {
                        changed |= self.symbols[s].entry(name.clone()).or_default().insert(id);
                        if import.kind == ImportKind::Use {
                            changed |= self.exports[s].entry(name).or_default().insert(id);
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for (s, schema) in self.schemas.iter().enumerate() {
            for import in &schema.imports {
                if let (Some(items), Some(&target)) =
                    (&import.items, schema_names.get(&import.schema.text))
                {
                    for item in items {
                        self.charge(item.name.span)?;
                        let valid = self.exports[target]
                            .get(&item.name.text)
                            .is_some_and(|ids| {
                                ids.iter().any(|id| {
                                    import.kind == ImportKind::Reference || self.is_type(*id)
                                })
                            });
                        if !valid {
                            self.error(
                                "EX2003",
                                item.name.span,
                                format!(
                                    "{} is not an eligible export of {}",
                                    item.name.text, import.schema.text
                                ),
                            );
                        }
                    }
                }
            }
            for (name, ids) in &self.symbols[s] {
                if ids.len() > 1 {
                    self.diagnostics.push(Diagnostic::error(
                        "EX2004",
                        schema.name.span,
                        format!("ambiguous visible declaration {name}"),
                    ));
                }
            }
        }
        let mut ir = SchemaIr {
            schemas: Vec::new(),
            declarations: Vec::new(),
        };
        for (s, schema) in self.schemas.iter().enumerate() {
            ir.schemas.push(IrSchema {
                name: schema.name.text.clone(),
                span: schema.span,
                symbols: single_symbols(&self.symbols[s]),
                exports: single_symbols(&self.exports[s]),
                dependencies: dependencies[s].iter().copied().collect(),
            });
            for (d, decl) in schema.declarations.iter().enumerate() {
                self.charge(decl.span)?;
                let kind = match &decl.kind {
                    DeclarationKind::Entity(e) => {
                        let mut supertypes = Vec::new();
                        let mut seen = BTreeSet::new();
                        for name in &e.supertypes {
                            if !seen.insert(&name.text) {
                                self.error("EX2002", name.span, "duplicate supertype");
                            }
                            if let Some(id) = self.resolve(s, name, true)? {
                                supertypes.push(id);
                            }
                        }
                        if let Some(expr) = &e.supertype {
                            self.opaque(expr, "SUPERTYPE constraint");
                        }
                        let mut attributes = Vec::new();
                        let mut names = BTreeSet::new();
                        for attr in &e.attributes {
                            self.charge(attr.span)?;
                            if !names.insert(&attr.name.text) {
                                self.error("EX2002", attr.name.span, "duplicate attribute");
                            }
                            if let AttributeKind::Derived(expr) = &attr.kind {
                                self.opaque(expr, "DERIVE");
                            }
                            let ty = self.ty(s, &attr.ty)?;
                            attributes.push(IrAttribute {
                                name: attr.name.text.clone(),
                                span: attr.span,
                                ty,
                                optional: attr.optional,
                                kind: attr.kind.clone(),
                            });
                        }
                        self.rules(&e.unique, "UNIQUE")?;
                        self.rules(&e.where_rules, "WHERE")?;
                        IrKind::Entity {
                            supertypes,
                            attributes,
                        }
                    }
                    DeclarationKind::Type {
                        underlying,
                        where_rules,
                    } => {
                        self.rules(where_rules, "WHERE")?;
                        IrKind::Type(self.ty(s, underlying)?)
                    }
                    DeclarationKind::Constant { ty, value } => {
                        self.opaque(value, "CONSTANT");
                        IrKind::Constant(self.ty(s, ty)?)
                    }
                    DeclarationKind::Unsupported { keyword, body } => {
                        self.opaque(body, keyword);
                        IrKind::Unsupported
                    }
                };
                ir.declarations.push(IrDeclaration {
                    name: decl.name.text.clone(),
                    span: decl.span,
                    schema: s,
                    ast_declaration: d,
                    kind,
                });
            }
        }
        self.cycles(&ir, true)?;
        self.cycles(&ir, false)?;
        // Defer graph-dependent checks if cycles or unresolved names already invalidate IR.
        if !self
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
        {
            self.attributes(&ir)?;
        }
        Ok(ir)
    }
    fn resolve(
        &mut self,
        schema: usize,
        name: &Name,
        entity_only: bool,
    ) -> Result<Option<DeclarationId>, Diagnostic> {
        self.charge(name.span)?;
        let id = self.symbols[schema]
            .get(&name.text)
            .filter(|s| s.len() == 1)
            .and_then(|s| s.first())
            .copied();
        if let Some(id) = id {
            let valid = if entity_only {
                matches!(self.decl(id).kind, DeclarationKind::Entity(_))
            } else {
                self.is_type(id)
            };
            if valid {
                return Ok(Some(id));
            }
            self.error(
                "EX2005",
                name.span,
                format!(
                    "{} must name {}",
                    name.text,
                    if entity_only {
                        "an entity"
                    } else {
                        "an entity or type"
                    }
                ),
            );
        } else {
            self.error(
                "EX2005",
                name.span,
                format!("unresolved or ambiguous name {}", name.text),
            );
        }
        Ok(None)
    }
    fn rules(&mut self, rules: &[Rule], context: &str) -> Result<(), Diagnostic> {
        let mut labels = BTreeSet::new();
        for rule in rules {
            self.charge(rule.expression.span)?;
            if let Some(label) = &rule.label {
                if !labels.insert(&label.text) {
                    self.error("EX2002", label.span, "duplicate rule label");
                }
            }
            self.opaque(&rule.expression, context);
        }
        Ok(())
    }
    fn ty(&mut self, schema: usize, ty: &TypeExpr) -> Result<IrType, Diagnostic> {
        self.charge(ty.span)?;
        Ok(match &ty.kind {
            TypeKind::Named(name) => match self.resolve(schema, name, false)? {
                Some(id) => IrType::Named(id),
                None => IrType::Select(Vec::new()), // Unobservable as valid IR after an error.
            },
            TypeKind::Builtin { name, width, fixed } => {
                if let Some(e) = width {
                    self.opaque(e, "width/precision");
                }
                IrType::Builtin {
                    name: name.clone(),
                    width: width.clone(),
                    fixed: *fixed,
                }
            }
            TypeKind::Aggregate {
                kind,
                bounds,
                optional,
                unique,
                element,
            } => {
                if let Some((lower, upper)) = bounds {
                    self.opaque(lower, "aggregate lower bound");
                    self.opaque(upper, "aggregate upper bound");
                    if let (Ok(lo), Ok(hi)) = (lower.text.parse::<i64>(), upper.text.parse::<i64>())
                    {
                        if hi < lo || (*kind != AggregateKind::Array && lo < 0) {
                            self.error("EX2006", ty.span, "invalid literal aggregate bounds");
                        }
                    }
                }
                IrType::Aggregate {
                    kind: *kind,
                    bounds: bounds.clone(),
                    optional: *optional,
                    unique: *unique,
                    element: Box::new(self.ty(schema, element)?),
                }
            }
            TypeKind::Enumeration(names) => {
                self.unique_names(names)?;
                IrType::Enumeration(names.clone())
            }
            TypeKind::Select(names) => {
                self.unique_names(names)?;
                let mut ids = Vec::new();
                let mut seen = BTreeSet::new();
                for name in names {
                    if let Some(id) = self.resolve(schema, name, false)? {
                        if !seen.insert(id) {
                            self.error(
                                "EX2002",
                                name.span,
                                "duplicate SELECT target through aliases",
                            );
                        }
                        ids.push(id);
                    }
                }
                IrType::Select(ids)
            }
        })
    }
    fn unique_names(&mut self, names: &[Name]) -> Result<(), Diagnostic> {
        let mut seen = BTreeSet::new();
        for n in names {
            self.charge(n.span)?;
            if !seen.insert(&n.text) {
                self.error("EX2002", n.span, "duplicate type member");
            }
        }
        Ok(())
    }
    fn cycles(&mut self, ir: &SchemaIr, inheritance: bool) -> Result<(), Diagnostic> {
        let n = ir.declarations.len();
        let mut edges = vec![Vec::new(); n];
        let mut degree = vec![0; n];
        for (i, decl) in ir.declarations.iter().enumerate() {
            let mut refs = Vec::new();
            match &decl.kind {
                IrKind::Entity { supertypes, .. } if inheritance => {
                    refs.extend(supertypes.iter().copied())
                }
                IrKind::Type(ty) if !inheritance => type_refs(ty, &mut refs),
                _ => {}
            }
            for id in refs {
                self.charge(decl.span)?;
                if !inheritance && !matches!(ir.declarations[id.0].kind, IrKind::Type(_)) {
                    continue;
                }
                edges[id.0].push(i);
                degree[i] += 1;
            }
        }
        let mut queue: VecDeque<_> = (0..n).filter(|i| degree[*i] == 0).collect();
        while let Some(i) = queue.pop_front() {
            self.charge(ir.declarations[i].span)?;
            for &next in &edges[i] {
                degree[next] -= 1;
                if degree[next] == 0 {
                    queue.push_back(next);
                }
            }
        }
        for (i, degree) in degree.iter().enumerate() {
            if *degree != 0 {
                self.error(
                    "EX2007",
                    ir.declarations[i].span,
                    if inheritance {
                        "cyclic inheritance or dependency on an inheritance cycle"
                    } else {
                        "cyclic defined type or dependency on a type cycle"
                    },
                );
            }
        }
        Ok(())
    }
    /// Gather unique declarations in a hierarchy: a diamond visits each ancestor once.
    fn hierarchy(
        &mut self,
        ir: &SchemaIr,
        root: DeclarationId,
    ) -> Result<BTreeSet<DeclarationId>, Diagnostic> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            self.charge(ir.declarations[id.0].span)?;
            if seen.insert(id) {
                if let IrKind::Entity { supertypes, .. } = &ir.declarations[id.0].kind {
                    stack.extend(supertypes);
                }
            }
        }
        Ok(seen)
    }
    fn entity_target(
        &mut self,
        ir: &SchemaIr,
        ty: &IrType,
        span: Span,
    ) -> Result<Option<DeclarationId>, Diagnostic> {
        let mut ty = ty;
        let mut aggregate = false;
        loop {
            self.charge(span)?;
            match ty {
                IrType::Aggregate { kind, element, .. }
                    if !aggregate && matches!(kind, AggregateKind::Set | AggregateKind::Bag) =>
                {
                    aggregate = true;
                    ty = element;
                }
                IrType::Named(id) => match &ir.declarations[id.0].kind {
                    IrKind::Entity { .. } => return Ok(Some(*id)),
                    IrKind::Type(next) => ty = next,
                    _ => return Ok(None),
                },
                _ => return Ok(None),
            }
        }
    }
    fn attributes(&mut self, ir: &SchemaIr) -> Result<(), Diagnostic> {
        for (i, decl) in ir.declarations.iter().enumerate() {
            let IrKind::Entity { attributes, .. } = &decl.kind else {
                continue;
            };
            let hierarchy = self.hierarchy(ir, DeclarationId(i))?;
            let mut names = BTreeSet::new();
            for id in &hierarchy {
                if let IrKind::Entity { attributes, .. } = &ir.declarations[id.0].kind {
                    for a in attributes {
                        self.charge(a.span)?;
                        if !names.insert(&a.name) {
                            self.error(
                                "EX2008",
                                decl.span,
                                format!("inherited attribute conflict: {}", a.name),
                            );
                        }
                    }
                }
            }
            for a in attributes {
                let AttributeKind::Inverse { entity, attribute } = &a.kind else {
                    continue;
                };
                let Some(target) = self.entity_target(ir, &a.ty, a.span)? else {
                    self.error("EX2009", a.span, "INVERSE requires entity or one SET/BAG OF entity (including through aliases)");
                    continue;
                };
                let target_hierarchy = self.hierarchy(ir, target)?;
                let owner = if let Some(entity) = entity {
                    let Some(id) = self.resolve(decl.schema, entity, true)? else {
                        continue;
                    };
                    if !target_hierarchy.contains(&id) {
                        self.error(
                            "EX2009",
                            entity.span,
                            "FOR qualifier is outside inverse target hierarchy",
                        );
                        continue;
                    }
                    Some(id)
                } else {
                    None
                };
                let mut matches = Vec::new();
                for id in &target_hierarchy {
                    if owner.is_some_and(|owner| owner != *id) {
                        continue;
                    }
                    if let IrKind::Entity { attributes, .. } = &ir.declarations[id.0].kind {
                        for forward in attributes {
                            self.charge(forward.span)?;
                            if forward.name == attribute.text
                                && forward.kind == AttributeKind::Explicit
                            {
                                matches.push(forward);
                            }
                        }
                    }
                }
                if matches.len() != 1 {
                    self.error(
                        "EX2009",
                        attribute.span,
                        "FOR must resolve to one explicit attribute",
                    );
                } else {
                    // SELECT compatibility and full type/cardinality checking remain later semantics.
                    self.diagnostics.push(Diagnostic::unsupported(a.span, "INVERSE FOR resolved; forward type compatibility/cardinality not validated"));
                }
            }
        }
        Ok(())
    }
}
fn single_symbols(symbols: &Symbols) -> BTreeMap<String, DeclarationId> {
    symbols
        .iter()
        .filter_map(|(name, ids)| {
            if ids.len() == 1 {
                Some((name.clone(), *ids.first().unwrap()))
            } else {
                None
            }
        })
        .collect()
}
fn type_refs(ty: &IrType, refs: &mut Vec<DeclarationId>) {
    match ty {
        IrType::Named(id) => refs.push(*id),
        IrType::Aggregate { element, .. } => type_refs(element, refs),
        IrType::Select(ids) => refs.extend(ids),
        _ => {}
    }
}
