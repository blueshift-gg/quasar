use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct EmitViaCpiInstruction {
    pub signer: Address,
    pub event_authority: Address,
    pub value: u64,
}

impl From<EmitViaCpiInstruction> for Instruction {
    fn from(ix: EmitViaCpiInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
            AccountMeta::new_readonly(ix.event_authority, false),
            AccountMeta::new_readonly(solana_address::address!("Bench111111111111111111111111111111111111111"), false),
        ];
        let mut data = vec![98];
        wincode::serialize_into(&mut data, &ix.value).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
