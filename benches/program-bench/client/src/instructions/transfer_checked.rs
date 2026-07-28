use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct TransferCheckedInstruction {
    pub authority: Address,
    pub from: Address,
    pub mint: Address,
    pub to: Address,
    pub amount: u64,
    pub decimals: u8,
}

impl From<TransferCheckedInstruction> for Instruction {
    fn from(ix: TransferCheckedInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new(ix.from, false),
            AccountMeta::new_readonly(ix.mint, false),
            AccountMeta::new(ix.to, false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
        ];
        let mut data = vec![43];
        wincode::serialize_into(&mut data, &ix.amount).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.decimals).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
