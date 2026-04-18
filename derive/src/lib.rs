//! Proc macros for the Quasar Solana framework.
//!
//! These macros generate the parsing, validation, and dispatch code that makes
//! Quasar programs work. Users typically access them through
//! `quasar_lang::prelude`.
//!
//! | Macro | Purpose |
//! |-------|---------|
//! | [`macro@Accounts`] | Derive account parsing and validation from a struct |
//! | [`macro@account`] | Define an on-chain account type with discriminator |
//! | [`macro@instruction`] | Define an instruction with discriminator and context |
//! | [`macro@program`] | Mark a module as a Quasar program entrypoint |
//! | [`macro@event`] | Define an on-chain event type |
//! | [`macro@error_code`] | Define a program error enum |
//! | [`emit_cpi`] | Emit an event via self-CPI (spoofing-resistant) |

use proc_macro::TokenStream;

mod account;
mod accounts;
mod client_macro;
mod declare_program;
mod error_code;
mod event;
mod helpers;
mod instruction;
mod program;
mod serialize;

/// Derive account parsing and validation for an instruction's accounts struct.
///
/// Each field specifies an account with optional constraints via the
/// `#[account]` attribute.
///
/// # Field attributes
///
/// - `mut` — account must be writable
/// - `signer` — account must be a signer
/// - `address = <expr>` — account address must match the given value
/// - `seeds = [seed, ...]` — PDA derivation seeds (verifies address matches
///   derived PDA)
/// - `init`, `init_if_needed` — create the account via CPI if it doesn't exist
/// - `payer = <field>` — which signer pays for account creation
/// - `space = <expr>` — account data size for creation
///
/// # Generated code
///
/// - A `parse()` method that validates all constraints and returns the parsed
///   struct
/// - A `Bumps` companion struct containing PDA bump seeds
/// - `AccountCount` implementation for dispatch buffer sizing
#[proc_macro_derive(Accounts, attributes(account, instruction))]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    accounts::derive_accounts(input)
}

/// Define an instruction handler with an explicit discriminator.
///
/// # Syntax
///
/// ```ignore
/// #[instruction(discriminator = [1, 0, 0, 0])]
/// pub fn make(ctx: Ctx<Make>, amount: u64) -> Result<(), ProgramError> {
///     // ...
/// }
/// ```
///
/// The discriminator bytes are prepended to the instruction data. All-zero
/// discriminators are rejected at compile time.
#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    instruction::instruction(attr, item)
}

/// Define an on-chain account type with an explicit discriminator.
///
/// Generates a zero-copy companion struct (`Zc*`) with `#[repr(C)]` layout
/// and alignment-1 Pod fields, plus implementations of `Discriminator`,
/// `Space`, `Owner`, `AccountCheck`, and `ZeroCopyDeref`.
///
/// # Syntax
///
/// ```ignore
/// #[account(discriminator = [1])]
/// pub struct Escrow {
///     pub maker: Address,
///     pub amount: u64,
/// }
/// ```
///
/// All-zero discriminators are rejected at compile time. A re-initialization
/// check is generated to prevent overwriting existing account data.
#[proc_macro_attribute]
pub fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    account::account(attr, item)
}

/// Mark a module as a Quasar program, generating the entrypoint and dispatch
/// logic.
///
/// # Syntax
///
/// ```ignore
/// #[program]
/// mod my_program {
///     use super::*;
///
///     #[instruction(discriminator = [0])]
///     pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
///         // ...
///     }
/// }
/// ```
///
/// Generates the `entrypoint!` invocation, instruction dispatch table,
/// and (for non-SBF targets) a client module with instruction builders.
#[proc_macro_attribute]
pub fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    program::program(attr, item)
}

/// Define an on-chain event type with an explicit discriminator.
///
/// Events support dual emission: `emit!()` via `sol_log_data` (~100 CU)
/// or `emit_cpi!()` via self-CPI (~1,000 CU) for spoofing resistance.
///
/// # Syntax
///
/// ```ignore
/// #[event(discriminator = [10])]
/// pub struct TradeExecuted {
///     pub maker: Address,
///     pub amount: u64,
/// }
/// ```
///
/// Generates `Event` trait implementation with `write_data` and `emit` methods.
/// Supported field types: `Address`, `u8`–`u128`, `i8`–`i128`, `bool`.
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    event::event(attr, item)
}

/// Define a program error enum with auto-numbered variants.
///
/// # Syntax
///
/// ```ignore
/// #[error_code]
/// pub enum MyError {
///     #[msg("Insufficient funds")]
///     InsufficientFunds,
///     #[msg("Invalid state")]
///     InvalidState,
/// }
/// ```
///
/// Variants are numbered starting from `6000` (Anchor convention).
/// Each variant includes a human-readable message for logging.
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    error_code::error_code(attr, item)
}

/// Emit an event via self-CPI for spoofing resistance.
///
/// Must be called inside an instruction handler that has access to
/// `self.program` and `self.event_authority`. Costs ~1,000 CU (vs ~100 CU for
/// `emit!()`).
///
/// # Syntax
///
/// ```ignore
/// emit_cpi!(MyEvent { maker: *ctx.accounts.maker.address(), amount });
/// ```
#[proc_macro]
pub fn emit_cpi(input: TokenStream) -> TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    quote::quote! {
        self.program.emit_event(&#input, &self.event_authority, crate::EventAuthority::BUMP)
    }
    .into()
}

/// Derive off-chain instruction data serialization and deserialization.
///
/// Generates wincode `SchemaWrite` and `SchemaRead` impls that
/// serialize/deserialize each field in declaration order. Only compiled for
/// non-SBF targets (off-chain clients). Also generates an `InstructionArg`
/// impl with a ZC companion struct for on-chain zero-copy deserialization.
///
/// # Syntax
///
/// ```ignore
/// #[derive(QuasarSerialize)]
/// #[repr(C)]
/// pub struct TradeParams {
///     pub amount: PodU64,
///     pub side: u8,
/// }
/// ```
///
/// All field types must implement wincode's `SchemaWrite` and `SchemaRead`.
#[proc_macro_derive(QuasarSerialize, attributes(max))]
pub fn derive_quasar_serialize(input: TokenStream) -> TokenStream {
    serialize::derive_quasar_serialize(input)
}

/// Declare an external program for CPI, generating typed helpers from its IDL.
///
/// Reads an IDL JSON file at compile time and generates:
/// - A program account type with `Program` trait implementation
/// - Free functions returning `CpiCall<'a, N, M>` for each instruction
/// - Methods on the program type accepting `&impl AsAccountView` arguments
///
/// # Syntax
///
/// ```ignore
/// quasar::declare_program!(
///     my_program,
///     "target/idl/my_program.json"
/// );
///
/// // Free function style:
/// my_program::make(&program_view, &maker, &escrow, 100u64, 50u64).invoke();
///
/// // Method style (shared program reference):
/// let program: &MyProgram = &ctx.accounts.my_program;
/// program.make(&maker, &escrow, 100u64, 50u64).invoke();
/// ```
///
/// The IDL path resolves relative to the calling crate's `CARGO_MANIFEST_DIR`.
/// Only fixed-size argument types are supported (u8–u128, i8–i128, bool,
/// pubkey).
#[proc_macro]
pub fn declare_program(input: TokenStream) -> TokenStream {
    declare_program::declare_program(input)
}
