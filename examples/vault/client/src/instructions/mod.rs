pub mod deposit;
pub mod withdraw;

pub use deposit::*;
pub use withdraw::*;

pub enum ProgramInstruction {
    Deposit { amount: u64 },
    Withdraw { amount: u64 },
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
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            if offset != payload.len() {
                return None;
            }
            Some(ProgramInstruction::Deposit { amount })
        }
        1 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            if offset != payload.len() {
                return None;
            }
            Some(ProgramInstruction::Withdraw { amount })
        }
        _ => None,
    }
}
