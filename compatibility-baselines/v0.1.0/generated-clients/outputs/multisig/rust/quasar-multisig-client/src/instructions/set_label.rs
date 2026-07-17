use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;
use quasar_lang::client::{DynString};

pub struct SetLabelInstruction {
    pub creator: Address,
    pub config: Address,
    pub label: DynString<u8>,
}

pub struct SetLabelInstructionInput {
    pub creator: Address,
    pub label: DynString<u8>,
}

impl From<SetLabelInstructionInput> for SetLabelInstruction {
    fn from(ix: SetLabelInstructionInput) -> SetLabelInstruction {
        let creator = ix.creator;
        let config = Address::find_program_address(&[b"multisig", creator.as_ref()], &ID).0;
        SetLabelInstruction {
            creator,
            config,
            label: ix.label,
        }
    }
}

impl From<SetLabelInstructionInput> for Instruction {
    fn from(ix: SetLabelInstructionInput) -> Instruction {
        SetLabelInstruction::from(ix).into()
    }
}

impl From<SetLabelInstruction> for Instruction {
    fn from(ix: SetLabelInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.creator, true),
            AccountMeta::new(ix.config, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![2];
        data.extend_from_slice(&(ix.label.len() as u64).to_le_bytes()[..1]);
        data.extend_from_slice(ix.label.as_bytes());
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
