// Middle hop for the ReturnDataFromWrongProgram construction: CPIs the
// test-misc fixture's return_u64 (disc 45), which sets return data owned by
// test-misc, and deliberately sets none of its own. A caller that
// invoke_with_return's THIS instruction then observes return data stamped
// with a foreign program id. The target program account is passed in rather
// than hardcoded, so this crate needs no dependency on test-misc.
use {crate::state::TestErrorsProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CpiNestedMiscReturn {
    pub program: Program<TestErrorsProgram>,
    /// CHECK: fixture pass-through of the foreign return-data program.
    pub misc_program: UncheckedAccount,
}

impl CpiNestedMiscReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        quasar_lang::cpi::CpiCall::<1, 1>::new(
            self.misc_program.address(),
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.misc_program.address(),
            )],
            [self.misc_program.to_account_view()],
            [45],
        )
        .invoke()?;
        Ok(())
    }
}
