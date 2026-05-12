//! Context structs for capability trait methods.
//!
//! These provide the input surface for internal capability traits.

use quasar_lang::prelude::AccountView;

/// Context for token account validation.
///
/// `token_program` is optional: for concrete `Account<Token>` types,
/// `AccountLoad::check` already validated the owner. Only
/// `InterfaceAccount<Token>` needs the runtime program check.
pub(crate) struct TokenCheckCtx<'a> {
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: Option<&'a AccountView>,
}

/// How freeze_authority should be validated.
///
/// Three distinct semantics:
/// - `Skip`: user omitted freeze_authority → do not check at all.
/// - `AssertNone`: user wrote `freeze_authority = None` → assert no freeze
///   authority.
/// - `AssertEquals`: user wrote `freeze_authority = Some(field)` → assert
///   matches.
pub(crate) enum FreezeAuthorityCheck<'a> {
    /// Omitted by user — skip check entirely.
    Skip,
    /// Assert the mint has no freeze authority.
    AssertNone,
    /// Assert the mint's freeze authority matches this address.
    AssertEquals(&'a AccountView),
}

/// Context for mint account validation.
///
/// `token_program` is optional: concrete types already have owner validated.
/// `decimals` is optional: defaults to "don't check" when None.
pub(crate) struct MintCheckCtx<'a> {
    pub decimals: Option<u8>,
    pub authority: &'a AccountView,
    pub freeze_authority: FreezeAuthorityCheck<'a>,
    pub token_program: Option<&'a AccountView>,
}

/// Context for associated token address + token data validation.
///
/// `token_program` is optional: for concrete `Account<Token>`, the program
/// is known from the owner. When None, uses the account's on-chain owner
/// for ATA address derivation (safe — AccountLoad validated the owner).
pub(crate) struct AssociatedTokenCheckCtx<'a> {
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: Option<&'a AccountView>,
}
