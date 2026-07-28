use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CreateAccountInstruction {
    pub admin: Address,
    pub account: Address,
}

impl From<CreateAccountInstruction> for Instruction {
    fn from(ix: CreateAccountInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.admin, true),
            AccountMeta::new(ix.account, true),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![2];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
