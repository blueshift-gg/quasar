use {
    crate::state::{NamespaceConfig, ScopedItem, ScopedItemInner},
    quasar_lang::prelude::*,
};

/// Tests that `seeds = Type::seeds(account.field)` works in an init context.
/// This exercises `typed_seed_slice_expr_init` in the derive macro.
#[derive(Accounts)]
pub struct InitScopedItemFromConfig {
    #[account(mut)]
    pub payer: Signer,
    pub config: Account<NamespaceConfig>,
    #[account(mut, init, payer = payer, seeds = ScopedItem::seeds(config.namespace), bump)]
    pub item: Account<ScopedItem>,
    pub system_program: Program<System>,
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
