use crate::prelude::*;

define_account!(
    /// An account that must be a transaction signer.
    ///
    /// Validated during account parsing — the `is_signer` flag must be
    /// set. Does not check owner, data, or any other property.
    pub struct Signer => [checks::Signer]
);

impl crate::account_load::AccountLoad for Signer {
    const IS_SIGNER: bool = true;

    type Params = ();

    #[inline(always)]
    fn check(_view: &AccountView, _field_name: &str) -> Result<(), ProgramError> {
        Ok(())
    }
}
