use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct SpaceOverrideInstructionRaw {
    pub payer: Address,
    pub account: Address,
    pub value: u64,
}

pub struct SpaceOverrideInstruction {
    pub payer: Address,
    pub value: u64,
}

impl From<SpaceOverrideInstruction> for SpaceOverrideInstructionRaw {
    fn from(ix: SpaceOverrideInstruction) -> SpaceOverrideInstructionRaw {
        let payer = ix.payer;
        let account = Address::find_program_address(&[b"spacetest", payer.as_ref()], &ID).0;
        SpaceOverrideInstructionRaw {
            payer,
            account,
            value: ix.value,
        }
    }
}

impl From<SpaceOverrideInstruction> for Instruction {
    fn from(ix: SpaceOverrideInstruction) -> Instruction {
        SpaceOverrideInstructionRaw::from(ix).into()
    }
}

impl From<SpaceOverrideInstructionRaw> for Instruction {
    fn from(ix: SpaceOverrideInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.account, false),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![33];
        wincode::serialize_into(&mut data, &ix.value).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
