use solana_instruction::{AccountMeta, Instruction};
use crate::ID;

pub struct CpiInvokeIgnoreReturnInstruction {
}

impl From<CpiInvokeIgnoreReturnInstruction> for Instruction {
    fn from(ix: CpiInvokeIgnoreReturnInstruction) -> Instruction {
        let _ = ix;
        let accounts = vec![
            AccountMeta::new_readonly(solana_address::address!("Bench111111111111111111111111111111111111111"), false),
        ];
        let data = vec![107];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
