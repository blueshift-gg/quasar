use {crate::state::*, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CheckConsensus {
    pub signer: Signer,
    pub consensus: Account<ConsensusAccount>,
}

impl CheckConsensus {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let _t = self.consensus.threshold();
        Ok(())
    }
}
