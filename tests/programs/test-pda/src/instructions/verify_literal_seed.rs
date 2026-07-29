use {crate::state::ConfigAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

/// All-literal seeds: the derive bakes the PDA address at compile time, so
/// verification is a single 32-byte compare (no hash, no bump search).
#[derive(Accounts)]
pub struct VerifyLiteralSeed {
    #[account(address = ConfigAccount::seeds())]
    pub config: Account<ConfigAccount>,
}

impl VerifyLiteralSeed {
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        Ok(())
    }
}
