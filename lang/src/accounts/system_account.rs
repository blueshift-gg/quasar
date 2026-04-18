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
    type Params = ();

    #[inline(always)]
    fn check(view: &AccountView, _field_name: &str) -> Result<(), ProgramError> {
        <Self as checks::Owner>::check(view)
    }
}
