use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct EmitEmptyEventInstruction {
    pub signer: Address,
}

impl From<EmitEmptyEventInstruction> for Instruction {
    fn from(ix: EmitEmptyEventInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
        ];
        let data = vec![93];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
