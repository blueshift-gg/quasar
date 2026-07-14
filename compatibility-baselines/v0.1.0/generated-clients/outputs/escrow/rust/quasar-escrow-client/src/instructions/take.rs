use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct TakeInstruction {
    pub taker: Address,
    pub escrow: Address,
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub taker_ta_a: Address,
    pub taker_ta_b: Address,
    pub maker_ta_b: Address,
    pub vault_ta_a: Address,
}

impl From<TakeInstruction> for Instruction {
    fn from(ix: TakeInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.taker, true),
            AccountMeta::new(ix.escrow, false),
            AccountMeta::new(ix.maker, false),
            AccountMeta::new_readonly(ix.mint_a, false),
            AccountMeta::new_readonly(ix.mint_b, false),
            AccountMeta::new(ix.taker_ta_a, true),
            AccountMeta::new(ix.taker_ta_b, false),
            AccountMeta::new(ix.maker_ta_b, true),
            AccountMeta::new(ix.vault_ta_a, false),
            AccountMeta::new_readonly(solana_address::address!("SysvarRent111111111111111111111111111111111"), false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![1];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
