use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct RefundInstruction {
    pub maker: Address,
    pub escrow: Address,
    pub mint_a: Address,
    pub maker_ta_a: Address,
    pub vault_ta_a: Address,
}

pub struct RefundInstructionInput {
    pub maker: Address,
    pub mint_a: Address,
    pub maker_ta_a: Address,
    pub vault_ta_a: Address,
}

impl From<RefundInstructionInput> for RefundInstruction {
    fn from(ix: RefundInstructionInput) -> RefundInstruction {
        let maker = ix.maker;
        let mint_a = ix.mint_a;
        let maker_ta_a = ix.maker_ta_a;
        let vault_ta_a = ix.vault_ta_a;
        let escrow = Address::find_program_address(&[b"escrow", maker.as_ref()], &ID).0;
        RefundInstruction {
            maker,
            escrow,
            mint_a,
            maker_ta_a,
            vault_ta_a,
        }
    }
}

impl From<RefundInstructionInput> for Instruction {
    fn from(ix: RefundInstructionInput) -> Instruction {
        RefundInstruction::from(ix).into()
    }
}

impl From<RefundInstruction> for Instruction {
    fn from(ix: RefundInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.maker, true),
            AccountMeta::new(ix.escrow, false),
            AccountMeta::new_readonly(ix.mint_a, false),
            AccountMeta::new(ix.maker_ta_a, true),
            AccountMeta::new(ix.vault_ta_a, false),
            AccountMeta::new_readonly(solana_address::address!("SysvarRent111111111111111111111111111111111"), false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let data = vec![2];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
