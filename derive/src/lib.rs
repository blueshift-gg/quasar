//! Proc macros for the Quasar Solana framework.

use proc_macro::TokenStream;

mod account;
mod accounts;
pub(crate) mod client_macro;
mod declare_program;
mod error_code;
mod event;
mod helpers;
mod instruction;
mod program;
mod seeds;
mod serialize;

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

/// Declare a program ID.
#[proc_macro]
pub fn declare_program(input: TokenStream) -> TokenStream {
    declare_program::declare_program(input)
}

/// Derive typed PDA seed specs from a unit struct.
#[proc_macro_derive(Seeds, attributes(seeds))]
pub fn derive_seeds(input: TokenStream) -> TokenStream {
    seeds::derive_seeds(input)
}
