use std::vec::Vec;
use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct ExecuteTransferInstruction {
    pub config: Address,
    pub creator: Address,
    pub vault: Address,
    pub recipient: Address,
    pub amount: u64,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl From<ExecuteTransferInstruction> for Instruction {
    fn from(ix: ExecuteTransferInstruction) -> Instruction {
        let mut accounts = vec![
            AccountMeta::new_readonly(ix.config, false),
            AccountMeta::new_readonly(ix.creator, false),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new(ix.recipient, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        accounts.extend(ix.remaining_accounts);
        let mut data = vec![3];
        wincode::serialize_into(&mut data, &ix.amount).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
