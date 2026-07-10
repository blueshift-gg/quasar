use crate::prelude::*;

define_account!(
    /// An account with no validation.
    ///
    /// Useful for accounts passed through to CPI calls or whose
    /// constraints are checked manually by the instruction handler. No
    /// owner, signer, writable, or data checks are performed.
    pub struct UncheckedAccount => []
);

impl crate::account_load::AccountLoad for UncheckedAccount {
    #[inline(always)]
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

impl<'input> crate::remaining::RemainingItem<'input> for UncheckedAccount {
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

/// Bounds-checked data writes for unchecked account slots.
///
/// This keeps the common "write these bytes at this offset" path safe without
/// exposing a long-lived mutable account-data slice to handlers.
pub trait AccountDataWrite {
    /// Writes bytes at `offset`, rejecting immutable or undersized accounts.
    fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), ProgramError>;
}

impl AccountDataWrite for AccountView {
    #[inline(always)]
    fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), ProgramError> {
        if crate::utils::hint::unlikely(!self.is_writable()) {
            return Err(ProgramError::Immutable);
        }

        let end = offset
            .checked_add(bytes.len())
            .ok_or(ProgramError::AccountDataTooSmall)?;
        if crate::utils::hint::unlikely(end > self.data_len()) {
            return Err(ProgramError::AccountDataTooSmall);
        }

        // SAFETY: Writability was checked above, `end <= data_len`, and
        // `copy_nonoverlapping` reads only from the caller-provided byte slice.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.data_mut_ptr().add(offset),
                bytes.len(),
            );
        }
        Ok(())
    }
}

impl AccountDataWrite for UncheckedAccount {
    #[inline(always)]
    fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), ProgramError> {
        self.view.write_bytes(offset, bytes)
    }
}
