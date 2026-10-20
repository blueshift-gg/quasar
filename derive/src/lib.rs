//! Quasar's macros. They arrange calls into `quasar-lang` and decide nothing
//! a reader cannot see in their output.

mod account;
mod accounts;
mod data;
mod program;
mod property;
mod vouch;

use proc_macro::TokenStream;
use syn::{DeriveInput, ItemEnum, ItemFn, ItemMod, ItemStruct, Path, parse_macro_input};

/// Loads each field from the input in declaration order. A field's
/// `#[account(..)]` checks run, in the order written, as soon as every field
/// they name has loaded.
///
/// `#[data(T)]` makes the instruction data, `data`, readable by the
/// constraints; the handler then takes `Ctx<Self, T>`.
#[proc_macro_derive(Accounts, attributes(account, data))]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    accounts::expand(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Makes a struct a kind of account: stored zero-copy after its
/// discriminator, owned by this program unless `owner = ..` says otherwise.
///
/// The kinds a program owns have distinct discriminators of one width, so
/// no account of one kind loads as another.
#[proc_macro_attribute]
pub fn account(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = proc_macro2::TokenStream::from(args);
    let input = parse_macro_input!(input as ItemStruct);
    account::expand_account(args, input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// `T::seeds(..)` for a PDA that holds no data.
#[proc_macro_derive(Seeds, attributes(seeds))]
pub fn derive_seeds(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    account::expand_seeds(input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Makes a struct an event, logged by `emit!`.
#[proc_macro_attribute]
pub fn event(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = proc_macro2::TokenStream::from(args);
    let input = parse_macro_input!(input as ItemStruct);
    account::expand_event(args, input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Makes an enum the program's errors.
#[proc_macro_attribute]
pub fn error_code(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemEnum);
    account::expand_error_code(input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Makes a struct an instruction's data: its fields in order, each of one
/// size, in Borsh's encoding.
#[proc_macro_derive(InstructionData)]
pub fn derive_instruction_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    data::expand(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// `#[instruction(0)]` on a handler, `fn name(ctx: Ctx<Accounts, Data>)`:
/// its discriminator, and a module of its name to build or call it.
#[proc_macro_attribute]
pub fn instruction(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = proc_macro2::TokenStream::from(args);
    let input = parse_macro_input!(input as ItemFn);
    program::instruction(args, input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// `#[program] mod name { .. }`: the entrypoint, dispatching on the
/// discriminator of each `#[instruction(n)]` handler in the module, the
/// program's type for callers, and its IDL.
#[proc_macro_attribute]
pub fn program(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = proc_macro2::TokenStream::from(args);
    let input = parse_macro_input!(input as ItemMod);
    program::program(args, input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Turns a predicate over the accounts before and after an instruction into a
/// Kani harness for that instruction. Exists only in Kani builds.
#[proc_macro_attribute]
pub fn property(args: TokenStream, input: TokenStream) -> TokenStream {
    let handler = parse_macro_input!(args as Path);
    let input = parse_macro_input!(input as ItemFn);
    property::expand(handler, input).unwrap_or_else(syn::Error::into_compile_error).into()
}
