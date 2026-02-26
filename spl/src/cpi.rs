use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

/// Trait for types that can execute SPL Token CPI calls.
///
/// Implemented by [`TokenProgram`], [`Token2022Program`], and [`TokenInterface`].
/// Used as a bound in lifecycle traits ([`InitToken`], [`InitMint`], [`TokenClose`])
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
        let from = from.to_account_view();
        let to = to.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 3;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(from.address()),
                InstructionAccount::writable(to.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [from, to, authority],
            data,
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
        let from = from.to_account_view();
        let mint = mint.to_account_view();
        let to = to.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 10];
        data[0] = 12;
        data[1..9].copy_from_slice(&amount.to_le_bytes());
        data[9] = decimals;

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(from.address()),
                InstructionAccount::readonly(mint.address()),
                InstructionAccount::writable(to.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [from, mint, to, authority],
            data,
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
        let mint = mint.to_account_view();
        let to = to.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 7;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(mint.address()),
                InstructionAccount::writable(to.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [mint, to, authority],
            data,
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
        let from = from.to_account_view();
        let mint = mint.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 8;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(from.address()),
                InstructionAccount::writable(mint.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [from, mint, authority],
            data,
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
        let source = source.to_account_view();
        let delegate = delegate.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 4;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(source.address()),
                InstructionAccount::readonly(delegate.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [source, delegate, authority],
            data,
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
        let account = account.to_account_view();
        let destination = destination.to_account_view();
        let authority = authority.to_account_view();

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(account.address()),
                InstructionAccount::writable(destination.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [account, destination, authority],
            [9],
        )
    }

    /// Revoke a delegate's authority.
    #[inline(always)]
    fn revoke<'a>(
        &'a self,
        source: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
    ) -> CpiCall<'a, 2, 1> {
        let source = source.to_account_view();
        let authority = authority.to_account_view();

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(source.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [source, authority],
            [5],
        )
    }

    /// Sync the lamport balance of a native SOL token account.
    #[inline(always)]
    fn sync_native<'a>(&'a self, token_account: &'a impl AsAccountView) -> CpiCall<'a, 1, 1> {
        let token_account = token_account.to_account_view();

        CpiCall::new(
            self.address(),
            [InstructionAccount::writable(token_account.address())],
            [token_account],
            [17],
        )
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
        let account = account.to_account_view();
        let mint = mint.to_account_view();

        let mut data = [0u8; 33];
        data[0] = 18;
        data[1..33].copy_from_slice(owner.as_ref());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(account.address()),
                InstructionAccount::readonly(mint.address()),
            ],
            [account, mint],
            data,
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
        let mint = mint.to_account_view();

        let mut data = [0u8; 67];
        data[0] = 20;
        data[1] = decimals;
        data[2..34].copy_from_slice(mint_authority.as_ref());
        match freeze_authority {
            Some(fa) => {
                data[34] = 1;
                data[35..67].copy_from_slice(fa.as_ref());
            }
            None => {
                // data[34] already 0 (COption::None), rest stays zero
            }
        }

        CpiCall::new(
            self.address(),
            [InstructionAccount::writable(mint.address())],
            [mint],
            data,
        )
    }
}
