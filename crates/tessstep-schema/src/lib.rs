//! Immutable schema reflection and owned Rust binding contracts.
//!
//! Metadata preserves EXPRESS constraints as source, without evaluating them.
//! Typed references describe an intended entity type; they do not prove existence
//! or schema compatibility in a physical document. IDs are local to a schema set.
#![forbid(unsafe_code)]
use std::{fmt, marker::PhantomData, num::NonZeroU64};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclarationId(pub usize);

/// Implemented by generated entity records. Records are unchecked owned data.
pub trait EntityBinding {
    const DECLARATION: DeclarationId;
}
/// Generated for direct inheritance edges. Upcasts can be chained.
pub trait SubtypeOf<T: EntityBinding>: EntityBinding {}

/// A nonzero physical occurrence number, with no document ownership or lookup.
pub struct EntityRef<T: EntityBinding> {
    id: NonZeroU64,
    marker: PhantomData<fn() -> T>,
}
impl<T: EntityBinding> EntityRef<T> {
    pub const fn new(id: NonZeroU64) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }
    pub const fn id(self) -> NonZeroU64 {
        self.id
    }
    pub fn upcast<U: EntityBinding>(self) -> EntityRef<U>
    where
        T: SubtypeOf<U>,
    {
        EntityRef::new(self.id)
    }
}
impl<T: EntityBinding> Copy for EntityRef<T> {}
impl<T: EntityBinding> Clone for EntityRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: EntityBinding> PartialEq for EntityRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<T: EntityBinding> Eq for EntityRef<T> {}
impl<T: EntityBinding> fmt::Debug for EntityRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EntityRef").field(&self.id).finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logical {
    False,
    True,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    pub source: usize,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expression {
    pub text: &'static str,
    pub span: SourceSpan,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub label: Option<&'static str>,
    pub expression: Expression,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateKind {
    Array,
    Bag,
    List,
    Set,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Boolean,
    Logical,
    Integer,
    Number,
    Real,
    String,
    Binary,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Builtin {
        kind: Builtin,
        width: Option<Expression>,
        fixed: bool,
    },
    Named(DeclarationId),
    Aggregate {
        kind: AggregateKind,
        bounds: Option<(Expression, Expression)>,
        optional: bool,
        unique: bool,
        element: &'static Domain,
    },
    Enumeration(&'static [&'static str]),
    Select(&'static [DeclarationId]),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeKind {
    Explicit,
    Derived(Expression),
    Inverse {
        entity: Option<&'static str>,
        attribute: &'static str,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: &'static str,
    pub span: SourceSpan,
    pub domain: Domain,
    pub optional: bool,
    pub kind: AttributeKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationKind {
    Entity {
        abstract_entity: bool,
        supertype_constraint: Option<Expression>,
        supertypes: &'static [DeclarationId],
        /// Local attributes only, in declaration order, including DERIVE/INVERSE.
        attributes: &'static [Attribute],
        unique: &'static [Rule],
        where_rules: &'static [Rule],
    },
    Type {
        domain: Domain,
        where_rules: &'static [Rule],
    },
    Constant {
        domain: Domain,
        value: Expression,
    },
    Unsupported {
        keyword: &'static str,
        body: Expression,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub name: &'static str,
    pub schema: usize,
    pub span: SourceSpan,
    pub kind: DeclarationKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: &'static str,
    pub declaration: DeclarationId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schema {
    pub name: &'static str,
    pub span: SourceSpan,
    pub dependencies: &'static [usize],
    pub symbols: &'static [Symbol],
    pub exports: &'static [Symbol],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsupportedDiagnostic {
    pub code: &'static str,
    pub span: SourceSpan,
    pub message: &'static str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaSet {
    pub sources: &'static [&'static str],
    pub schemas: &'static [Schema],
    pub declarations: &'static [Declaration],
    pub unsupported: &'static [UnsupportedDiagnostic],
}
impl SchemaSet {
    pub fn schema(&self, name: &str) -> Option<&Schema> {
        self.schemas
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }
    pub fn declaration(&self, id: DeclarationId) -> Option<&Declaration> {
        self.declarations.get(id.0)
    }
}
impl Schema {
    pub fn lookup(&self, name: &str) -> Option<DeclarationId> {
        self.symbols
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .map(|s| s.declaration)
    }
}
