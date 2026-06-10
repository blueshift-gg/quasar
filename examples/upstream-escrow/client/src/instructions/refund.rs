use {
    crate::ID,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

pub struct RefundInstruction {
    pub maker: Address,
    pub escrow: Address,
    pub mint_a: Address,
    pub maker_ta_a: Address,
    pub vault_ta_a: Address,
    pub rent: Address,
    pub token_program: Address,
    pub system_program: Address,
}

impl From<RefundInstruction> for Instruction {
    fn from(ix: RefundInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.maker, true),
            AccountMeta::new(ix.escrow, false),
            AccountMeta::new_readonly(ix.mint_a, false),
            AccountMeta::new(ix.maker_ta_a, false),
            AccountMeta::new(ix.vault_ta_a, false),
            AccountMeta::new_readonly(ix.rent, false),
            AccountMeta::new_readonly(ix.token_program, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let data = vec![2];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
