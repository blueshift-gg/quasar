use {
    crate::ID,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    std::vec::Vec,
};

pub struct CreateInstruction {
    pub creator: Address,
    pub config: Address,
    pub rent: Address,
    pub system_program: Address,
    pub threshold: u8,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl From<CreateInstruction> for Instruction {
    fn from(ix: CreateInstruction) -> Instruction {
        let mut accounts = vec![
            AccountMeta::new(ix.creator, true),
            AccountMeta::new(ix.config, false),
            AccountMeta::new_readonly(ix.rent, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        accounts.extend(ix.remaining_accounts);
        let mut data = vec![0];
        wincode::serialize_into(&mut data, &ix.threshold).unwrap();
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
