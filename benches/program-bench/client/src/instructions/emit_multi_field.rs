use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct EmitMultiFieldInstruction {
    pub signer: Address,
    pub a: u64,
    pub b: u64,
    pub c: Address,
}

impl From<EmitMultiFieldInstruction> for Instruction {
    fn from(ix: EmitMultiFieldInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
        ];
        let mut data = vec![95];
        wincode::serialize_into(&mut data, &ix.a).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.b).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.c).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
