use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct RevokeInstruction {
    pub authority: Address,
    pub source: Address,
}

impl From<RevokeInstruction> for Instruction {
    fn from(ix: RevokeInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new(ix.source, false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
        ];
        let data = vec![47];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
