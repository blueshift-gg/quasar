use solana_instruction::{AccountMeta, Instruction};
use crate::ID;

pub struct ProgramCheckInstruction {
}

impl From<ProgramCheckInstruction> for Instruction {
    fn from(ix: ProgramCheckInstruction) -> Instruction {
        let _ = ix;
        let accounts = vec![
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![89];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
