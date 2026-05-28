//! Quasar parser and AST, shared between `quasar-derive` (compile-time macro
//! emission) and `quasar-lsp` (interactive language analysis).
//!
//! The two consumers share one parser and one diagnostic type so they can
//! never drift in what they accept or what they report.

pub mod account;
pub mod accounts;
pub mod diagnostics;
pub mod span;
pub mod types;

pub use diagnostics::{DiagCode, DiagFamily, DiagLabel, Diagnostic, Diagnostics, Fix, Severity};
pub use span::LineIndex;
