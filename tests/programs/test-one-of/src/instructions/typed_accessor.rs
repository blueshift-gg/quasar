use {crate::state::*, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct TypedAccessor {
    pub signer: Signer,
    pub consensus: Account<ConsensusAccount>,
}

impl TypedAccessor {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        // Pattern match on variant()
        match self.consensus.variant() {
            ConsensusAccountRef::Settings(s) => {
                let _auth = s.authority;
            }
            ConsensusAccountRef::Policy(p) => {
                let _max: u64 = p.max_amount.into();
            }
        }
        Ok(())
    }
}
