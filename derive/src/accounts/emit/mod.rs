pub(crate) mod entry;
mod output;
pub(super) mod parse;
mod typed_emit;

pub(crate) use output::{emit_accounts_output, AccountsOutput};

pub(crate) struct EmitCx {
    pub bumps_name: syn::Ident,
}
