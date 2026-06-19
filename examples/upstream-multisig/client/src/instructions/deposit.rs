use {
    crate::ID,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

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
        wincode::serialize_into(&mut data, &ix.amount).unwrap();
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
