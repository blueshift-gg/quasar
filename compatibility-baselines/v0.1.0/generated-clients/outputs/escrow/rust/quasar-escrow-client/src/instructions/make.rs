use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct MakeInstruction {
    pub maker: Address,
    pub escrow: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_a: Address,
    pub maker_ta_b: Address,
    pub vault_ta_a: Address,
    pub deposit: u64,
    pub receive: u64,
}

impl From<MakeInstruction> for Instruction {
    fn from(ix: MakeInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.maker, true),
            AccountMeta::new(ix.escrow, false),
            AccountMeta::new_readonly(ix.mint_a, false),
            AccountMeta::new_readonly(ix.mint_b, false),
            AccountMeta::new(ix.maker_ta_a, false),
            AccountMeta::new(ix.maker_ta_b, true),
            AccountMeta::new(ix.vault_ta_a, true),
            AccountMeta::new_readonly(solana_address::address!("SysvarRent111111111111111111111111111111111"), false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![0];
        wincode::serialize_into(&mut data, &ix.deposit).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.receive).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
