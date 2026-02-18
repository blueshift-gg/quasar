use crate::prelude::*;

/// Raw entrypoint context before parsing.
pub struct Context<'info> {
    pub program_id: &'info [u8; 32],
    pub accounts: &'info [AccountView],
    pub data: &'info [u8],
}

/// Parsed instruction context with typed accounts and PDA bumps.
pub struct Ctx<'info, T: ParseAccounts<'info>> {
    pub accounts: T,
    pub bumps: T::Bumps,
    pub program_id: &'info [u8; 32],
    pub data: &'info [u8],
}

impl<'info, T: ParseAccounts<'info>> Ctx<'info, T> {
    #[inline(always)]
    pub fn new(ctx: Context<'info>) -> Result<Self, ProgramError> {
        let (accounts, bumps) = T::parse(ctx.accounts)?;
        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
        })
    }
}
