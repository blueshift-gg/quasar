use solana_instruction::{AccountMeta, Instruction};
use crate::ID;
use solana_address::Address;

pub struct VerifyPdaStaticInstructionRaw {
    pub account: Address,
}

pub struct VerifyPdaStaticInstruction {
}

impl From<VerifyPdaStaticInstruction> for VerifyPdaStaticInstructionRaw {
    fn from(ix: VerifyPdaStaticInstruction) -> VerifyPdaStaticInstructionRaw {
        let _ = ix;
        let account = Address::find_program_address(&[b"pda"], &ID).0;
        VerifyPdaStaticInstructionRaw {
            account,
        }
    }
}

impl From<VerifyPdaStaticInstruction> for Instruction {
    fn from(ix: VerifyPdaStaticInstruction) -> Instruction {
        VerifyPdaStaticInstructionRaw::from(ix).into()
    }
}

impl From<VerifyPdaStaticInstructionRaw> for Instruction {
    fn from(ix: VerifyPdaStaticInstructionRaw) -> Instruction {
        let accounts = vec![
            AccountMeta::new_readonly(ix.account, false),
        ];
        let data = vec![7];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
