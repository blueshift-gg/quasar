pub(crate) mod ops;
mod output;
pub(crate) mod parse;

pub(crate) use output::{emit_accounts_output, AccountsOutput};

pub(crate) struct EmitCx {
    pub bumps_name: syn::Ident,
}

pub(crate) fn emit_parse_body(
    semantics: &[super::resolve::FieldSemantics],
    cx: &EmitCx,
) -> syn::Result<proc_macro2::TokenStream> {
    parse::emit_parse_body(semantics, cx)
}

pub(crate) fn emit_bump_struct_def(
    semantics: &[super::resolve::FieldSemantics],
    cx: &EmitCx,
) -> proc_macro2::TokenStream {
    parse::emit_bump_struct_def(semantics, cx)
}

pub(crate) fn emit_epilogue(
    semantics: &[super::resolve::FieldSemantics],
) -> syn::Result<proc_macro2::TokenStream> {
    let op_ctx = ops::OpEmitCtx {
        field_names: semantics.iter().map(|s| s.core.ident.to_string()).collect(),
    };
    parse::emit_epilogue(semantics, &op_ctx)
}

pub(crate) fn emit_has_epilogue(
    semantics: &[super::resolve::FieldSemantics],
) -> proc_macro2::TokenStream {
    parse::emit_has_epilogue(semantics)
}
