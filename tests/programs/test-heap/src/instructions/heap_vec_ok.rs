// This instruction is declared with #[instruction(heap)] because it allocates.
extern crate alloc;
use {alloc::vec, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct HeapVecOk {
    pub signer: Signer,
}

impl HeapVecOk {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        #[allow(clippy::useless_vec)]
        let v = vec![1u8; 64];
        if core::hint::black_box(v.len()) != 64 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}
