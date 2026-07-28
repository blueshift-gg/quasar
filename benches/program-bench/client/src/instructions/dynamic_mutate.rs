use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;
use quasar_lang::client::{DynString};

pub struct DynamicMutateInstruction {
    pub account: Address,
    pub payer: Address,
    pub new_name: DynString<u8>,
}

impl From<DynamicMutateInstruction> for Instruction {
    fn from(ix: DynamicMutateInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.account, false),
            AccountMeta::new(ix.payer, true),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![23];
        data.extend_from_slice(&(ix.new_name.len() as u64).to_le_bytes()[..1]);
        data.extend_from_slice(ix.new_name.as_bytes());
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
