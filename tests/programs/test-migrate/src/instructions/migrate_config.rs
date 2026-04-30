use {crate::state::*, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct MigrateConfig {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,

    #[account(mut,
        constraints(config.authority == *authority.address()),
        payer = payer,
    )]
    pub config: Migration<ConfigV1, ConfigV2>,

    /// CHECK: authority validated via constraint
    pub authority: Signer,
}

impl MigrateConfig {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let old_val = self.config.value;
        let old_auth = self.config.authority;
        self.config.migrate(ConfigV2Data {
            authority: old_auth,
            value: old_val,
            extra: PodU32::from(42),
        })
    }
}
