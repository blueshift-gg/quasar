use {
    crate::ID,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

pub struct CheckPriceInstruction {
    pub user: Address,
    pub price_feed: Address,
    pub clock: Address,
}

impl From<CheckPriceInstruction> for Instruction {
    fn from(ix: CheckPriceInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.user, true),
            AccountMeta::new_readonly(ix.price_feed, false),
            AccountMeta::new_readonly(ix.clock, false),
        ];
        Instruction {
            program_id: ID,
            accounts,
            data: vec![0],
        }
    }
}
