use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct TransferCheckedInterfaceInstruction {
    pub authority: Address,
    pub from: Address,
    pub mint: Address,
    pub to: Address,
    pub token_program: Address,
    pub amount: u64,
    pub decimals: u8,
}

impl From<TransferCheckedInterfaceInstruction> for Instruction {
    fn from(ix: TransferCheckedInterfaceInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new(ix.from, false),
            AccountMeta::new_readonly(ix.mint, false),
            AccountMeta::new(ix.to, false),
            AccountMeta::new_readonly(ix.token_program, false),
        ];
        let mut data = vec![51];
        wincode::serialize_into(&mut data, &ix.amount).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.decimals).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
