use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct DynStrCheckInstruction {
    pub account: Address,
    pub expected_len: u8,
}

impl From<DynStrCheckInstruction> for Instruction {
    fn from(ix: DynStrCheckInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.account, false),
        ];
        let mut data = vec![27];
        wincode::serialize_into(&mut data, &ix.expected_len).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
