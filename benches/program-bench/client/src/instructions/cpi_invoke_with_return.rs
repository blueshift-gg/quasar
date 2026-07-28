use solana_instruction::{AccountMeta, Instruction};
use crate::ID;

pub struct CpiInvokeWithReturnInstruction {
}

impl From<CpiInvokeWithReturnInstruction> for Instruction {
    fn from(ix: CpiInvokeWithReturnInstruction) -> Instruction {
        let _ = ix;
        let accounts = vec![
            AccountMeta::new_readonly(solana_address::address!("Bench111111111111111111111111111111111111111"), false),
        ];
        let data = vec![106];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
