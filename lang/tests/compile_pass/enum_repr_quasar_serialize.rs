use quasar_lang::prelude::*;

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, QuasarSerialize)]
pub enum Status {
    Pending = 1,
    Ready = 2,
    Failed = 9,
}

#[derive(Accounts)]
pub struct NoAccounts {}

#[instruction(discriminator = 1)]
pub fn enum_round_trip(_ctx: Ctx<NoAccounts>, status: Status) -> Result<(), ProgramError> {
    match status {
        Status::Pending | Status::Ready | Status::Failed => Ok(()),
    }
}

fn main() {}
