use {crate::state::VaultInterface, quasar_derive::Accounts, quasar_lang::prelude::*};

/// Accepts either VaultV1 or VaultV2 through a single InterfaceAccount field.
#[derive(Accounts)]
pub struct InterfaceMigrationCheck {
    pub vault: InterfaceAccount<VaultInterface>,
}

impl InterfaceMigrationCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
