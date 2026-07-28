use std::vec::Vec;
use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct RemainingTypedCheckInstruction {
    pub authority: Address,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl From<RemainingTypedCheckInstruction> for Instruction {
    fn from(ix: RemainingTypedCheckInstruction) -> Instruction {
        let mut accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
        ];
        accounts.extend(ix.remaining_accounts);
        let data = vec![92];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
