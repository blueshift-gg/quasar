use solana_instruction::Instruction;
use crate::ID;

pub struct PingInstruction {
}

impl From<PingInstruction> for Instruction {
    fn from(ix: PingInstruction) -> Instruction {
        let _ = ix;
        let accounts = vec![
        ];
        let data = vec![0];
        Instruction {
            program_id: ID,
            accounts,
            data,
        }
    }
}
