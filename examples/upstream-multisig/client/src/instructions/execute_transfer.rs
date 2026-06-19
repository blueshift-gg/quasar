use {
    crate::ID,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    std::vec::Vec,
};

pub struct ExecuteTransferInstruction {
    pub config: Address,
    pub creator: Address,
    pub vault: Address,
    pub recipient: Address,
    pub system_program: Address,
    pub amount: u64,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl From<ExecuteTransferInstruction> for Instruction {
    fn from(ix: ExecuteTransferInstruction) -> Instruction {
        let mut accounts = vec![
            AccountMeta::new_readonly(ix.config, false),
            AccountMeta::new_readonly(ix.creator, false),
            AccountMeta::new(ix.vault, false),
            AccountMeta::new(ix.recipient, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        accounts.extend(ix.remaining_accounts);
        let mut data = vec![3];
        wincode::serialize_into(&mut data, &ix.amount).unwrap();
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
