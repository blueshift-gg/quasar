use quasar_lang::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PythError {
    InvalidOwner = 100,
    AccountTooSmall = 101,
    InvalidDiscriminator = 102,
    FeedMismatch = 103,
    StalePrice = 104,
}

impl From<PythError> for ProgramError {
    fn from(e: PythError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
