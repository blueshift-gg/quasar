use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct HasOneDefaultInstruction {
    pub authority: Address,
    pub account: Address,
}

impl From<HasOneDefaultInstruction> for Instruction {
    fn from(ix: HasOneDefaultInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new_readonly(ix.account, false),
        ];
        let data = vec![87];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
