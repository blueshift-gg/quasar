//! Proc macros for the Quasar Solana framework.

use proc_macro::TokenStream;

mod account;
mod accounts;
pub(crate) mod client_macro;
#[cfg(feature = "declare-program")]
mod declare_program;
mod error_code;
mod event;
mod helpers;
mod instruction;
mod program;
mod seed_param;
mod seeds;
mod serialize;

#[cfg(test)]
mod plan_snapshots;
#[cfg(test)]
mod snapshot_tests;

/// Derive account parsing and validation from a struct.
#[proc_macro_derive(Accounts, attributes(account, instruction))]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    accounts::derive_accounts(input)
}

/// Define an instruction with discriminator and context.
#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    instruction::instruction(attr, item)
}

/// Define an on-chain account type with discriminator.
#[proc_macro_attribute]
pub fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    account::account(attr, item)
}

/// Mark a module as a Quasar program entrypoint.
#[proc_macro_attribute]
pub fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    program::program(attr, item)
}

/// Define an on-chain event type.
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    event::event(attr, item)
}

/// Define a program error enum.
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    error_code::error_code(attr, item)
}

/// Emit an event via self-CPI (spoofing-resistant).
#[proc_macro]
pub fn emit_cpi(input: TokenStream) -> TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    quote::quote! {
        self.program.emit_event(&#input, &self.event_authority, crate::EventAuthority::BUMP)
    }
    .into()
}

/// Derive QuasarSerialize for instruction argument types.
#[proc_macro_derive(QuasarSerialize, attributes(max))]
pub fn derive_quasar_serialize(input: TokenStream) -> TokenStream {
    serialize::derive_quasar_serialize(input)
}

/// Generate typed Rust bindings from an external program's IDL JSON.
///
/// Gated behind the `declare-program` feature so that the serde/IDL-schema
/// stack it needs is not pulled into ordinary program builds.
#[cfg(feature = "declare-program")]
#[proc_macro]
pub fn declare_program(input: TokenStream) -> TokenStream {
    declare_program::declare_program(input)
}

/// Feature-off stub for [`declare_program!`]: emits a spanned error telling the
/// caller to enable the `declare-program` feature.
#[cfg(not(feature = "declare-program"))]
#[proc_macro]
pub fn declare_program(_input: TokenStream) -> TokenStream {
    syn::Error::new(
        proc_macro2::Span::call_site(),
        "`declare_program!` requires the `declare-program` feature; enable it on quasar-lang \
         (e.g. `quasar-lang = { version = \"...\", features = [\"declare-program\"] }`)",
    )
    .to_compile_error()
    .into()
}

/// Derive typed PDA seed specs from a unit struct.
#[proc_macro_derive(Seeds, attributes(seeds))]
pub fn derive_seeds(input: TokenStream) -> TokenStream {
    seeds::derive_seeds(input)
}
