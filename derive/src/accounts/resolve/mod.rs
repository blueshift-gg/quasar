mod lower;
mod model;
pub(crate) mod rules;

pub(crate) use model::*;

pub(crate) fn lower_semantics(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    instruction_args: &Option<Vec<crate::accounts::InstructionArg>>,
) -> syn::Result<Vec<FieldSemantics>> {
    lower::lower_semantics(fields, instruction_args)
}
