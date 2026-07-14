pub(crate) mod describe;
#[cfg(test)]
pub(crate) mod dump;
mod lower;
mod model;
pub(crate) mod planner;
pub(crate) mod reserved;
pub(crate) mod rules;
pub(crate) mod specs;
pub(crate) mod wrapper;

pub(crate) use model::*;

pub(crate) fn lower_semantics(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    instruction_args: &[super::InstructionArg],
) -> syn::Result<Vec<FieldSemantics>> {
    lower::lower_semantics(fields, instruction_args)
}
