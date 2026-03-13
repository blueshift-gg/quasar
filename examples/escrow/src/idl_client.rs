use {
    alloc::vec,
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
        data.extend_from_slice(&ix.deposit.to_le_bytes());
        data.extend_from_slice(&ix.receive.to_le_bytes());
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}

pub struct TakeInstruction {
    pub taker: Address,
    pub escrow: Address,
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub taker_ta_a: Address,
    pub taker_ta_b: Address,
    pub maker_ta_b: Address,
    pub vault_ta_a: Address,
    pub rent: Address,
    pub token_program: Address,
    pub system_program: Address,
}

impl From<TakeInstruction> for Instruction {
    fn from(ix: TakeInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.taker, true),
            AccountMeta::new(ix.escrow, false),
            AccountMeta::new(ix.maker, false),
            AccountMeta::new_readonly(ix.mint_a, false),
            AccountMeta::new_readonly(ix.mint_b, false),
            AccountMeta::new(ix.taker_ta_a, false),
            AccountMeta::new(ix.taker_ta_b, false),
            AccountMeta::new(ix.maker_ta_b, false),
            AccountMeta::new(ix.vault_ta_a, false),
            AccountMeta::new_readonly(ix.rent, false),
            AccountMeta::new_readonly(ix.token_program, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let data = vec![1];
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}

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
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}
