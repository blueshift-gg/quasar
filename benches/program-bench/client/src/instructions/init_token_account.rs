use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct InitTokenAccountInstruction {
    pub payer: Address,
    pub token_account: Address,
    pub mint: Address,
}

impl From<InitTokenAccountInstruction> for Instruction {
    fn from(ix: InitTokenAccountInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.token_account, true),
            AccountMeta::new_readonly(ix.mint, false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![41];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
