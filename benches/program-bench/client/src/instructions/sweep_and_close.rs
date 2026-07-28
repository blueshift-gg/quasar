use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct SweepAndCloseInstruction {
    pub authority: Address,
    pub source: Address,
    pub receiver: Address,
    pub mint: Address,
    pub destination: Address,
}

impl From<SweepAndCloseInstruction> for Instruction {
    fn from(ix: SweepAndCloseInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new(ix.source, false),
            AccountMeta::new(ix.receiver, false),
            AccountMeta::new_readonly(ix.mint, false),
            AccountMeta::new(ix.destination, false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
        ];
        let data = vec![49];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
