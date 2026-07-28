use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct EmitTwoEventsInstruction {
    pub signer: Address,
    pub first: u64,
    pub second: u64,
}

impl From<EmitTwoEventsInstruction> for Instruction {
    fn from(ix: EmitTwoEventsInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
        ];
        let mut data = vec![97];
        wincode::serialize_into(&mut data, &ix.first).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.second).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
