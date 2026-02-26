use super::{CpiCall, InstructionAccount};
use crate::checks;
use crate::sysvars::rent::Rent;
use crate::traits::{AsAccountView, Program};
use solana_account_view::AccountView;
use solana_address::Address;
use solana_program_error::ProgramError;

pub const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0u8; 32]);

// --- Free functions (used by derive macro init_signed + account realloc) ---

#[inline(always)]
pub fn create_account<'a>(
    from: &'a AccountView,
    to: &'a AccountView,
    lamports: impl Into<u64>,
    space: u64,
    owner: &'a Address,
) -> CpiCall<'a, 2, 52> {
    let lamports: u64 = lamports.into();
    // SAFETY: All 52 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 52]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::copy_nonoverlapping(0u32.to_le_bytes().as_ptr(), ptr, 4);
        core::ptr::copy_nonoverlapping(lamports.to_le_bytes().as_ptr(), ptr.add(4), 8);
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

#[inline(always)]
pub fn transfer<'a>(
    from: &'a AccountView,
    to: &'a AccountView,
    lamports: impl Into<u64>,
) -> CpiCall<'a, 2, 12> {
    let lamports: u64 = lamports.into();
    let mut data = [0u8; 12];
    data[0] = 2;
    data[4..12].copy_from_slice(&lamports.to_le_bytes());

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

#[inline(always)]
pub fn assign<'a>(account: &'a AccountView, owner: &'a Address) -> CpiCall<'a, 1, 36> {
    let mut data = [0u8; 36];
    data[0] = 1;
    data[4..36].copy_from_slice(owner.as_ref());

    CpiCall::new(
        &SYSTEM_PROGRAM_ID,
        [InstructionAccount::writable_signer(account.address())],
        [account],
        data,
    )
}

// --- SystemProgram account type ---

define_account!(pub struct SystemProgram => [checks::Executable, checks::Address]);

impl Program for SystemProgram {
    const ID: Address = Address::new_from_array([0u8; 32]);
}

impl SystemProgram {
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

    #[inline(always)]
    pub fn transfer<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        lamports: impl Into<u64>,
    ) -> CpiCall<'a, 2, 12> {
        transfer(from.to_account_view(), to.to_account_view(), lamports)
    }

    #[inline(always)]
    pub fn create_account_with_minimum_balance<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        space: u64,
        owner: &'a Address,
        rent: Option<&Rent>,
    ) -> Result<CpiCall<'a, 2, 52>, ProgramError> {
        let lamports = match rent {
            Some(r) => r.try_minimum_balance(space as usize)?,
            None => {
                use crate::sysvars::Sysvar;
                Rent::get()?.try_minimum_balance(space as usize)?
            }
        };
        Ok(create_account(
            from.to_account_view(),
            to.to_account_view(),
            lamports,
            space,
            owner,
        ))
    }

    #[inline(always)]
    pub fn assign<'a>(
        &'a self,
        account: &'a impl AsAccountView,
        owner: &'a Address,
    ) -> CpiCall<'a, 1, 36> {
        assign(account.to_account_view(), owner)
    }
}
