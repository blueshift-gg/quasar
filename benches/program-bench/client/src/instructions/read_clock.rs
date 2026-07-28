use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct ReadClockInstructionRaw {
    pub payer: Address,
    pub snapshot: Address,
}

pub struct ReadClockInstruction {
    pub payer: Address,
}

impl From<ReadClockInstruction> for ReadClockInstructionRaw {
    fn from(ix: ReadClockInstruction) -> ReadClockInstructionRaw {
        let payer = ix.payer;
        let snapshot = Address::find_program_address(&[b"clock"], &ID).0;
        ReadClockInstructionRaw {
            payer,
            snapshot,
        }
    }
}

impl From<ReadClockInstruction> for Instruction {
    fn from(ix: ReadClockInstruction) -> Instruction {
        ReadClockInstructionRaw::from(ix).into()
    }
}

impl From<ReadClockInstructionRaw> for Instruction {
    fn from(ix: ReadClockInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.snapshot, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![101];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
