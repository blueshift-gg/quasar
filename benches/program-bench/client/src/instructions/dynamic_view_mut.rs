use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;
use quasar_lang::client::{DynString, DynVec};

pub struct DynamicViewMutInstruction {
    pub account: Address,
    pub payer: Address,
    pub new_name: DynString<u8>,
    pub new_tags: DynVec<Address, u16>,
}

impl From<DynamicViewMutInstruction> for Instruction {
    fn from(ix: DynamicViewMutInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.account, false),
            AccountMeta::new(ix.payer, true),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![24];
        data.extend_from_slice(&(ix.new_name.len() as u64).to_le_bytes()[..1]);
        data.extend_from_slice(&(ix.new_tags.len() as u64).to_le_bytes()[..2]);
        data.extend_from_slice(ix.new_name.as_bytes());
        for item in ix.new_tags.iter() {
            wincode::serialize_into(&mut data, item).expect("serialization into Vec<u8> is infallible");
        }
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
