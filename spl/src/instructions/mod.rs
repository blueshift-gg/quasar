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

use quasar_lang::{cpi::CpiCall, prelude::*};
pub use {
    close_account::close_account, initialize_account::initialize_account3,
    initialize_mint::initialize_mint2, transfer_checked::transfer_checked,
};

/// Trait for types that can execute SPL Token CPI calls.
///
/// Implemented by `Program<Token>`, `Program<Token2022>`, and `TokenInterface`.
/// Ensures only actual token programs are accepted — not arbitrary accounts.
pub trait TokenCpi: AsAccountView {
    /// Transfer tokens between accounts.
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Source token account
    ///   1. `[WRITE]` Destination token account
    ///   2. `[SIGNER]` Source account owner / delegate
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

    /// Transfer tokens with mint decimal verification.
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Source token account
    ///   1. `[]`      Token mint
    ///   2. `[WRITE]` Destination token account
    ///   3. `[SIGNER]` Source account owner / delegate
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

    /// Mint new tokens to an account.
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Mint account
    ///   1. `[WRITE]` Destination token account
    ///   2. `[SIGNER]` Mint authority
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
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Source token account
    ///   1. `[WRITE]` Token mint
    ///   2. `[SIGNER]` Source account owner / delegate
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
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Source token account
    ///   1. `[]`      Delegate
    ///   2. `[SIGNER]` Source account owner
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
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Account to close
    ///   1. `[WRITE]` Destination for remaining lamports
    ///   2. `[SIGNER]` Account owner / close authority
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
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Source token account
    ///   1. `[SIGNER]` Source account owner
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
    ///
    /// ### Accounts:
    ///   0. `[WRITE]` Native SOL token account
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

// ---------------------------------------------------------------------------
// Kani proof harnesses for SPL Token instruction data layout
// ---------------------------------------------------------------------------
//
// Each harness replicates the unsafe `MaybeUninit` + pointer-write pattern used
// by the corresponding instruction builder and asserts:
//   1. The discriminator byte is correct.
//   2. Payload fields are written at the expected offsets.
//   3. All bytes of the buffer are initialised before `assume_init`.
//
// Because the harnesses use `kani::any()` for payload values, Kani explores
// *every* possible input, giving us a full proof — not just example-based
// tests.
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {

    // -- transfer (disc=3, 9-byte buffer) ----------------------------------

    /// Prove that the `transfer` instruction data layout is correct for all
    /// possible `amount` values.
    #[kani::proof]
    fn transfer_instruction_layout() {
        let amount: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 3u8);
            (ptr.add(1) as *mut u64).write_unaligned(amount);
            buf.assume_init()
        };

        // Discriminator at offset 0
        assert!(data[0] == 3u8);
        // Amount at offset 1..9 (little-endian)
        let amount_bytes = amount.to_le_bytes();
        assert!(data[1] == amount_bytes[0]);
        assert!(data[2] == amount_bytes[1]);
        assert!(data[3] == amount_bytes[2]);
        assert!(data[4] == amount_bytes[3]);
        assert!(data[5] == amount_bytes[4]);
        assert!(data[6] == amount_bytes[5]);
        assert!(data[7] == amount_bytes[6]);
        assert!(data[8] == amount_bytes[7]);
    }

    // -- mint_to (disc=7, 9-byte buffer) -----------------------------------

    /// Prove that the `mint_to` instruction data layout is correct for all
    /// possible `amount` values.
    #[kani::proof]
    fn mint_to_instruction_layout() {
        let amount: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 7u8);
            (ptr.add(1) as *mut u64).write_unaligned(amount);
            buf.assume_init()
        };

        assert!(data[0] == 7u8);
        let amount_bytes = amount.to_le_bytes();
        assert!(data[1] == amount_bytes[0]);
        assert!(data[2] == amount_bytes[1]);
        assert!(data[3] == amount_bytes[2]);
        assert!(data[4] == amount_bytes[3]);
        assert!(data[5] == amount_bytes[4]);
        assert!(data[6] == amount_bytes[5]);
        assert!(data[7] == amount_bytes[6]);
        assert!(data[8] == amount_bytes[7]);
    }

    // -- burn (disc=8, 9-byte buffer) --------------------------------------

    /// Prove that the `burn` instruction data layout is correct for all
    /// possible `amount` values.
    #[kani::proof]
    fn burn_instruction_layout() {
        let amount: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 8u8);
            (ptr.add(1) as *mut u64).write_unaligned(amount);
            buf.assume_init()
        };

        assert!(data[0] == 8u8);
        let amount_bytes = amount.to_le_bytes();
        assert!(data[1] == amount_bytes[0]);
        assert!(data[2] == amount_bytes[1]);
        assert!(data[3] == amount_bytes[2]);
        assert!(data[4] == amount_bytes[3]);
        assert!(data[5] == amount_bytes[4]);
        assert!(data[6] == amount_bytes[5]);
        assert!(data[7] == amount_bytes[6]);
        assert!(data[8] == amount_bytes[7]);
    }

    // -- approve (disc=4, 9-byte buffer) -----------------------------------

    /// Prove that the `approve` instruction data layout is correct for all
    /// possible `amount` values.
    #[kani::proof]
    fn approve_instruction_layout() {
        let amount: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 4u8);
            (ptr.add(1) as *mut u64).write_unaligned(amount);
            buf.assume_init()
        };

        assert!(data[0] == 4u8);
        let amount_bytes = amount.to_le_bytes();
        assert!(data[1] == amount_bytes[0]);
        assert!(data[2] == amount_bytes[1]);
        assert!(data[3] == amount_bytes[2]);
        assert!(data[4] == amount_bytes[3]);
        assert!(data[5] == amount_bytes[4]);
        assert!(data[6] == amount_bytes[5]);
        assert!(data[7] == amount_bytes[6]);
        assert!(data[8] == amount_bytes[7]);
    }

    // -- transfer_checked (disc=12, 10-byte buffer) ------------------------

    /// Prove that the `transfer_checked` instruction data layout is correct
    /// for all possible `amount` and `decimals` values.
    #[kani::proof]
    fn transfer_checked_instruction_layout() {
        let amount: u64 = kani::any();
        let decimals: u8 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 10]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 12u8);
            (ptr.add(1) as *mut u64).write_unaligned(amount);
            core::ptr::write(ptr.add(9), decimals);
            buf.assume_init()
        };

        assert!(data[0] == 12u8);
        let amount_bytes = amount.to_le_bytes();
        assert!(data[1] == amount_bytes[0]);
        assert!(data[2] == amount_bytes[1]);
        assert!(data[3] == amount_bytes[2]);
        assert!(data[4] == amount_bytes[3]);
        assert!(data[5] == amount_bytes[4]);
        assert!(data[6] == amount_bytes[5]);
        assert!(data[7] == amount_bytes[6]);
        assert!(data[8] == amount_bytes[7]);
        assert!(data[9] == decimals);
    }

    // -- initialize_account3 (disc=18, 33-byte buffer) ---------------------

    /// Prove that the `initialize_account3` instruction data layout is
    /// correct for all possible owner addresses.
    #[kani::proof]
    fn initialize_account3_instruction_layout() {
        let owner: [u8; 32] = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 33]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 18u8);
            core::ptr::copy_nonoverlapping(owner.as_ptr(), ptr.add(1), 32);
            buf.assume_init()
        };

        // Discriminator
        assert!(data[0] == 18u8);
        // Owner address at [1..33]
        let mut i: usize = 0;
        while i < 32 {
            assert!(data[1 + i] == owner[i]);
            i += 1;
        }
    }

    // -- initialize_mint2 (disc=20, 67-byte buffer) ------------------------

    /// Prove that the `initialize_mint2` instruction data layout is correct
    /// when a freeze authority IS provided.
    #[kani::proof]
    fn initialize_mint2_instruction_layout_with_freeze() {
        let decimals: u8 = kani::any();
        let mint_authority: [u8; 32] = kani::any();
        let freeze_authority: [u8; 32] = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 67]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 20u8);
            core::ptr::write(ptr.add(1), decimals);
            core::ptr::copy_nonoverlapping(mint_authority.as_ptr(), ptr.add(2), 32);
            // freeze authority present
            core::ptr::write(ptr.add(34), 1u8);
            core::ptr::copy_nonoverlapping(freeze_authority.as_ptr(), ptr.add(35), 32);
            buf.assume_init()
        };

        // Discriminator
        assert!(data[0] == 20u8);
        // Decimals
        assert!(data[1] == decimals);
        // Mint authority at [2..34]
        let mut i: usize = 0;
        while i < 32 {
            assert!(data[2 + i] == mint_authority[i]);
            i += 1;
        }
        // has_freeze_auth flag
        assert!(data[34] == 1u8);
        // Freeze authority at [35..67]
        i = 0;
        while i < 32 {
            assert!(data[35 + i] == freeze_authority[i]);
            i += 1;
        }
    }

    /// Prove that the `initialize_mint2` instruction data layout is correct
    /// when NO freeze authority is provided (33 zero bytes at [34..67]).
    #[kani::proof]
    fn initialize_mint2_instruction_layout_without_freeze() {
        let decimals: u8 = kani::any();
        let mint_authority: [u8; 32] = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 67]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 20u8);
            core::ptr::write(ptr.add(1), decimals);
            core::ptr::copy_nonoverlapping(mint_authority.as_ptr(), ptr.add(2), 32);
            // no freeze authority — zero 33 bytes
            core::ptr::write_bytes(ptr.add(34), 0, 33);
            buf.assume_init()
        };

        // Discriminator
        assert!(data[0] == 20u8);
        // Decimals
        assert!(data[1] == decimals);
        // Mint authority at [2..34]
        let mut i: usize = 0;
        while i < 32 {
            assert!(data[2 + i] == mint_authority[i]);
            i += 1;
        }
        // has_freeze_auth flag must be 0
        assert!(data[34] == 0u8);
        // Remaining 32 bytes must be zero
        i = 0;
        while i < 32 {
            assert!(data[35 + i] == 0u8);
            i += 1;
        }
    }
}
