use crate::prelude::*;

define_account!(pub struct SystemAccount => [checks::Owner]);

impl Owner for SystemAccount {
    const OWNER: Address = Address::new_from_array([0u8; 32]);
}