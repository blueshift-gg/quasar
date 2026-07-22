use crate::ID;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

pub struct DepositInstructionRaw {
    pub user: Address,
    pub vault: Address,
    pub amount: u64,
}

pub struct DepositInstruction {
    pub user: Address,
    pub amount: u64,
}

impl From<DepositInstruction> for DepositInstructionRaw {
    fn from(ix: DepositInstruction) -> DepositInstructionRaw {
        let user = ix.user;
        let vault = Address::find_program_address(&[b"vault", user.as_ref()], &ID).0;
        DepositInstructionRaw {
            user,
            vault,
            amount: ix.amount,
        }
    }
}

impl From<DepositInstruction> for Instruction {
    fn from(ix: DepositInstruction) -> Instruction {
        DepositInstructionRaw::from(ix).into()
    }
}

impl From<DepositInstructionRaw> for Instruction {
    fn from(ix: DepositInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.user, true),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new_readonly(
                solana_address::address!("11111111111111111111111111111111"),
                false,
            ),
        ];
        let mut data = vec![0];
        wincode::serialize_into(&mut data, &ix.amount)
            .expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
