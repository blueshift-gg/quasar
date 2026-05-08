use {
    crate::account_layout::AccountLayout, solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Validates `DATA_SIZE` bytes at `DATA_OFFSET` via `ZeroPodFixed::validate`.
///
/// Self-guarding: includes its own range check before slicing, so it can be
/// used standalone without `checks::DataLen`.
pub trait ZeroPod: AccountLayout {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        let data = unsafe { view.borrow_unchecked() };
        Self::check_data(data)
    }

    #[inline(always)]
    fn check_checked(view: &AccountView) -> Result<(), ProgramError> {
        let data = view.try_borrow()?;
        Self::check_data(&data)
    }

    #[inline(always)]
    fn check_data(data: &[u8]) -> Result<(), ProgramError> {
        let offset = Self::DATA_OFFSET;
        let size = Self::DATA_SIZE;
        if data.len() < offset + size {
            return Err(ProgramError::AccountDataTooSmall);
        }
        <Self::Schema as crate::__zeropod::ZeroPodFixed>::validate(&data[offset..offset + size])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}
