use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct AssignTestInstruction {
    pub account: Address,
    pub owner: Address,
}

impl From<AssignTestInstruction> for Instruction {
    fn from(ix: AssignTestInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.account, true),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![32];
        wincode::serialize_into(&mut data, &ix.owner).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
