use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct AccountsInstruction {
    pub account1: Address,
    pub account2: Address,
    pub account3: Address,
    pub account4: Address,
    pub account5: Address,
    pub account6: Address,
    pub account7: Address,
    pub account8: Address,
    pub account9: Address,
    pub account10: Address,
}

impl From<AccountsInstruction> for Instruction {
    fn from(ix: AccountsInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.account1, false),
            AccountMeta::new_readonly(ix.account2, false),
            AccountMeta::new_readonly(ix.account3, false),
            AccountMeta::new_readonly(ix.account4, false),
            AccountMeta::new_readonly(ix.account5, false),
            AccountMeta::new_readonly(ix.account6, false),
            AccountMeta::new_readonly(ix.account7, false),
            AccountMeta::new_readonly(ix.account8, false),
            AccountMeta::new_readonly(ix.account9, false),
            AccountMeta::new_readonly(ix.account10, false),
        ];
        let data = vec![5];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
