//! Core types for the account relationship linter.

use std::collections::HashMap;

/// Lint rule identifier.
///
/// L001–L009 are the original account-relationship rules — single-build
/// graph integrity checks. L010+ are the upgrade-safety extension
/// (preflight + cross-build diff). The two families share the same
/// `Diagnostic` / `LintReport` plumbing; the diff family runs through
/// the parallel `comparative` entry point with two `ParsedProgram`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LintRule {
    // --- L001–L009: account-relationship rules (single build) -----------
    L001, // Disconnected account (island)
    L002, // Disconnected subgraph
    L003, // Missing has_one
    L004, // Unvalidated token mint
    L005, // Unvalidated token authority
    L006, // Writable without authority
    L007, // Unchecked account without validation
    L009, // Cross-instruction unverified field

    // --- L010+: preflight + cross-build upgrade-safety rules -------------
    //
    // Preflight (single-build readiness):
    L010, // Account missing `version: u8` prefix field
    L011, // Account missing `_reserved: [u8; N]` trailing padding
    L012, // Account name collides with a well-known Solana type

    // Cross-build diff (compares last release's surface to the candidate):
    L013, // Account field reorder
    L014, // Account field retype
    L015, // Account field removed
    L016, // Account field inserted mid-list
    L017, // Account field appended (needs realloc)
    L018, // Account discriminator change
    L019, // Instruction removed
    L020, // Instruction argument signature change
    L021, // Instruction account-list change
    L022, // Instruction signer/writable flag flip
    L023, // PDA seed change (field-path precision limited; see rule doc)
    L024, // Instruction discriminator change
    L025, // Account struct removed
    L026, // Event discriminator change
}

impl LintRule {
    pub fn code(&self) -> &'static str {
        match self {
            Self::L001 => "L001",
            Self::L002 => "L002",
            Self::L003 => "L003",
            Self::L004 => "L004",
            Self::L005 => "L005",
            Self::L006 => "L006",
            Self::L007 => "L007",
            Self::L009 => "L009",
            Self::L010 => "L010",
            Self::L011 => "L011",
            Self::L012 => "L012",
            Self::L013 => "L013",
            Self::L014 => "L014",
            Self::L015 => "L015",
            Self::L016 => "L016",
            Self::L017 => "L017",
            Self::L018 => "L018",
            Self::L019 => "L019",
            Self::L020 => "L020",
            Self::L021 => "L021",
            Self::L022 => "L022",
            Self::L023 => "L023",
            Self::L024 => "L024",
            Self::L025 => "L025",
            Self::L026 => "L026",
        }
    }

    pub fn default_severity(&self) -> Severity {
        match self {
            // Existing rules
            Self::L001 | Self::L003 | Self::L004 => Severity::Error,
            Self::L002 | Self::L005 | Self::L006 | Self::L007 | Self::L009 => Severity::Warning,

            // Preflight: missing-version / missing-padding are deploy-time
            // landmines for any future schema change, but the program
            // itself is valid today. Warning, not Error.
            Self::L010 | Self::L011 => Severity::Warning,
            // Name collision is purely an ergonomics/tooling hazard.
            Self::L012 => Severity::Warning,

            // Cross-build breakage (corrupts on-chain state, breaks
            // existing callers, orphans PDAs). All Error.
            Self::L013
            | Self::L014
            | Self::L015
            | Self::L016
            | Self::L018
            | Self::L019
            | Self::L020
            | Self::L021
            | Self::L022
            | Self::L023
            | Self::L024
            | Self::L025
            | Self::L026 => Severity::Error,
            // Append needs a realloc on existing accounts but isn't
            // automatically corrupting — Warning so devs see it without
            // failing a build that's intentionally adding migration code.
            Self::L017 => Severity::Warning,
        }
    }

    pub fn suppression_attr(&self) -> &'static str {
        match self {
            Self::L001 => "quasar::unconstrained",
            Self::L002 => "quasar::disconnected_graph",
            Self::L003 => "quasar::missing_has_one",
            Self::L004 => "quasar::unvalidated_mint",
            Self::L005 => "quasar::unvalidated_authority",
            Self::L006 => "quasar::writable_no_authority",
            Self::L007 => "quasar::unchecked_account",
            Self::L009 => "quasar::cross_instruction",
            Self::L010 => "quasar::missing_version_field",
            Self::L011 => "quasar::missing_reserved_padding",
            Self::L012 => "quasar::reserved_name_collision",
            Self::L013 => "quasar::field_reorder",
            Self::L014 => "quasar::field_retype",
            Self::L015 => "quasar::field_removed",
            Self::L016 => "quasar::field_insert_middle",
            Self::L017 => "quasar::field_append",
            Self::L018 => "quasar::account_discriminator_change",
            Self::L019 => "quasar::instruction_removed",
            Self::L020 => "quasar::instruction_arg_change",
            Self::L021 => "quasar::instruction_account_list_change",
            Self::L022 => "quasar::instruction_signer_writable_flip",
            Self::L023 => "quasar::pda_seed_change",
            Self::L024 => "quasar::instruction_discriminator_change",
            Self::L025 => "quasar::account_removed",
            Self::L026 => "quasar::event_discriminator_change",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A single lint diagnostic produced by a rule.
#[derive(Debug)]
pub struct Diagnostic {
    pub rule: LintRule,
    pub severity: Severity,
    pub accounts_struct: String,
    pub field: Option<String>,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Full lint results for the program.
#[derive(Debug)]
pub struct LintReport {
    pub diagnostics: Vec<Diagnostic>,
    pub instruction_scores: Vec<InstructionScore>,
}

#[derive(Debug)]
pub struct InstructionScore {
    pub program_name: String,
    pub instruction_name: String,
    pub accounts_struct: String,
    pub total_edges: usize,
    pub constrained_edges: usize,
}

impl LintReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// Options controlling lint behavior.
#[derive(Debug, Default)]
pub struct LintConfig {
    pub fix: bool,
    pub graph: Option<GraphFormat>,
}

#[derive(Debug, Clone)]
pub enum GraphFormat {
    Ascii,
    Mermaid,
    Dot,
    Json,
}

/// Registry of account types -> their Address fields.
/// Built from #[account(discriminator)] state structs.
#[derive(Debug, Default)]
pub struct TypeRegistry {
    pub address_fields: HashMap<String, Vec<String>>,
}

impl TypeRegistry {
    pub fn from_parsed(parsed: &crate::parser::ParsedProgram) -> Self {
        let mut registry = Self::default();
        for state in &parsed.state_accounts {
            registry.register(&state.name, &state.fields);
        }
        registry
    }

    pub fn register(&mut self, type_name: &str, fields: &[(String, syn::Type)]) {
        let addr_fields: Vec<String> = fields
            .iter()
            .filter(|(_, ty)| is_address_type(ty))
            .map(|(name, _)| name.clone())
            .collect();
        if !addr_fields.is_empty() {
            self.address_fields
                .insert(type_name.to_string(), addr_fields);
        }
    }

    pub fn get_address_fields(&self, type_name: &str) -> Vec<String> {
        match type_name {
            "TokenAccount" | "Token" => vec!["mint".to_string(), "owner".to_string()],
            _ => self
                .address_fields
                .get(type_name)
                .cloned()
                .unwrap_or_default(),
        }
    }
}

fn is_address_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return seg.ident == "Address" || seg.ident == "Pubkey";
        }
    }
    false
}
