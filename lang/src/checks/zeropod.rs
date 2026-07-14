use {
    crate::account_layout::AccountLayout, solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Validates `DATA_SIZE` bytes at `DATA_OFFSET` via `ZeroPodFixed::validate`.
///
/// Self-guarding: includes its own range check before slicing, so it can be
/// used standalone without `checks::DataLen`.
pub trait ZeroPod: AccountLayout {
    /// Validates the schema using the unchecked unique-account fast path.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        // SAFETY: This is the unchecked fast path used when generated parsing
        // has ruled out aliasing that requires runtime borrow tracking.
        let data = unsafe { view.borrow_unchecked() };
        Self::check_data(data)
    }

    /// Validates the schema through a runtime-checked data borrow.
    #[inline(always)]
    fn check_checked(view: &AccountView) -> Result<(), ProgramError> {
        let data = view.try_borrow()?;
        Self::check_data(&data)
    }

    /// Validates the schema in an already borrowed data slice.
    #[inline(always)]
    fn check_data(data: &[u8]) -> Result<(), ProgramError> {
        let offset = Self::DATA_OFFSET;
        let size = Self::DATA_SIZE;
        let end = offset + size;
        if data.len() < end {
            return Err(ProgramError::AccountDataTooSmall);
        }
        <Self::Schema as crate::__zeropod::ZeroPodFixed>::validate(&data[offset..end])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}
