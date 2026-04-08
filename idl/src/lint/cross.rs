//! Cross-instruction field tracking.
use super::types::{Diagnostic, TypeRegistry};
use crate::parser::ParsedProgram;

pub fn check_cross_instruction(
    _parsed: &ParsedProgram,
    _registry: &TypeRegistry,
) -> Vec<Diagnostic> {
    Vec::new()
}
