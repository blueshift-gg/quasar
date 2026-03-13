use crate::prelude::*;

define_account!(
    /// An account with no validation.
    ///
    /// Useful for accounts passed through to CPI calls or whose
    /// constraints are checked manually by the instruction handler. No
    /// owner, signer, writable, or data checks are performed.
    pub struct UncheckedAccount => []
);
