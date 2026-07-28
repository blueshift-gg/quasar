use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct HeapVecOkInstruction {
    pub signer: Address,
}

impl From<HeapVecOkInstruction> for Instruction {
    fn from(ix: HeapVecOkInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.signer, true),
        ];
        let data = vec![100];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
