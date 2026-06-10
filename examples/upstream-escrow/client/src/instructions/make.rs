use {
    crate::ID,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

pub struct MakeInstruction {
    pub maker: Address,
    pub escrow: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_a: Address,
    pub maker_ta_b: Address,
    pub vault_ta_a: Address,
    pub rent: Address,
    pub token_program: Address,
    pub system_program: Address,
    pub deposit: u64,
    pub receive: u64,
}

impl From<MakeInstruction> for Instruction {
    fn from(ix: MakeInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.maker, true),
            AccountMeta::new(ix.escrow, false),
            AccountMeta::new_readonly(ix.mint_a, false),
            AccountMeta::new_readonly(ix.mint_b, false),
            AccountMeta::new(ix.maker_ta_a, false),
            AccountMeta::new(ix.maker_ta_b, false),
            AccountMeta::new(ix.vault_ta_a, false),
            AccountMeta::new_readonly(ix.rent, false),
            AccountMeta::new_readonly(ix.token_program, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let mut data = vec![0];
        wincode::serialize_into(&mut data, &ix.deposit).unwrap();
        wincode::serialize_into(&mut data, &ix.receive).unwrap();
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
