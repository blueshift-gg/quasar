use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CreatePdaStaticInstructionRaw {
    pub admin: Address,
    pub account: Address,
}

pub struct CreatePdaStaticInstruction {
    pub admin: Address,
}

impl From<CreatePdaStaticInstruction> for CreatePdaStaticInstructionRaw {
    fn from(ix: CreatePdaStaticInstruction) -> CreatePdaStaticInstructionRaw {
        let admin = ix.admin;
        let account = Address::find_program_address(&[b"pda"], &ID).0;
        CreatePdaStaticInstructionRaw {
            admin,
            account,
        }
    }
}

impl From<CreatePdaStaticInstruction> for Instruction {
    fn from(ix: CreatePdaStaticInstruction) -> Instruction {
        CreatePdaStaticInstructionRaw::from(ix).into()
    }
}

impl From<CreatePdaStaticInstructionRaw> for Instruction {
    fn from(ix: CreatePdaStaticInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.admin, true),
            AccountMeta::new(ix.account, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![6];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
