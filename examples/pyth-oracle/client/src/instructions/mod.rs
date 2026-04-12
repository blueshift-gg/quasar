pub mod check_price;

pub use check_price::*;

pub enum ProgramInstruction {
    CheckPrice,
}

pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {
    let disc = *data.first()?;
    match disc {
        0 => Some(ProgramInstruction::CheckPrice),
        _ => None,
    }
}
