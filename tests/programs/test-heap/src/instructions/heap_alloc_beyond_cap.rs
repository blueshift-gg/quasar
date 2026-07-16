// Heap-enabled instruction that allocates past MAX_HEAP_LENGTH (256 KiB).
// The bump allocator must return null -> handle_alloc_error -> abort; the
// suite asserts the abort surfaces as ProgramFailedToComplete. Declared with
// #[instruction(heap)] so the failure comes from the cap guard on a valid
// heap arm, not from the poisoned cursor of a non-heap arm.
extern crate alloc;
use {alloc::vec, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct HeapAllocBeyondCap {
    pub signer: Signer,
}

impl HeapAllocBeyondCap {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        const BEYOND_CAP: usize = 256 * 1024 + 1;
        #[allow(clippy::useless_vec)]
        let v = vec![0u8; BEYOND_CAP];
        if core::hint::black_box(v.len()) != BEYOND_CAP {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}
