//! Emit layer: renders from the resolved accounts model and plan.

mod init;
mod output;
mod parse;

pub(crate) struct EmitCx {
    pub bumps_name: syn::Ident,
}

pub(crate) use {
    output::{emit_accounts_output, AccountsOutput},
    parse::{emit_bump_struct_def, emit_parse_body, emit_seed_methods},
};

pub(crate) fn emit_epilogue(
    semantics: &[super::resolve::FieldSemantics],
) -> syn::Result<proc_macro2::TokenStream> {
    init::emit_epilogue(semantics)
}
