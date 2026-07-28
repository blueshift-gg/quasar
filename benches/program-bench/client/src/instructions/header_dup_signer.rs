use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct HeaderDupSignerInstruction {
    pub payer: Address,
    pub authority: Address,
}

impl From<HeaderDupSignerInstruction> for Instruction {
    fn from(ix: HeaderDupSignerInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new_readonly(ix.authority, true),
        ];
        let data = vec![83];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
