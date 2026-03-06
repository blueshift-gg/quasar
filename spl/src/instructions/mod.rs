mod approve;
mod burn;
mod close_account;
mod initialize_account;
mod initialize_mint;
mod mint_to;
mod revoke;
mod sync_native;
mod transfer;
mod transfer_checked;

pub use initialize_account::initialize_account3;
pub use initialize_mint::initialize_mint2;

use quasar_core::cpi::CpiCall;
use quasar_core::prelude::*;

/// Trait for types that can execute SPL Token CPI calls.
///
/// Implemented by `Program<Token>`, `Program<Token2022>`, and `TokenInterface`.
/// Used as a bound in lifecycle traits (`InitToken`, `InitMint`, `TokenClose`)
/// to ensure only actual token programs are accepted — not arbitrary accounts.
pub trait TokenCpi: AsAccountView {
    /// Transfer tokens between accounts.
    #[inline(always)]
    fn transfer<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        transfer::transfer(
            self.to_account_view(),
            from.to_account_view(),
            to.to_account_view(),
            authority.to_account_view(),
            amount.into(),
        )
    }

    /// Transfer tokens with decimal verification.
    #[inline(always)]
    fn transfer_checked<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
        decimals: u8,
    ) -> CpiCall<'a, 4, 10> {
        transfer_checked::transfer_checked(
            self.to_account_view(),
            from.to_account_view(),
            mint.to_account_view(),
            to.to_account_view(),
            authority.to_account_view(),
            amount.into(),
            decimals,
        )
    }

    /// Mint tokens to an account.
    #[inline(always)]
    fn mint_to<'a>(
        &'a self,
        mint: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        mint_to::mint_to(
            self.to_account_view(),
            mint.to_account_view(),
            to.to_account_view(),
            authority.to_account_view(),
            amount.into(),
        )
    }

    /// Burn tokens from an account.
    #[inline(always)]
    fn burn<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        burn::burn(
            self.to_account_view(),
            from.to_account_view(),
            mint.to_account_view(),
            authority.to_account_view(),
            amount.into(),
        )
    }

    /// Approve a delegate to transfer tokens.
    #[inline(always)]
    fn approve<'a>(
        &'a self,
        source: &'a impl AsAccountView,
        delegate: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        approve::approve(
            self.to_account_view(),
            source.to_account_view(),
            delegate.to_account_view(),
            authority.to_account_view(),
            amount.into(),
        )
    }

    /// Close a token account and reclaim its lamports.
    #[inline(always)]
    fn close_account<'a>(
        &'a self,
        account: &'a impl AsAccountView,
        destination: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
    ) -> CpiCall<'a, 3, 1> {
        close_account::close_account(
            self.to_account_view(),
            account.to_account_view(),
            destination.to_account_view(),
            authority.to_account_view(),
        )
    }

    /// Revoke a delegate's authority.
    #[inline(always)]
    fn revoke<'a>(
        &'a self,
        source: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
    ) -> CpiCall<'a, 2, 1> {
        revoke::revoke(
            self.to_account_view(),
            source.to_account_view(),
            authority.to_account_view(),
        )
    }

    /// Sync the lamport balance of a native SOL token account.
    #[inline(always)]
    fn sync_native<'a>(&'a self, token_account: &'a impl AsAccountView) -> CpiCall<'a, 1, 1> {
        sync_native::sync_native(self.to_account_view(), token_account.to_account_view())
    }

    /// Initialize a token account (InitializeAccount3 — opcode 18).
    ///
    /// Unlike InitializeAccount/InitializeAccount2, this variant does not
    /// require the Rent sysvar account, saving one account in the CPI.
    /// The account must already be allocated with the correct size (165 bytes).
    #[inline(always)]
    fn initialize_account3<'a>(
        &'a self,
        account: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        owner: &Address,
    ) -> CpiCall<'a, 2, 33> {
        initialize_account::initialize_account3(
            self.to_account_view(),
            account.to_account_view(),
            mint.to_account_view(),
            owner,
        )
    }

    /// Initialize a mint (InitializeMint2 — opcode 20).
    ///
    /// Unlike InitializeMint, this variant does not require the Rent
    /// sysvar account, saving one account in the CPI. The account must
    /// already be allocated with the correct size (82 bytes).
    #[inline(always)]
    fn initialize_mint2<'a>(
        &'a self,
        mint: &'a impl AsAccountView,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> CpiCall<'a, 1, 67> {
        initialize_mint::initialize_mint2(
            self.to_account_view(),
            mint.to_account_view(),
            decimals,
            mint_authority,
            freeze_authority,
        )
    }
}
