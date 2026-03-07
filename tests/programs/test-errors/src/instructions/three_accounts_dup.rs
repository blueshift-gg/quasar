use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct ThreeAccountsDup<'info> {
    pub first: &'info Signer,
    pub second: &'info mut UncheckedAccount,
    pub third: &'info UncheckedAccount,
}

impl<'info> ThreeAccountsDup<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
