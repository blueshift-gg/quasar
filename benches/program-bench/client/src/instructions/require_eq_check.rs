use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct RequireEqCheckInstruction {
    pub signer: Address,
    pub a: u64,
    pub b: u64,
}

impl From<RequireEqCheckInstruction> for Instruction {
    fn from(ix: RequireEqCheckInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
        ];
        let mut data = vec![91];
        wincode::serialize_into(&mut data, &ix.a).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.b).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
