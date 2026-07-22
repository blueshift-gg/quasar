use quasar_lang::client::DynString;
pub mod create;
pub mod deposit;
pub mod execute_transfer;
pub mod set_label;

pub use create::*;
pub use deposit::*;
pub use execute_transfer::*;
pub use set_label::*;

pub enum ProgramInstruction {
    Create { threshold: u8 },
    Deposit { amount: u64 },
    SetLabel { label: DynString<u8> },
    ExecuteTransfer { amount: u64 },
}

fn quasar_take<'a>(data: &'a [u8], offset: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = offset.checked_add(len)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(bytes)
}

fn quasar_read_len(data: &[u8], offset: &mut usize, width: usize) -> Option<usize> {
    let mut buf = [0u8; 8];
    buf.get_mut(..width)?
        .copy_from_slice(quasar_take(data, offset, width)?);
    usize::try_from(u64::from_le_bytes(buf)).ok()
}

pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {
    let disc = *data.first()?;
    match disc {
        0 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let threshold: u8 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let threshold_size =
                usize::try_from(wincode::serialized_size(&threshold).ok()?).ok()?;
            quasar_take(payload, &mut offset, threshold_size)?;
            if offset != payload.len() {
                return None;
            }
            Some(ProgramInstruction::Create { threshold })
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
            Some(ProgramInstruction::Deposit { amount })
        }
        2 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let label_len = quasar_read_len(payload, &mut offset, 1)?;
            let label_bytes = quasar_take(payload, &mut offset, label_len)?;
            let label: DynString<u8> = core::str::from_utf8(label_bytes).ok()?.into();
            if offset != payload.len() {
                return None;
            }
            Some(ProgramInstruction::SetLabel { label })
        }
        3 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            if offset != payload.len() {
                return None;
            }
            Some(ProgramInstruction::ExecuteTransfer { amount })
        }
        _ => None,
    }
}
