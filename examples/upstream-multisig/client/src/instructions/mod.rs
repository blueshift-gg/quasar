use quasar_lang::client::DynBytes;
pub mod create;
pub mod deposit;
pub mod execute_transfer;
pub mod set_label;

pub use {create::*, deposit::*, execute_transfer::*, set_label::*};

pub enum ProgramInstruction {
    Create { threshold: u8 },
    Deposit { amount: u64 },
    SetLabel { label: DynBytes<u8> },
    ExecuteTransfer { amount: u64 },
}

pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {
    let disc = *data.first()?;
    match disc {
        0 => {
            let payload = &data[1..];
            let threshold: u8 = wincode::deserialize(payload).ok()?;
            Some(ProgramInstruction::Create { threshold })
        }
        1 => {
            let payload = &data[1..];
            let amount: u64 = wincode::deserialize(payload).ok()?;
            Some(ProgramInstruction::Deposit { amount })
        }
        2 => {
            let payload = &data[1..];
            let label: DynBytes<u8> = wincode::deserialize(payload).ok()?;
            Some(ProgramInstruction::SetLabel { label })
        }
        3 => {
            let payload = &data[1..];
            let amount: u64 = wincode::deserialize(payload).ok()?;
            Some(ProgramInstruction::ExecuteTransfer { amount })
        }
        _ => None,
    }
}
