use {
    crate::state::{NamespaceConfig, ScopedItem, ScopedItemInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitScopedItemFromConfig {
    #[account(mut)]
    pub payer: Signer,
    pub config: Account<NamespaceConfig>,
    #[account(mut, init, payer = payer, address = ScopedItem::seeds(config.namespace.into()))]
    pub item: Account<ScopedItem>,
    pub system_program: Program<SystemProgram>,
}

impl InitScopedItemFromConfig {
    pub fn handler(&mut self, bumps: &InitScopedItemFromConfigBumps) -> Result<(), ProgramError> {
        self.item.set_inner(ScopedItemInner {
            namespace: self.config.namespace.into(),
            data: 0,
            bump: bumps.item,
        });
        Ok(())
    }
}
