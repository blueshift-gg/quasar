use crate::prelude::*;

define_account!(
    /// An account that must be a transaction signer.
    ///
    /// Validated during account parsing; the `is_signer` flag must be
    /// set. Does not check owner, data, or any other property.
    pub struct Signer => [checks::Signer]
);

impl crate::account_load::AccountLoad for Signer {
    const IS_SIGNER: bool = true;

    #[inline(always)]
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

impl<'input> crate::remaining::RemainingItem<'input> for Signer {
    const COUNT: usize = 1;
    const REJECT_DUPLICATES: bool = false;

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
