mod lower;
mod model;
mod rules;
mod support;

pub(crate) use self::model::*;

pub(super) fn lower_semantics(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    instruction_args: &Option<Vec<crate::accounts::InstructionArg>>,
) -> syn::Result<Vec<FieldSemantics>> {
    self::lower::lower_semantics(fields, instruction_args)
}
