use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct HeaderNodupSignerInstruction {
    pub account: Address,
}

impl From<HeaderNodupSignerInstruction> for Instruction {
    fn from(ix: HeaderNodupSignerInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.account, true),
        ];
        let data = vec![81];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
