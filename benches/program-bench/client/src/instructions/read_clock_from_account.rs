use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct ReadClockFromAccountInstruction {
    pub payer: Address,
    pub snapshot: Address,
}

impl From<ReadClockFromAccountInstruction> for Instruction {
    fn from(ix: ReadClockFromAccountInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.payer, true),
            AccountMeta::new(ix.snapshot, false),
            AccountMeta::new_readonly(solana_address::address!("SysvarC1ock11111111111111111111111111111111"), false),
        ];
        let data = vec![102];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
