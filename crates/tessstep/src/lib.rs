//! TessSTEP: a memory-safe STEP/EXPRESS geometry kernel and adaptive tessellator.
//!
//! This release exposes physical parsing, generic storage, the EXPRESS frontend,
//! deterministic Rust generation and static schema reflection.
//! A bounded structural instance decoder is available in `model::decode`.
//! A bounded product graph adapter is available in `ap242`, with an independent
//! owned model in `product`. No AP conformance, STEP geometry adaptation or
//! STEP-to-mesh conversion is implemented. Product placements and bounded assembly expansion are implemented. Independent
//! coordinates, units, tolerances and affine transforms are available in `math`;
//! analytic lines/conics, derivatives, parameter spans and NURBS are in `curves`.
//! Analytic and tensor-product NURBS evaluators are available in `surfaces`.
//! `topology`, `trim` and `tessellate` provide constructed structural B-reps,
//! supplied-pcurve UV reconstruction, adaptive regular face tessellation and shell/solid
//! assembly. `mesh` owns checked indexed assets and per-corner attributes.
//! `mesh::scene` shares assets across explicit nested affine occurrences and bakes instances.
//! `mesh::appearance` adds explicit color/opacity, face styles and inherited overrides.
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
pub use tessstep_curves as curves;
pub use tessstep_express as express;
pub use tessstep_math as math;
pub use tessstep_model as model;
pub use tessstep_model::{Document, parse};
pub use tessstep_part21 as part21;
pub use tessstep_part21::{Diagnostic, ParseLimits};
pub use tessstep_product as product;
pub use tessstep_schema as schema;

pub use tessstep_surfaces as surfaces;

pub use tessstep_tessellate as tessellate;
pub use tessstep_topology as topology;
pub use tessstep_trim as trim;

pub use tessstep_mesh as mesh;
