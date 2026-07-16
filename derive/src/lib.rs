//! Proc macros for the Quasar Solana framework.

#![warn(missing_docs)]

use proc_macro::TokenStream;

#[macro_use]
mod ice;
mod account;
mod accounts;
pub(crate) mod client_macro;
mod ctx;
#[cfg(feature = "declare-program")]
mod declare_program;
mod error_code;
mod event;
mod helpers;
mod idl;
mod instruction;
mod krate;
mod program;
mod schema_ir;
mod seed_param;
mod seeds;
mod serialize;

#[cfg(test)]
mod plan_snapshots;
#[cfg(test)]
mod snapshot_tests;

/// Derive account parsing and validation from a struct.
///
/// Field wrapper types (`Account<T>`, `Signer`, `Sysvar<Rent>`, …) are matched
/// **syntactically on their last path segment**. Proc macros cannot resolve
/// type aliases, so a field written through an alias
/// (`type Vault = Account<'a, VaultState>; vault: Vault`) is not recognized as
/// its underlying wrapper — spell the wrapper directly on the field.
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
///
/// A module named `vault` emits the executable marker `VaultProgram`, leaving
/// `Vault` available for application state or account-context types.
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
///
/// Auto-assigned variants start at error code **6000** (the Anchor-compatible
/// offset); a variant with an explicit integer discriminant keeps that literal
/// and re-bases the auto-increment from there. Two variants that resolve to the
/// same code are a hard, spanned error.
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    error_code::error_code(attr, item)
}

/// Emit an event via self-CPI (spoofing-resistant, ~1,000 CU): the program ID
/// appears in the transaction's inner-instruction trace, so the event cannot be
/// forged by another program. Use `emit!` instead (~100 CU) when a spoofable
/// log-based event is acceptable.
///
/// Expands to the typed `quasar_lang::event::EventCpi::emit` call on `self`;
/// `#[derive(Accounts)]` supplies the `EventCpi` impl from the struct's
/// `event_authority` + `Program<T>` fields (the program field is found by type,
/// so its name is not hard-coded here).
#[proc_macro]
pub fn emit_cpi(input: TokenStream) -> TokenStream {
    emit_cpi_inner(input.into()).into()
}

fn emit_cpi_inner(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    quote::quote! {
        #krate::event::EventCpi::emit(self, &#input)
    }
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
///
/// A constant byte prefix is optional:
///
/// ```ignore
/// #[derive(Seeds)]
/// #[seeds(b"escrow", maker: Address, seed: u64)]
/// pub struct EscrowPda;
///
/// #[derive(Seeds)]
/// #[seeds(signer: Address)]
/// pub struct VaultPda;
/// ```
#[proc_macro_derive(Seeds, attributes(seeds))]
pub fn derive_seeds(input: TokenStream) -> TokenStream {
    seeds::derive_seeds(input)
}
