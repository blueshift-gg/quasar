use {crate::state::TestMiscProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CpiInvokeIgnoreReturn {
    pub program: Program<TestMiscProgram>,
}

impl CpiInvokeIgnoreReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        quasar_lang::cpi::CpiCall::<1, 1>::new(
            &crate::ID,
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.program.address(),
            )],
            [self.program.to_account_view()],
            [45],
        )
        .invoke()
    }
}
