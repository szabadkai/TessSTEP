use crate::SourceSpan;
use std::{fmt, num::NonZeroU64, sync::Arc};

/// Physical-file entity identifier, independent of storage slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(NonZeroU64);
impl EntityId {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(NonZeroU64);
impl ValueId {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Occurrence {
    Entity(EntityId),
    Value(ValueId),
}
impl Occurrence {
    pub const fn number(self) -> u64 {
        match self {
            Self::Entity(id) => id.get(),
            Self::Value(id) => id.get(),
        }
    }
}
impl fmt::Display for Occurrence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entity(id) => write!(f, "{id}"),
            Self::Value(id) => write!(f, "@{}", id.get()),
        }
    }
}

/// Interned per parser; cloning does not copy text.
pub type Symbol = Arc<str>;

/// Exact binary payload, including non-byte-aligned lengths. Bits are packed
/// most-significant-bit first, with zero padding at the end of the last byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryValue {
    pub bytes: Vec<u8>,
    pub bit_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StepValue {
    pub kind: ValueKind,
    pub source: SourceSpan,
}
#[derive(Debug, Clone, PartialEq)]
pub enum ValueKind {
    Null,
    Omitted,
    Integer(i64),
    Real(f64),
    String(String),
    Binary(BinaryValue),
    Enumeration(Symbol),
    Reference(EntityId),
    ValueReference(ValueId),
    ConstantEntity(Symbol),
    ConstantValue(Symbol),
    Aggregate(Vec<StepValue>),
    Typed {
        type_name: Symbol,
        value: Box<StepValue>,
    },
    /// Permitted in anchor items, never in ordinary entity parameters.
    Resource(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub name: Symbol,
    pub parameters: Vec<StepValue>,
    pub source: SourceSpan,
}
#[derive(Debug, Clone, PartialEq)]
pub enum EntityKind {
    Simple(Record),
    Complex(Vec<Record>),
}
impl EntityKind {
    pub fn records(&self) -> &[Record] {
        match self {
            Self::Simple(record) => std::slice::from_ref(record),
            Self::Complex(records) => records,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct EntityInstance {
    pub id: EntityId,
    pub kind: EntityKind,
    pub source: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Anchor {
    pub name: String,
    pub value: StepValue,
    pub tags: Vec<(String, StepValue)>,
    pub source: SourceSpan,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalReference {
    pub id: Occurrence,
    pub resource: String,
    pub source: SourceSpan,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub base64: String,
    pub source: SourceSpan,
}
