use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct TwoAccountsCheckInstruction {
    pub first: Address,
    pub second: Address,
}

impl From<TwoAccountsCheckInstruction> for Instruction {
    fn from(ix: TwoAccountsCheckInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.first, false),
            AccountMeta::new_readonly(ix.second, false),
        ];
        let data = vec![85];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
