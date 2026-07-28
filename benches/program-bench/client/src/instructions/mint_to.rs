use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct MintToInstruction {
    pub authority: Address,
    pub mint: Address,
    pub to: Address,
    pub amount: u64,
}

impl From<MintToInstruction> for Instruction {
    fn from(ix: MintToInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new(ix.mint, false),
            AccountMeta::new(ix.to, false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
        ];
        let mut data = vec![44];
        wincode::serialize_into(&mut data, &ix.amount).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
