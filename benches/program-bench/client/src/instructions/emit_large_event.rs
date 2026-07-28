use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct EmitLargeEventInstruction {
    pub signer: Address,
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
    pub e: Address,
    pub f: Address,
    pub g: u128,
    pub h: u128,
}

impl From<EmitLargeEventInstruction> for Instruction {
    fn from(ix: EmitLargeEventInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
        ];
        let mut data = vec![96];
        wincode::serialize_into(&mut data, &ix.a).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.b).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.c).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.d).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.e).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.f).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.g).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.h).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
