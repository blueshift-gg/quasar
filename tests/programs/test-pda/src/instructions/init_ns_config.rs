use {
    crate::state::{NamespaceConfig, NamespaceConfigInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitNsConfig {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = NamespaceConfig::seeds())]
    pub config: Account<NamespaceConfig>,
    pub system_program: Program<SystemProgram>,
}

impl InitNsConfig {
    pub fn handler(
        &mut self,
        namespace: u32,
        bumps: &InitNsConfigBumps,
    ) -> Result<(), ProgramError> {
        self.config.set_inner(NamespaceConfigInner {
            namespace,
            bump: bumps.config,
        });
        Ok(())
    }
}
