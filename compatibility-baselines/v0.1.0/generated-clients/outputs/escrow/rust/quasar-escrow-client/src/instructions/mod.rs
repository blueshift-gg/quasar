pub mod make;
pub mod take;
pub mod refund;

pub use make::*;
pub use take::*;
pub use refund::*;

pub enum ProgramInstruction {
    Make { deposit: u64, receive: u64 },
    Take,
    Refund,
}

fn quasar_take<'a>(data: &'a [u8], offset: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = offset.checked_add(len)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(bytes)
}

pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {
    let disc = *data.first()?;
    match disc {
        0 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let deposit: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let deposit_size = usize::try_from(wincode::serialized_size(&deposit).ok()?).ok()?;
            quasar_take(payload, &mut offset, deposit_size)?;
            let receive: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let receive_size = usize::try_from(wincode::serialized_size(&receive).ok()?).ok()?;
            quasar_take(payload, &mut offset, receive_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::Make { deposit, receive })
        }
        1 => if data.len() == 1 { Some(ProgramInstruction::Take) } else { None },
        2 => if data.len() == 1 { Some(ProgramInstruction::Refund) } else { None },
        _ => None,
    }
}
