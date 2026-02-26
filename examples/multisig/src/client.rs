use alloc::vec;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

pub struct CreateInstruction {
    pub creator: Address,
    pub config: Address,
    pub rent: Address,
    pub system_program: Address,
    pub threshold: u8,
}

impl From<CreateInstruction> for Instruction {
    fn from(ix: CreateInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.creator, true),
            AccountMeta::new(ix.config, false),
            AccountMeta::new_readonly(ix.rent, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let mut data = vec![0];
        data.push(ix.threshold as u8);
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}

pub struct DepositInstruction {
    pub depositor: Address,
    pub config: Address,
    pub vault: Address,
    pub system_program: Address,
    pub amount: u64,
}

impl From<DepositInstruction> for Instruction {
    fn from(ix: DepositInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.depositor, true),
            AccountMeta::new_readonly(ix.config, false),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new_readonly(ix.system_program, false),
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

pub struct SetLabelInstruction<'a> {
    pub creator: Address,
    pub config: Address,
    pub system_program: Address,
    pub label: &'a [u8],
}

impl<'a> From<SetLabelInstruction<'a>> for Instruction {
    fn from(ix: SetLabelInstruction<'a>) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.creator, true),
            AccountMeta::new(ix.config, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let mut data = vec![2];
        data.extend_from_slice(&(ix.label.len() as u16).to_le_bytes());
        data.extend_from_slice(ix.label);
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}

pub struct ExecuteTransferInstruction {
    pub config: Address,
    pub creator: Address,
    pub vault: Address,
    pub recipient: Address,
    pub system_program: Address,
    pub amount: u64,
}

impl From<ExecuteTransferInstruction> for Instruction {
    fn from(ix: ExecuteTransferInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.config, false),
            AccountMeta::new_readonly(ix.creator, false),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new(ix.recipient, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let mut data = vec![3];
        data.extend_from_slice(&ix.amount.to_le_bytes());
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}
