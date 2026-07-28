use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;
use quasar_lang::client::{DynString};

pub struct TwoDynInstruction {
    pub account: Address,
    pub tag: u64,
    pub a: DynString<u8>,
    pub b: DynString<u8>,
}

impl From<TwoDynInstruction> for Instruction {
    fn from(ix: TwoDynInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.account, false),
        ];
        let mut data = vec![25];
        wincode::serialize_into(&mut data, &ix.tag).expect("serialization into Vec<u8> is infallible");
        data.extend_from_slice(&(ix.a.len() as u64).to_le_bytes()[..1]);
        data.extend_from_slice(&(ix.b.len() as u64).to_le_bytes()[..1]);
        data.extend_from_slice(ix.a.as_bytes());
        data.extend_from_slice(ix.b.as_bytes());
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
