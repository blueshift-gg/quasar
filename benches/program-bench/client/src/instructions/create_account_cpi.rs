use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct CreateAccountCpiInstruction {
    pub payer: Address,
    pub new_account: Address,
    pub lamports: u64,
    pub space: u64,
    pub owner: Address,
}

impl From<CreateAccountCpiInstruction> for Instruction {
    fn from(ix: CreateAccountCpiInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.payer, true),
            AccountMeta::new(ix.new_account, true),
            AccountMeta::new_readonly(solana_address::address!("11111111111111111111111111111111"), false),
        ];
        let mut data = vec![28];
        wincode::serialize_into(&mut data, &ix.lamports).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.space).expect("serialization into Vec<u8> is infallible");
        wincode::serialize_into(&mut data, &ix.owner).expect("serialization into Vec<u8> is infallible");
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
