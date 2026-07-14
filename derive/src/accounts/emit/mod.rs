pub(crate) mod entry;
pub(crate) mod ix_args;
mod output;
pub(super) mod parse;
pub(crate) mod typed_emit;

pub(crate) use output::{emit_accounts_output, AccountsOutput};

pub(crate) struct EmitCx {
    pub bumps_name: syn::Ident,
}
