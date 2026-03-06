use alloc::vec;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

pub struct DepositInstruction {
    pub user: Address,
    pub vault: Address,
    pub system_program: Address,
    pub amount: u64,
}

impl From<DepositInstruction> for Instruction {
    fn from(ix: DepositInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.user, true),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let mut data = vec![0];
        data.extend_from_slice(&ix.amount.to_le_bytes());
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}

pub struct WithdrawInstruction {
    pub user: Address,
    pub vault: Address,
    pub amount: u64,
}

impl From<WithdrawInstruction> for Instruction {
    fn from(ix: WithdrawInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.user, true),
            AccountMeta::new(ix.vault, false),
        ];
        let mut data = vec![1];
        data.extend_from_slice(&ix.amount.to_le_bytes());
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}
