// Self-CPI to return_pair (disc 32, 12-byte payload) then decode as u64
// (8 bytes), propagating the error: must surface
// QuasarError::InvalidReturnData (3019) to the transaction level.
use {crate::state::TestErrorsProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CpiDecodeMismatch {
    pub program: Program<TestErrorsProgram>,
}

impl CpiDecodeMismatch {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let ret = quasar_lang::cpi::CpiCall::<1, 1>::new(
            &crate::ID,
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.program.address(),
            )],
            [self.program.to_account_view()],
            [32],
        )
        .invoke_with_return()?;
        let value = ret.decode::<u64>()?;
        let _ = core::hint::black_box(value);
        Ok(())
    }
}
