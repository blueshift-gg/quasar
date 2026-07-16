use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

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
}

pub struct TakeInstructionInput {
    pub taker: Address,
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub taker_ta_a: Address,
    pub taker_ta_b: Address,
    pub maker_ta_b: Address,
    pub vault_ta_a: Address,
}

impl From<TakeInstructionInput> for TakeInstruction {
    fn from(ix: TakeInstructionInput) -> TakeInstruction {
        let taker = ix.taker;
        let maker = ix.maker;
        let mint_a = ix.mint_a;
        let mint_b = ix.mint_b;
        let taker_ta_a = ix.taker_ta_a;
        let taker_ta_b = ix.taker_ta_b;
        let maker_ta_b = ix.maker_ta_b;
        let vault_ta_a = ix.vault_ta_a;
        let escrow = Address::find_program_address(&[b"escrow", maker.as_ref()], &ID).0;
        TakeInstruction {
            taker,
            escrow,
            maker,
            mint_a,
            mint_b,
            taker_ta_a,
            taker_ta_b,
            maker_ta_b,
            vault_ta_a,
        }
    }
}

impl From<TakeInstructionInput> for Instruction {
    fn from(ix: TakeInstructionInput) -> Instruction {
        TakeInstruction::from(ix).into()
    }
}

impl From<TakeInstruction> for Instruction {
    fn from(ix: TakeInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.taker, true),
            AccountMeta::new(ix.escrow, false),
            AccountMeta::new(ix.maker, false),
            AccountMeta::new_readonly(ix.mint_a, false),
            AccountMeta::new_readonly(ix.mint_b, false),
            AccountMeta::new(ix.taker_ta_a, true),
            AccountMeta::new(ix.taker_ta_b, false),
            AccountMeta::new(ix.maker_ta_b, true),
            AccountMeta::new(ix.vault_ta_a, false),
            AccountMeta::new_readonly(solana_address::address!("SysvarRent111111111111111111111111111111111"), false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![1];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
