//! Capability traits for SPL token behavior modules.
//!
//! Each trait represents a single capability (check, init contribution, close,
//! sweep). Layout-backed blanket impls wire types with the correct schema to
//! the appropriate validation logic.
//!
//! All trait method impls use `#[inline(always)]` — critical for sBPF CU
//! parity. Missing this annotation costs 100-200 CU per non-inlined call.

use {
    super::ctx::{AssociatedTokenCheckCtx, MintCheckCtx, TokenCheckCtx},
    quasar_lang::{account_layout::AccountLayout, prelude::*},
};

// ---------------------------------------------------------------------------
// Check capabilities
// ---------------------------------------------------------------------------

/// Capability: validate a token account (mint, authority, program).
pub(crate) trait TokenCheck: AsAccountView {
    fn check_token_view(view: &AccountView, ctx: TokenCheckCtx<'_>) -> Result<(), ProgramError>;
}

/// Capability: validate a mint account (authority, decimals, freeze authority).
pub(crate) trait MintCheck: AsAccountView {
    fn check_mint_view(view: &AccountView, ctx: MintCheckCtx<'_>) -> Result<(), ProgramError>;
}

/// Capability: validate an associated token account.
pub(crate) trait AssociatedTokenCheck: AsAccountView {
    fn check_associated_token_view(
        view: &AccountView,
        ctx: AssociatedTokenCheckCtx<'_>,
    ) -> Result<(), ProgramError>;
}

// ---------------------------------------------------------------------------
// Layout-backed blanket impls — check capabilities
// ---------------------------------------------------------------------------

impl<T> TokenCheck for T
where
    T: AsAccountView + AccountLayout<Schema = crate::token::TokenData>,
{
    #[inline(always)]
    fn check_token_view(view: &AccountView, ctx: TokenCheckCtx<'_>) -> Result<(), ProgramError> {
        crate::validate::validate_token_account(
            view,
            ctx.mint.address(),
            ctx.authority.address(),
            ctx.token_program.map(|tp| tp.address()),
        )
    }
}

impl<T> MintCheck for T
where
    T: AsAccountView + AccountLayout<Schema = crate::token::MintData>,
{
    #[inline(always)]
    fn check_mint_view(view: &AccountView, ctx: MintCheckCtx<'_>) -> Result<(), ProgramError> {
        use super::ctx::FreezeAuthorityCheck;
        let freeze = match ctx.freeze_authority {
            FreezeAuthorityCheck::Skip => crate::validate::FreezeCheck::Skip,
            FreezeAuthorityCheck::AssertNone => crate::validate::FreezeCheck::AssertNone,
            FreezeAuthorityCheck::AssertEquals(fa) => {
                crate::validate::FreezeCheck::AssertEquals(fa.address())
            }
        };
        crate::validate::validate_mint_with_freeze(
            view,
            ctx.authority.address(),
            ctx.decimals,
            freeze,
            ctx.token_program.map(|tp| tp.address()),
        )
    }
}

/// Associated token validation: token-layout validation + address derivation.
impl<T> AssociatedTokenCheck for T
where
    T: AsAccountView + AccountLayout<Schema = crate::token::TokenData>,
{
    #[inline(always)]
    fn check_associated_token_view(
        view: &AccountView,
        ctx: AssociatedTokenCheckCtx<'_>,
    ) -> Result<(), ProgramError> {
        // When token_program is None (concrete Account<Token>), use the
        // account's on-chain owner for derivation — AccountLoad validated it.
        let tp = ctx
            .token_program
            .map(|p| p.address())
            .unwrap_or_else(|| view.owner());
        crate::validate::validate_ata(view, ctx.authority.address(), ctx.mint.address(), tp)
    }
}
