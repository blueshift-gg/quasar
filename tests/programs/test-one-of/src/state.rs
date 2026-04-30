use quasar_lang::prelude::*;

// ---------------------------------------------------------------------------
// Shared trait
// ---------------------------------------------------------------------------

pub trait Consensus {
    fn threshold(&self) -> u16;
}

// ---------------------------------------------------------------------------
// Variant A: Settings
// ---------------------------------------------------------------------------

#[account(discriminator = 10)]
pub struct Settings {
    pub authority: Address,
    pub threshold: PodU16,
}

impl Consensus for Settings {
    fn threshold(&self) -> u16 {
        self.threshold.into()
    }
}

// ---------------------------------------------------------------------------
// Variant B: Policy
// ---------------------------------------------------------------------------

#[account(discriminator = 11)]
pub struct Policy {
    pub authority: Address,
    pub max_amount: PodU64,
    pub threshold: PodU16,
}

impl Consensus for Policy {
    fn threshold(&self) -> u16 {
        self.threshold.into()
    }
}

// ---------------------------------------------------------------------------
// Polymorphic type
// ---------------------------------------------------------------------------

#[account(one_of, implements(Consensus))]
pub enum ConsensusAccount {
    Settings(Settings),
    Policy(Policy),
}
