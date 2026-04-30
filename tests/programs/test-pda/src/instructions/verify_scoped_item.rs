use {
    crate::state::{NamespaceConfig, ScopedItem},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct VerifyScopedItem {
    pub config: Account<NamespaceConfig>,
    #[account(address = ScopedItem::seeds(config.namespace.into()))]
    pub item: Account<ScopedItem>,
}

impl VerifyScopedItem {
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        Ok(())
    }
}
