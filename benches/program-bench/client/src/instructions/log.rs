use solana_instruction::Instruction;
use crate::ID;

pub struct LogInstruction {
}

impl From<LogInstruction> for Instruction {
    fn from(ix: LogInstruction) -> Instruction {
        let _ = ix;
        let accounts = vec![
        ];
        let data = vec![1];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
