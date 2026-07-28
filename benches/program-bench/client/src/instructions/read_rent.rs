use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct ReadRentInstructionRaw {
    pub payer: Address,
    pub snapshot: Address,
}

pub struct ReadRentInstruction {
    pub payer: Address,
}

impl From<ReadRentInstruction> for ReadRentInstructionRaw {
    fn from(ix: ReadRentInstruction) -> ReadRentInstructionRaw {
        let payer = ix.payer;
        let snapshot = Address::find_program_address(&[b"rent"], &ID).0;
        ReadRentInstructionRaw {
            payer,
            snapshot,
        }
    }
}

impl From<ReadRentInstruction> for Instruction {
    fn from(ix: ReadRentInstruction) -> Instruction {
        ReadRentInstructionRaw::from(ix).into()
    }
}

impl From<ReadRentInstructionRaw> for Instruction {
    fn from(ix: ReadRentInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.snapshot, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![103];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
