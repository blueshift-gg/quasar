use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct ReadRentCalcInstructionRaw {
    pub payer: Address,
    pub snapshot: Address,
    pub data_len: u64,
}

pub struct ReadRentCalcInstruction {
    pub payer: Address,
    pub data_len: u64,
}

impl From<ReadRentCalcInstruction> for ReadRentCalcInstructionRaw {
    fn from(ix: ReadRentCalcInstruction) -> ReadRentCalcInstructionRaw {
        let payer = ix.payer;
        let snapshot = Address::find_program_address(&[b"rent_calc"], &ID).0;
        ReadRentCalcInstructionRaw {
            payer,
            snapshot,
            data_len: ix.data_len,
        }
    }
}

impl From<ReadRentCalcInstruction> for Instruction {
    fn from(ix: ReadRentCalcInstruction) -> Instruction {
        ReadRentCalcInstructionRaw::from(ix).into()
    }
}

impl From<ReadRentCalcInstructionRaw> for Instruction {
    fn from(ix: ReadRentCalcInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.snapshot, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![104];
        wincode::serialize_into(&mut data, &ix.data_len).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
