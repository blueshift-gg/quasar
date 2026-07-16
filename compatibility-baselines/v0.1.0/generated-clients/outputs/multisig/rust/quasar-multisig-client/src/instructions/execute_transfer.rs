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

pub struct ExecuteTransferInstructionInput {
    pub creator: Address,
    pub recipient: Address,
    pub amount: u64,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl From<ExecuteTransferInstructionInput> for ExecuteTransferInstruction {
    fn from(ix: ExecuteTransferInstructionInput) -> ExecuteTransferInstruction {
        let creator = ix.creator;
        let recipient = ix.recipient;
        let config = Address::find_program_address(&[b"multisig", creator.as_ref()], &ID).0;
        let vault = Address::find_program_address(&[b"vault", config.as_ref()], &ID).0;
        ExecuteTransferInstruction {
            config,
            creator,
            vault,
            recipient,
            amount: ix.amount,
            remaining_accounts: ix.remaining_accounts,
        }
    }
}

impl From<ExecuteTransferInstructionInput> for Instruction {
    fn from(ix: ExecuteTransferInstructionInput) -> Instruction {
        ExecuteTransferInstruction::from(ix).into()
    }
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
