use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct EmitU64EventInstruction {
    pub signer: Address,
    pub value: u64,
}

impl From<EmitU64EventInstruction> for Instruction {
    fn from(ix: EmitU64EventInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
        ];
        let mut data = vec![94];
        wincode::serialize_into(&mut data, &ix.value).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
