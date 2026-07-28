use solana_instruction::{AccountMeta, Instruction};
use crate::ID;

pub struct ReturnU64Instruction {
}

impl From<ReturnU64Instruction> for Instruction {
    fn from(ix: ReturnU64Instruction) -> Instruction {
        let _ = ix;
        let accounts = vec![
            AccountMeta::new_readonly(solana_address::address!("Bench111111111111111111111111111111111111111"), false),
        ];
        let data = vec![105];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
