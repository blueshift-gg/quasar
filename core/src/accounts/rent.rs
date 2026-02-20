use crate::prelude::*;

define_account!(pub struct Rent => [checks::Address]);

impl crate::traits::Program for Rent {
    const ID: Address = Address::new_from_array([
        6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155,
        161, 253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
    ]);
}

impl Rent {
    #[inline(always)]
    pub fn get(
        &self,
    ) -> Result<solana_account_view::Ref<'_, crate::sysvars::rent::Rent>, ProgramError> {
        crate::sysvars::rent::Rent::from_account_view(self.to_account_view())
    }

    /// Access rent data without borrow tracking or address verification.
    ///
    /// # Safety
    ///
    /// The caller must ensure this Rent account was already validated via
    /// `from_account_view` (which checks the address). Account data must
    /// not be mutably borrowed.
    #[inline(always)]
    pub unsafe fn get_unchecked(&self) -> &crate::sysvars::rent::Rent {
        crate::sysvars::rent::Rent::from_bytes_unchecked(self.to_account_view().borrow_unchecked())
    }
}
