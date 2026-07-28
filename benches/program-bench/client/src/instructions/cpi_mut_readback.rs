use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CpiMutReadbackInstruction {
    pub account: Address,
    pub payer: Address,
    pub new_value: u64,
}

impl From<CpiMutReadbackInstruction> for Instruction {
    fn from(ix: CpiMutReadbackInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.account, false),
            AccountMeta::new(ix.payer, true),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![108];
        wincode::serialize_into(&mut data, &ix.new_value).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
