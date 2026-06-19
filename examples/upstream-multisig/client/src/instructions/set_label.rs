use {
    crate::ID,
    quasar_lang::client::DynBytes,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

pub struct SetLabelInstruction {
    pub creator: Address,
    pub config: Address,
    pub system_program: Address,
    pub label: DynBytes<u8>,
}

impl From<SetLabelInstruction> for Instruction {
    fn from(ix: SetLabelInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.creator, true),
            AccountMeta::new(ix.config, false),
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let mut data = vec![2];
        wincode::serialize_into(&mut data, &ix.label).unwrap();
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
