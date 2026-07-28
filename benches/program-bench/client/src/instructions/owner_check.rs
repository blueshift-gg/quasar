use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct OwnerCheckInstruction {
    pub account: Address,
}

impl From<OwnerCheckInstruction> for Instruction {
    fn from(ix: OwnerCheckInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.account, false),
        ];
        let data = vec![88];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
