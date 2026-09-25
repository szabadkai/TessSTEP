//! TessSTEP: a memory-safe STEP/EXPRESS geometry kernel and adaptive tessellator.
//!
//! This release exposes physical parsing, generic storage, the EXPRESS frontend,
//! deterministic Rust generation and static schema reflection.
//! A bounded structural instance decoder is available in `model::decode`.
//! A bounded product graph adapter is available in `ap242`, with an independent
//! owned model in `product`. No AP conformance, geometry evaluation or tessellation
//! is implemented. Placements remain ordered descriptions.
//!
//! ```
//! use tessstep::{parse, ParseLimits};
//! let input = concat!(
//!     "ISO-10303-21;HEADER;FILE_DESCRIPTION(('example'),'2;1');",
//!     "FILE_NAME('','',('author'),('org'),'','','');",
//!     "FILE_SCHEMA(('EXAMPLE'));ENDSEC;DATA;#1=ITEM(#2);#2=ITEM($);",
//!     "ENDSEC;END-ISO-10303-21;"
//! );
//! let doc = parse(input.as_bytes(), ParseLimits::default())?;
//! assert_eq!(doc.entities().len(), 2);
//! assert!(doc.diagnostics().is_empty());
//! # Ok::<(), tessstep::Diagnostic>(())
//! ```
#![forbid(unsafe_code)]

pub use tessstep_ap242 as ap242;
pub use tessstep_codegen as codegen;
pub use tessstep_express as express;
pub use tessstep_model as model;
pub use tessstep_model::{Document, parse};
pub use tessstep_part21 as part21;
pub use tessstep_part21::{Diagnostic, ParseLimits};
pub use tessstep_product as product;
pub use tessstep_schema as schema;
