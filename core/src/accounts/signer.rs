use crate::prelude::*;

define_account!(
    /// An account that must be a transaction signer.
    ///
    /// Validated during account parsing — the `is_signer` flag must be
    /// set. Does not check owner, data, or any other property.
    pub struct Signer => [checks::Signer]
);
