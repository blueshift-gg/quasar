use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct DepositInstruction {
    pub depositor: Address,
    pub config: Address,
    pub vault: Address,
    pub amount: u64,
}

pub struct DepositInstructionInput {
    pub depositor: Address,
    pub config: Address,
    pub amount: u64,
}

impl From<DepositInstructionInput> for DepositInstruction {
    fn from(ix: DepositInstructionInput) -> DepositInstruction {
        let depositor = ix.depositor;
        let config = ix.config;
        let vault = Address::find_program_address(&[b"vault", config.as_ref()], &ID).0;
        DepositInstruction {
            depositor,
            config,
            vault,
            amount: ix.amount,
        }
    }
}

impl From<DepositInstructionInput> for Instruction {
    fn from(ix: DepositInstructionInput) -> Instruction {
        DepositInstruction::from(ix).into()
    }
}

impl From<DepositInstruction> for Instruction {
    fn from(ix: DepositInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.depositor, true),
            AccountMeta::new_readonly(ix.config, false),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![1];
        wincode::serialize_into(&mut data, &ix.amount).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
