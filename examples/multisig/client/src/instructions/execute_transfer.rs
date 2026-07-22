use crate::ID;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use std::vec::Vec;

pub struct ExecuteTransferInstructionRaw {
    pub config: Address,
    pub creator: Address,
    pub vault: Address,
    pub recipient: Address,
    pub amount: u64,
    pub remaining_accounts: Vec<AccountMeta>,
}

pub struct ExecuteTransferInstruction {
    pub creator: Address,
    pub recipient: Address,
    pub amount: u64,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl From<ExecuteTransferInstruction> for ExecuteTransferInstructionRaw {
    fn from(ix: ExecuteTransferInstruction) -> ExecuteTransferInstructionRaw {
        let creator = ix.creator;
        let recipient = ix.recipient;
        let config = Address::find_program_address(&[b"multisig", creator.as_ref()], &ID).0;
        let vault = Address::find_program_address(&[b"vault", config.as_ref()], &ID).0;
        ExecuteTransferInstructionRaw {
            config,
            creator,
            vault,
            recipient,
            amount: ix.amount,
            remaining_accounts: ix.remaining_accounts,
        }
    }
}

impl From<ExecuteTransferInstruction> for Instruction {
    fn from(ix: ExecuteTransferInstruction) -> Instruction {
        ExecuteTransferInstructionRaw::from(ix).into()
    }
}

impl From<ExecuteTransferInstructionRaw> for Instruction {
    fn from(ix: ExecuteTransferInstructionRaw) -> Instruction {
        let mut accounts = vec![
            AccountMeta::new_readonly(ix.config, false),
            AccountMeta::new_readonly(ix.creator, false),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new(ix.recipient, false),
            AccountMeta::new_readonly(
                solana_address::address!("11111111111111111111111111111111"),
                false,
            ),
        ];
        accounts.extend(ix.remaining_accounts);
        let mut data = vec![3];
        wincode::serialize_into(&mut data, &ix.amount)
            .expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
