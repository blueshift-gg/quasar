//! Span-free diagnostic carried in tracked outputs.
//!
//! The on-disk parser diagnostics in `quasar-syntax` use `proc_macro2::Span`,
//! which doesn't satisfy Salsa's value-equality requirements for early
//! cutoff. `HirDiagnostic` lowers spans into byte ranges so tracked outputs
//! are comparable and hashable.

use crate::items::ByteRange;
use quasar_syntax::diagnostics::{DiagCode, Diagnostic, Severity};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HirDiagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub primary: ByteRange,
    pub labels: Vec<HirLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HirLabel {
    pub range: ByteRange,
    pub message: String,
}

impl HirDiagnostic {
    pub fn lower(d: Diagnostic) -> Self {
        Self {
            severity: d.severity,
            code: d.code,
            message: d.message,
            primary: ByteRange::from_span(d.primary),
            labels: d
                .labels
                .into_iter()
                .map(|l| HirLabel {
                    range: ByteRange::from_span(l.span),
                    message: l.message,
                })
                .collect(),
        }
    }
}
