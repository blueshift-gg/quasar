pub mod make;
pub mod refund;
pub mod take;

pub use {make::*, refund::*, take::*};

pub enum ProgramInstruction {
    Make { deposit: u64, receive: u64 },
    Take,
    Refund,
}

pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {
    let disc = *data.first()?;
    match disc {
        0 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let deposit: u64 = wincode::deserialize(&payload[offset..]).ok()?;
            offset += wincode::serialized_size(&deposit).ok()? as usize;
            let receive: u64 = wincode::deserialize(&payload[offset..]).ok()?;
            Some(ProgramInstruction::Make { deposit, receive })
        }
        1 => Some(ProgramInstruction::Take),
        2 => Some(ProgramInstruction::Refund),
        _ => None,
    }
}
