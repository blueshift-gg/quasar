//! System-owned account wrapper.
//!
//! `SystemAccount` (via `define_account!`) validates during parsing that the
//! account's owner is the System program (the all-zeros address). Construction
//! follows the shared check-then-cast model in the module header.

use crate::prelude::*;

define_account!(
    /// An account owned by the System program (address `111...`).
    ///
    /// Validates that the account's owner is the all-zeros address.
    /// Typically used for SOL-holding accounts that have no program data.
    pub struct SystemAccount => [checks::Owner]
);

impl Owner for SystemAccount {
    const OWNER: Address = Address::new_from_array([0u8; 32]);
}

impl crate::account_load::AccountLoad for SystemAccount {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        <Self as checks::Owner>::check(view)
    }
}

impl<'input> crate::remaining::RemainingItem<'input> for SystemAccount {
    const COUNT: usize = 1;

    #[inline(always)]
    unsafe fn parse_remaining_one(
        account: AccountView,
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        crate::remaining::parse_remaining_view::<Self>(&account)
    }

    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [AccountView],
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        crate::remaining::parse_remaining_account::<Self>(accounts)
    }
}
