use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct VerifyPdaDynamicInstructionRaw {
    pub authority: Address,
    pub account: Address,
}

pub struct VerifyPdaDynamicInstruction {
    pub authority: Address,
}

impl From<VerifyPdaDynamicInstruction> for VerifyPdaDynamicInstructionRaw {
    fn from(ix: VerifyPdaDynamicInstruction) -> VerifyPdaDynamicInstructionRaw {
        let authority = ix.authority;
        let account = Address::find_program_address(&[b"pda", authority.as_ref()], &ID).0;
        VerifyPdaDynamicInstructionRaw {
            authority,
            account,
        }
    }
}

impl From<VerifyPdaDynamicInstruction> for Instruction {
    fn from(ix: VerifyPdaDynamicInstruction) -> Instruction {
        VerifyPdaDynamicInstructionRaw::from(ix).into()
    }
}

impl From<VerifyPdaDynamicInstructionRaw> for Instruction {
    fn from(ix: VerifyPdaDynamicInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, false),
            AccountMeta::new_readonly(ix.account, false),
        ];
        let data = vec![9];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
