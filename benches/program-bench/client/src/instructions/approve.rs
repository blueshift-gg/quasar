use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct ApproveInstruction {
    pub authority: Address,
    pub source: Address,
    pub delegate: Address,
    pub amount: u64,
}

impl From<ApproveInstruction> for Instruction {
    fn from(ix: ApproveInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.authority, true),
            AccountMeta::new(ix.source, false),
            AccountMeta::new_readonly(ix.delegate, false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
        ];
        let mut data = vec![46];
        wincode::serialize_into(&mut data, &ix.amount).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
