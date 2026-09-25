use crate::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}
/// Opaque source, including interior whitespace/comments. No expression validity claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expression {
    pub text: String,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schema {
    pub name: Name,
    pub span: Span,
    pub imports: Vec<Import>,
    pub declarations: Vec<Declaration>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportKind {
    Use,
    Reference,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub kind: ImportKind,
    pub schema: Name,
    /// None imports all eligible exported declarations.
    pub items: Option<Vec<ImportItem>>,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportItem {
    pub name: Name,
    pub alias: Option<Name>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub name: Name,
    pub span: Span,
    pub kind: DeclarationKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationKind {
    Entity(Entity),
    Type {
        underlying: TypeExpr,
        where_rules: Vec<Rule>,
    },
    Constant {
        ty: TypeExpr,
        value: Expression,
    },
    Unsupported {
        keyword: String,
        body: Expression,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub abstract_entity: bool,
    pub supertype: Option<Expression>,
    pub supertypes: Vec<Name>,
    pub attributes: Vec<Attribute>,
    pub unique: Vec<Rule>,
    pub where_rules: Vec<Rule>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: Name,
    pub ty: TypeExpr,
    pub optional: bool,
    pub kind: AttributeKind,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeKind {
    Explicit,
    Derived(Expression),
    Inverse {
        entity: Option<Name>,
        attribute: Name,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub label: Option<Name>,
    pub expression: Expression,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateKind {
    Array,
    Bag,
    List,
    Set,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Builtin {
        name: String,
        width: Option<Expression>,
        fixed: bool,
    },
    Named(Name),
    Aggregate {
        kind: AggregateKind,
        bounds: Option<(Expression, Expression)>,
        optional: bool,
        unique: bool,
        element: Box<TypeExpr>,
    },
    Enumeration(Vec<Name>),
    Select(Vec<Name>),
}
