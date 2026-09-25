//! TessSTEP: a memory-safe STEP/EXPRESS geometry kernel and adaptive tessellator.
//!
//! This release exposes physical parsing, generic storage and the EXPRESS frontend.
//! No schema-aware instance validation, CAD interpretation or tessellation is implemented.
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

pub use tessstep_express as express;
pub use tessstep_model as model;
pub use tessstep_model::{Document, parse};
pub use tessstep_part21 as part21;
pub use tessstep_part21::{Diagnostic, ParseLimits};
