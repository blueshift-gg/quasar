use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CreatePdaDynamicInstructionRaw {
    pub admin: Address,
    pub authority: Address,
    pub account: Address,
}

pub struct CreatePdaDynamicInstruction {
    pub admin: Address,
    pub authority: Address,
}

impl From<CreatePdaDynamicInstruction> for CreatePdaDynamicInstructionRaw {
    fn from(ix: CreatePdaDynamicInstruction) -> CreatePdaDynamicInstructionRaw {
        let admin = ix.admin;
        let authority = ix.authority;
        let account = Address::find_program_address(&[b"pda", authority.as_ref()], &ID).0;
        CreatePdaDynamicInstructionRaw {
            admin,
            authority,
            account,
        }
    }
}

impl From<CreatePdaDynamicInstruction> for Instruction {
    fn from(ix: CreatePdaDynamicInstruction) -> Instruction {
        CreatePdaDynamicInstructionRaw::from(ix).into()
    }
}

impl From<CreatePdaDynamicInstructionRaw> for Instruction {
    fn from(ix: CreatePdaDynamicInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.admin, true),
            AccountMeta::new_readonly(ix.authority, false),
            AccountMeta::new(ix.account, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![8];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
