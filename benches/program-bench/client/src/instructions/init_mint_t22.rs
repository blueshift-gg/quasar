use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct InitMintT22Instruction {
    pub payer: Address,
    pub mint: Address,
    pub mint_authority: Address,
}

impl From<InitMintT22Instruction> for Instruction {
    fn from(ix: InitMintT22Instruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.mint, true),
            AccountMeta::new_readonly(ix.mint_authority, true),
            AccountMeta::new_readonly(solana_address::address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![50];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
