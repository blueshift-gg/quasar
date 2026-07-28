use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct DynamicReadbackInstruction {
    pub account: Address,
    pub expected_name_len: u8,
    pub expected_tags_count: u8,
}

impl From<DynamicReadbackInstruction> for Instruction {
    fn from(ix: DynamicReadbackInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.account, false),
        ];
        let mut data = vec![22];
        wincode::serialize_into(&mut data, &ix.expected_name_len).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.expected_tags_count).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
