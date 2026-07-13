use std::vec::Vec;
use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CreateInstruction {
    pub creator: Address,
    pub config: Address,
    pub threshold: u8,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl From<CreateInstruction> for Instruction {
    fn from(ix: CreateInstruction) -> Instruction {
        let mut accounts = vec![
            AccountMeta::new(ix.creator, true),
            AccountMeta::new(ix.config, false),
            AccountMeta::new_readonly(solana_address::address!("SysvarRent111111111111111111111111111111111"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        accounts.extend(ix.remaining_accounts);
        let mut data = vec![0];
        wincode::serialize_into(&mut data, &ix.threshold).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
