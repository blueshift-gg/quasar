use {
    crate::state::{Priority, Side},
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct EnumCheck {
    pub signer: Signer,
}

impl EnumCheck {
    #[inline(always)]
    pub fn handler(&self, side: Side, priority: Priority) -> Result<(), ProgramError> {
        // Sentinel values: the happy-path test submits exactly Ask + High.
        // Any other combination (including mutated explicit-discriminant
        // bytes that still happen to map to another declared variant)
        // must fail `require!`, surfacing as `InvalidInstructionData`.
        require!(side == Side::Ask, ProgramError::InvalidInstructionData);
        require!(
            priority == Priority::High,
            ProgramError::InvalidInstructionData
        );
        Ok(())
    }
}
