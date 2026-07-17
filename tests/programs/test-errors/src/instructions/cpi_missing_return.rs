// Self-CPI to err_plain_ok (disc 31) with invoke_with_return, propagating the
// error: the callee sets no return data, so this must surface
// QuasarError::MissingReturnData (3017) to the transaction level.
use {crate::state::TestErrorsProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CpiMissingReturn {
    pub program: Program<TestErrorsProgram>,
}

impl CpiMissingReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let ret = quasar_lang::cpi::CpiCall::<1, 1>::new(
            &crate::ID,
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.program.address(),
            )],
            [self.program.to_account_view()],
            [31],
        )
        .invoke_with_return()?;
        let _ = core::hint::black_box(ret.as_slice().len());
        Ok(())
    }
}
