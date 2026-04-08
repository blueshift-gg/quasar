//! Account relationship linter for Quasar programs.

pub mod cross;
pub mod fix;
pub mod graph;
pub mod output;
pub mod rules;
pub mod types;
pub mod viz;

use crate::parser::ParsedProgram;
pub use types::{Diagnostic, GraphFormat, LintConfig, LintReport, LintRule, Severity};

/// Run the linter on a parsed program.
pub fn run_lint(parsed: &ParsedProgram, _config: &LintConfig) -> LintReport {
    let type_registry = types::TypeRegistry::from_parsed(parsed);
    let mut diagnostics = Vec::new();
    let mut instruction_scores = Vec::new();

    for accounts_struct in &parsed.accounts_structs {
        let g = graph::AccountGraph::build(accounts_struct, &type_registry);

        let mut struct_diags = Vec::new();
        rules::run_all(&g, &mut struct_diags);

        instruction_scores.push(types::InstructionScore {
            program_name: parsed.program_name.clone(),
            instruction_name: find_instruction_name(parsed, &accounts_struct.name),
            accounts_struct: accounts_struct.name.clone(),
            total_edges: g.expected_edge_count(),
            constrained_edges: g.constrained_edge_count(),
        });

        diagnostics.extend(struct_diags);
    }

    let cross_diags = cross::check_cross_instruction(parsed, &type_registry);
    diagnostics.extend(cross_diags);

    LintReport {
        diagnostics,
        instruction_scores,
    }
}

fn find_instruction_name(parsed: &ParsedProgram, accounts_type: &str) -> String {
    parsed
        .instructions
        .iter()
        .find(|i| i.accounts_type_name == accounts_type)
        .map(|i| i.name.clone())
        .unwrap_or_else(|| accounts_type.to_string())
}
