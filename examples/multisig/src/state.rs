use quasar_lang::prelude::*;

/// Configuration for one multisig: signer set, threshold, and label.
#[account(discriminator = 1, set_inner)]
#[seeds(b"multisig", creator: Address)]
pub struct MultisigConfig {
    pub creator: Address,
    /// Minimum number of signer approvals required to execute.
    pub threshold: u8,
    pub bump: u8,
    pub label: String<32>,
    pub signers: Vec<Address, 10>,
}
