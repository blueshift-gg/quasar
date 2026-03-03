use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

// SPL Token instruction opcodes
const TRANSFER: u8 = 3;
const APPROVE: u8 = 4;
const REVOKE: u8 = 5;
const MINT_TO: u8 = 7;
const BURN: u8 = 8;
const CLOSE_ACCOUNT: u8 = 9;
const TRANSFER_CHECKED: u8 = 12;
const SYNC_NATIVE: u8 = 17;
const INITIALIZE_ACCOUNT3: u8 = 18;
const INITIALIZE_MINT2: u8 = 20;

/// Initialize a token account (InitializeAccount3 — opcode 18).
///
/// Free function variant for generated code that works with raw `AccountView`
/// references during parse-time init. Equivalent to [`TokenCpi::initialize_account3`].
#[inline(always)]
#[allow(dead_code)]
pub fn initialize_account3<'a>(
    token_program: &'a AccountView,
    account: &'a AccountView,
    mint: &'a AccountView,
    owner: &Address,
) -> CpiCall<'a, 2, 33> {
    // SAFETY: All 33 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 33]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, INITIALIZE_ACCOUNT3);
        core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(1), 32);
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
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
/// Free function variant for generated code that works with raw `AccountView`
/// references during parse-time init. Equivalent to [`TokenCpi::initialize_mint2`].
#[inline(always)]
#[allow(dead_code)]
pub fn initialize_mint2<'a>(
    token_program: &'a AccountView,
    mint: &'a AccountView,
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
) -> CpiCall<'a, 1, 67> {
    // SAFETY: All 67 bytes are written before assume_init. The None branch
    // explicitly zeroes bytes 34..67 (COption::None tag + 32 padding bytes).
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, INITIALIZE_MINT2);
        core::ptr::write(ptr.add(1), decimals);
        core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
        match freeze_authority {
            Some(fa) => {
                core::ptr::write(ptr.add(34), 1u8);
                core::ptr::copy_nonoverlapping(fa.as_ref().as_ptr(), ptr.add(35), 32);
            }
            None => {
                core::ptr::write_bytes(ptr.add(34), 0, 33);
            }
        }
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
        [InstructionAccount::writable(mint.address())],
        [mint],
        data,
    )
}

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

        // SAFETY: All 9 bytes are written before assume_init.
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, TRANSFER);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

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

        // SAFETY: All 10 bytes are written before assume_init.
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 10]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, TRANSFER_CHECKED);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            core::ptr::write(ptr.add(9), decimals);
            buf.assume_init()
        };

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

        // SAFETY: All 9 bytes are written before assume_init.
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, MINT_TO);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

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

        // SAFETY: All 9 bytes are written before assume_init.
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, BURN);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

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

        // SAFETY: All 9 bytes are written before assume_init.
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, APPROVE);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

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
            [CLOSE_ACCOUNT],
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
            [REVOKE],
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
            [SYNC_NATIVE],
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

        // SAFETY: All 33 bytes are written before assume_init.
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 33]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, INITIALIZE_ACCOUNT3);
            core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(1), 32);
            buf.assume_init()
        };

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

        // SAFETY: All 67 bytes are written before assume_init. The None branch
        // explicitly zeroes bytes 34..67 (COption::None tag + 32 padding bytes).
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 67]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, INITIALIZE_MINT2);
            core::ptr::write(ptr.add(1), decimals);
            core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
            match freeze_authority {
                Some(fa) => {
                    core::ptr::write(ptr.add(34), 1u8);
                    core::ptr::copy_nonoverlapping(fa.as_ref().as_ptr(), ptr.add(35), 32);
                }
                None => {
                    core::ptr::write_bytes(ptr.add(34), 0, 33);
                }
            }
            buf.assume_init()
        };

        CpiCall::new(
            self.address(),
            [InstructionAccount::writable(mint.address())],
            [mint],
            data,
        )
    }
}
