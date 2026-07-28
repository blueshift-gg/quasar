use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct InitMintInstruction {
    pub payer: Address,
    pub mint: Address,
    pub mint_authority: Address,
}

impl From<InitMintInstruction> for Instruction {
    fn from(ix: InitMintInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.mint, true),
            AccountMeta::new_readonly(ix.mint_authority, true),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![40];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
