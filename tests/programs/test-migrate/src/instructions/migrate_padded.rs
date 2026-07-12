use {crate::state::*, quasar_derive::Accounts, quasar_lang::prelude::*};

/// Regression coverage for issue #239: migrates a hand-rolled account whose
/// `To::SPACE` reserves padding beyond `disc_len + size_of::<To::Target>()`,
/// and whose `To::SPACE` is smaller than `From::SPACE`. Exercises the exact
/// shrink-with-padding path `write_target` previously left dirty.
#[derive(Accounts)]
pub struct MigratePadded {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
    pub config: Migration<PaddedSourceV1, PaddedTarget>,
}

impl MigratePadded {
    #[inline(always)]
    pub fn handler(&mut self, authority: Address, value: u64) -> Result<(), ProgramError> {
        self.config.migrate(
            &self.payer,
            PaddedTargetData {
                authority,
                value: PodU64::from(value),
            },
        )?;
        Ok(())
    }
}
