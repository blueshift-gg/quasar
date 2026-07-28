use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CloseAccountInstructionRaw {
    pub authority: Address,
    pub account: Address,
}

pub struct CloseAccountInstruction {
    pub authority: Address,
}

impl From<CloseAccountInstruction> for CloseAccountInstructionRaw {
    fn from(ix: CloseAccountInstruction) -> CloseAccountInstructionRaw {
        let authority = ix.authority;
        let account = Address::find_program_address(&[b"simple", authority.as_ref()], &ID).0;
        CloseAccountInstructionRaw {
            authority,
            account,
        }
    }
}

impl From<CloseAccountInstruction> for Instruction {
    fn from(ix: CloseAccountInstruction) -> Instruction {
        CloseAccountInstructionRaw::from(ix).into()
    }
}

impl From<CloseAccountInstructionRaw> for Instruction {
    fn from(ix: CloseAccountInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.authority, true),
            AccountMeta::new(ix.account, false),
        ];
        let data = vec![29];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
