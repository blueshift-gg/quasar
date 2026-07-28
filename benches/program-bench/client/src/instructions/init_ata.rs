use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct InitAtaInstructionRaw {
    pub payer: Address,
    pub ata: Address,
    pub wallet: Address,
    pub mint: Address,
}

pub struct InitAtaInstruction {
    pub payer: Address,
    pub wallet: Address,
    pub mint: Address,
}

impl From<InitAtaInstruction> for InitAtaInstructionRaw {
    fn from(ix: InitAtaInstruction) -> InitAtaInstructionRaw {
        let payer = ix.payer;
        let wallet = ix.wallet;
        let mint = ix.mint;
        let token_program = solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let ata = Address::find_program_address(&[wallet.as_ref(), token_program.as_ref(), mint.as_ref()], &solana_address::address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")).0;
        InitAtaInstructionRaw {
            payer,
            ata,
            wallet,
            mint,
        }
    }
}

impl From<InitAtaInstruction> for Instruction {
    fn from(ix: InitAtaInstruction) -> Instruction {
        InitAtaInstructionRaw::from(ix).into()
    }
}

impl From<InitAtaInstructionRaw> for Instruction {
    fn from(ix: InitAtaInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.ata, false),
            AccountMeta::new_readonly(ix.wallet, true),
            AccountMeta::new_readonly(ix.mint, false),
            AccountMeta::new_readonly(solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
            AccountMeta::new_readonly(solana_address::address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"), false),
        ];
        let data = vec![42];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
