use crate::ID;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

pub struct WithdrawInstructionRaw {
    pub user: Address,
    pub vault: Address,
    pub amount: u64,
}

pub struct WithdrawInstruction {
    pub user: Address,
    pub amount: u64,
}

impl From<WithdrawInstruction> for WithdrawInstructionRaw {
    fn from(ix: WithdrawInstruction) -> WithdrawInstructionRaw {
        let user = ix.user;
        let vault = Address::find_program_address(&[b"vault", user.as_ref()], &ID).0;
        WithdrawInstructionRaw {
            user,
            vault,
            amount: ix.amount,
        }
    }
}

impl From<WithdrawInstruction> for Instruction {
    fn from(ix: WithdrawInstruction) -> Instruction {
        WithdrawInstructionRaw::from(ix).into()
    }
}

impl From<WithdrawInstructionRaw> for Instruction {
    fn from(ix: WithdrawInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.user, true),
            AccountMeta::new(ix.vault, false),
        ];
        let mut data = vec![1];
        wincode::serialize_into(&mut data, &ix.amount)
            .expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
