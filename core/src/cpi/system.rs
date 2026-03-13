use {
    super::{CpiCall, InstructionAccount},
    crate::{
        sysvars::rent::Rent,
        traits::{AsAccountView, Id},
    },
    solana_account_view::AccountView,
    solana_address::{declare_id, Address},
    solana_program_error::ProgramError,
};

declare_id!("11111111111111111111111111111111");
pub use ID as SYSTEM_PROGRAM_ID;

/// Create a new account via the System program.
///
/// ### Accounts:
///   0. `[WRITE, SIGNER]` Funding account
///   1. `[WRITE, SIGNER]` New account
///
/// ### Instruction data (52 bytes):
/// ```text
/// [0..4  ] discriminator (0)
/// [4..12 ] lamports      (u64 LE)
/// [12..20] space          (u64 LE)
/// [20..52] owner          (32-byte address)
/// ```
#[inline(always)]
pub fn create_account<'a>(
    from: &'a AccountView,
    to: &'a AccountView,
    lamports: impl Into<u64>,
    space: u64,
    owner: &'a Address,
) -> CpiCall<'a, 2, 52> {
    // SAFETY: All 52 bytes written before `assume_init`.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 52]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        // discriminator 0 — four zero bytes
        core::ptr::write_bytes(ptr, 0, 4);
        core::ptr::copy_nonoverlapping(lamports.into().to_le_bytes().as_ptr(), ptr.add(4), 8);
        core::ptr::copy_nonoverlapping(space.to_le_bytes().as_ptr(), ptr.add(12), 8);
        core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(20), 32);
        buf.assume_init()
    };

    CpiCall::new(
        &SYSTEM_PROGRAM_ID,
        [
            InstructionAccount::writable_signer(from.address()),
            InstructionAccount::writable_signer(to.address()),
        ],
        [from, to],
        data,
    )
}

/// Transfer lamports between accounts via the System program.
///
/// ### Accounts:
///   0. `[WRITE, SIGNER]` Source account
///   1. `[WRITE]` Destination account
///
/// ### Instruction data (12 bytes):
/// ```text
/// [0..4 ] discriminator (2)
/// [4..12] lamports      (u64 LE)
/// ```
#[inline(always)]
pub fn transfer<'a>(
    from: &'a AccountView,
    to: &'a AccountView,
    lamports: impl Into<u64>,
) -> CpiCall<'a, 2, 12> {
    // SAFETY: All 12 bytes written before `assume_init`.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 12]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::copy_nonoverlapping(2u32.to_le_bytes().as_ptr(), ptr, 4);
        core::ptr::copy_nonoverlapping(lamports.into().to_le_bytes().as_ptr(), ptr.add(4), 8);
        buf.assume_init()
    };

    CpiCall::new(
        &SYSTEM_PROGRAM_ID,
        [
            InstructionAccount::writable_signer(from.address()),
            InstructionAccount::writable(to.address()),
        ],
        [from, to],
        data,
    )
}

/// Assign an account to a new owner program via the System program.
///
/// ### Accounts:
///   0. `[WRITE, SIGNER]` Account to assign
///
/// ### Instruction data (36 bytes):
/// ```text
/// [0..4 ] discriminator (1)
/// [4..36] owner          (32-byte address)
/// ```
#[inline(always)]
pub fn assign<'a>(account: &'a AccountView, owner: &'a Address) -> CpiCall<'a, 1, 36> {
    // SAFETY: All 36 bytes written before `assume_init`.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 36]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::copy_nonoverlapping(1u32.to_le_bytes().as_ptr(), ptr, 4);
        core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(4), 32);
        buf.assume_init()
    };

    CpiCall::new(
        &SYSTEM_PROGRAM_ID,
        [InstructionAccount::writable_signer(account.address())],
        [account],
        data,
    )
}

// --- System program account type ---

/// Marker type for the system program.
///
/// Use with the `Program<T>` wrapper:
/// ```ignore
/// pub system_program: &'info Program<System>,
/// ```
pub struct System;

impl Id for System {
    const ID: Address = Address::new_from_array([0u8; 32]);
}

impl crate::accounts::Program<System> {
    /// Create a new account. See [`create_account`] for account and data
    /// layout.
    #[inline(always)]
    pub fn create_account<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        lamports: impl Into<u64>,
        space: u64,
        owner: &'a Address,
    ) -> CpiCall<'a, 2, 52> {
        create_account(
            from.to_account_view(),
            to.to_account_view(),
            lamports,
            space,
            owner,
        )
    }

    /// Transfer lamports. See [`transfer`] for account and data layout.
    #[inline(always)]
    pub fn transfer<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        lamports: impl Into<u64>,
    ) -> CpiCall<'a, 2, 12> {
        transfer(from.to_account_view(), to.to_account_view(), lamports)
    }

    /// Create a new account with the minimum rent-exempt balance.
    ///
    /// If `rent` is `None`, fetches the Rent sysvar via syscall.
    ///
    /// # Errors
    ///
    /// Returns `ProgramError::InvalidArgument` if `space` exceeds the
    /// maximum permitted data length.
    #[inline(always)]
    pub fn create_account_with_minimum_balance<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        space: u64,
        owner: &'a Address,
        rent: Option<&Rent>,
    ) -> Result<CpiCall<'a, 2, 52>, ProgramError> {
        let lamports = if let Some(r) = rent {
            r.try_minimum_balance(space as usize)?
        } else {
            use crate::sysvars::Sysvar;
            Rent::get()?.try_minimum_balance(space as usize)?
        };
        Ok(create_account(
            from.to_account_view(),
            to.to_account_view(),
            lamports,
            space,
            owner,
        ))
    }

    /// Assign an account to a new owner. See [`assign`] for details.
    #[inline(always)]
    pub fn assign<'a>(
        &'a self,
        account: &'a impl AsAccountView,
        owner: &'a Address,
    ) -> CpiCall<'a, 1, 36> {
        assign(account.to_account_view(), owner)
    }
}
