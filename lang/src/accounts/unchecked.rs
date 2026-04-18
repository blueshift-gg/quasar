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
    type Params = ();

    #[inline(always)]
    fn check(_view: &AccountView, _field_name: &str) -> Result<(), ProgramError> {
        Ok(())
    }
}
