//! Core types for the account relationship linter.

use std::collections::HashMap;

/// Lint rule identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LintRule {
    L001, // Disconnected account (island)
    L002, // Disconnected subgraph
    L003, // Missing has_one
    L004, // Unvalidated token mint
    L005, // Unvalidated token authority
    L006, // Writable without authority
    L007, // Unchecked account without validation
    L009, // Cross-instruction unverified field
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
        }
    }

    pub fn default_severity(&self) -> Severity {
        match self {
            Self::L001 | Self::L003 | Self::L004 => Severity::Error,
            Self::L002 | Self::L005 | Self::L006 | Self::L007 | Self::L009 => Severity::Warning,
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
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
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
            self.address_fields.insert(type_name.to_string(), addr_fields);
        }
    }

    pub fn get_address_fields(&self, type_name: &str) -> Vec<String> {
        match type_name {
            "TokenAccount" | "Token" => vec!["mint".to_string(), "owner".to_string()],
            _ => self.address_fields.get(type_name).cloned().unwrap_or_default(),
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
