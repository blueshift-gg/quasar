//! Diagnostic data shared by parser, resolver, and both consumers.

use proc_macro2::Span;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagCode {
    AccountAttrUnknownDirective,
    AccountAttrMalformedDirective,
    AccountAttrDuplicateDirective,
    AccountAttrDiscriminatorByteOutOfRange,
    AccountAttrMissingDiscriminatorOrUnsafe,
    AccountAttrImplementsRequiresOneOf,
    AccountAttrDiscriminatorWithUnsafe,
    AccountAttrOneOfWithDiscriminator,
    AccountsDirectiveUnknown,
    AccountsDirectiveMalformed,
    AccountsDirectiveDuplicate,
    AccountsBehaviorArgInvalid,
    AccountsBehaviorArgDuplicate,
    InstructionArgDuplicate,
    InstructionArgMalformed,
    UnknownAccountType,
    HasOneUnknownBinding,
    HasOneMissingAccountField,
    AccountsConstraintViolation,
}

impl DiagCode {
    pub fn family(self) -> DiagFamily {
        match self {
            DiagCode::AccountAttrUnknownDirective
            | DiagCode::AccountAttrMalformedDirective
            | DiagCode::AccountAttrDuplicateDirective
            | DiagCode::AccountAttrDiscriminatorByteOutOfRange
            | DiagCode::AccountAttrMissingDiscriminatorOrUnsafe
            | DiagCode::AccountAttrImplementsRequiresOneOf
            | DiagCode::AccountAttrDiscriminatorWithUnsafe
            | DiagCode::AccountAttrOneOfWithDiscriminator => DiagFamily::AccountAttr,
            DiagCode::AccountsDirectiveUnknown
            | DiagCode::AccountsDirectiveMalformed
            | DiagCode::AccountsDirectiveDuplicate
            | DiagCode::AccountsBehaviorArgInvalid
            | DiagCode::AccountsBehaviorArgDuplicate => DiagFamily::AccountsDirective,
            DiagCode::InstructionArgDuplicate | DiagCode::InstructionArgMalformed => {
                DiagFamily::InstructionArgs
            }
            DiagCode::UnknownAccountType
            | DiagCode::HasOneUnknownBinding
            | DiagCode::HasOneMissingAccountField
            | DiagCode::AccountsConstraintViolation => DiagFamily::Resolver,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DiagCode::AccountAttrUnknownDirective => "quasar::account_attr_unknown_directive",
            DiagCode::AccountAttrMalformedDirective => "quasar::account_attr_malformed_directive",
            DiagCode::AccountAttrDuplicateDirective => "quasar::account_attr_duplicate_directive",
            DiagCode::AccountAttrDiscriminatorByteOutOfRange => {
                "quasar::account_attr_discriminator_byte_out_of_range"
            }
            DiagCode::AccountAttrMissingDiscriminatorOrUnsafe => {
                "quasar::account_attr_missing_discriminator_or_unsafe"
            }
            DiagCode::AccountAttrImplementsRequiresOneOf => {
                "quasar::account_attr_implements_requires_one_of"
            }
            DiagCode::AccountAttrDiscriminatorWithUnsafe => {
                "quasar::account_attr_discriminator_with_unsafe"
            }
            DiagCode::AccountAttrOneOfWithDiscriminator => {
                "quasar::account_attr_one_of_with_discriminator"
            }
            DiagCode::AccountsDirectiveUnknown => "quasar::accounts_directive_unknown",
            DiagCode::AccountsDirectiveMalformed => "quasar::accounts_directive_malformed",
            DiagCode::AccountsDirectiveDuplicate => "quasar::accounts_directive_duplicate",
            DiagCode::AccountsBehaviorArgInvalid => "quasar::accounts_behavior_arg_invalid",
            DiagCode::AccountsBehaviorArgDuplicate => "quasar::accounts_behavior_arg_duplicate",
            DiagCode::InstructionArgDuplicate => "quasar::instruction_arg_duplicate",
            DiagCode::InstructionArgMalformed => "quasar::instruction_arg_malformed",
            DiagCode::UnknownAccountType => "quasar::unknown_account_type",
            DiagCode::HasOneUnknownBinding => "quasar::has_one_unknown_binding",
            DiagCode::HasOneMissingAccountField => "quasar::has_one_missing_account_field",
            DiagCode::AccountsConstraintViolation => "quasar::accounts_constraint_violation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagFamily {
    AccountAttr,
    AccountsDirective,
    InstructionArgs,
    Resolver,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub primary: Span,
    pub labels: Vec<DiagLabel>,
    pub fixes: Vec<Fix>,
}

#[derive(Debug, Clone)]
pub struct DiagLabel {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum Fix {
    InsertText {
        at: Span,
        text: String,
        title: String,
    },
    Replace {
        range: Span,
        with: String,
        title: String,
    },
    DeleteRange {
        range: Span,
        title: String,
    },
}

/// Collects diagnostics during parsing and resolution.
///
/// `emit()` deduplicates by `(primary span, code)`. `mark_parse_failed` /
/// `is_parse_failed` let the resolver skip emitting on input the parser
/// already reported as broken. [`dedup_subsume_narrower`](Self::dedup_subsume_narrower)
/// drops diagnostics whose primary span strictly contains another's of the
/// same [`DiagFamily`].
#[derive(Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    seen: HashSet<DedupKey>,
    parse_failed_spans: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct DedupKey {
    start: usize,
    end: usize,
    code: DiagCode,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, d: Diagnostic) {
        let r = d.primary.byte_range();
        let key = DedupKey {
            start: r.start,
            end: r.end,
            code: d.code,
        };
        if self.seen.insert(key) {
            self.items.push(d);
        }
    }

    pub fn mark_parse_failed(&mut self, span: Span) {
        let r = span.byte_range();
        self.parse_failed_spans.push((r.start, r.end));
    }

    pub fn is_parse_failed(&self, span: Span) -> bool {
        let r = span.byte_range();
        self.parse_failed_spans
            .iter()
            .any(|(s, e)| *s <= r.start && r.end <= *e)
    }

    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn into_items(self) -> Vec<Diagnostic> {
        self.items
    }

    /// Drops every diagnostic whose primary span strictly contains another
    /// diagnostic's of the same [`DiagFamily`] — the narrower diagnostic
    /// wins.
    pub fn dedup_subsume_narrower(&mut self) {
        let mut keep = vec![true; self.items.len()];
        for i in 0..self.items.len() {
            if !keep[i] {
                continue;
            }
            let ri = self.items[i].primary.byte_range();
            let fi = self.items[i].code.family();
            for j in 0..self.items.len() {
                if i == j || !keep[j] {
                    continue;
                }
                let rj = self.items[j].primary.byte_range();
                let fj = self.items[j].code.family();
                if fi == fj && ri.start <= rj.start && rj.end <= ri.end && ri != rj {
                    keep[i] = false;
                    break;
                }
            }
        }
        let mut i = 0;
        self.items.retain(|_| {
            let k = keep[i];
            i += 1;
            k
        });
    }
}
