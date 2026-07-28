use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CloseTokenAccountInstruction {
    pub account: Address,
    pub destination: Address,
    pub authority: Address,
}

impl From<CloseTokenAccountInstruction> for Instruction {
    fn from(ix: CloseTokenAccountInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.account, false),
            AccountMeta::new(ix.destination, true),
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
        ];
        let data = vec![48];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
