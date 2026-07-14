use {
    crate::account_layout::AccountLayout, solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Validates that account data is at least `DATA_OFFSET + DATA_SIZE` bytes.
pub trait DataLen: AccountLayout {
    /// Checks that the account covers the declared layout range.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        let end = Self::DATA_OFFSET + Self::DATA_SIZE;
        if view.data_len() < end {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }
}
