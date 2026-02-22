use crate::prelude::*;
use crate::remaining::RemainingAccounts;

/// Raw entrypoint context before parsing.
pub struct Context<'info> {
    pub program_id: &'info [u8; 32],
    pub accounts: &'info [AccountView],
    pub remaining_ptr: *mut u8,
    pub data: &'info [u8],
    /// Boundary pointer marking end of accounts region in the SVM buffer.
    /// Computed from the original instruction data pointer (before discriminator
    /// stripping) as `ix_data_ptr - sizeof(u64)`.
    pub accounts_boundary: *const u8,
}

/// Parsed instruction context with typed accounts and PDA bumps.
pub struct Ctx<'info, T: ParseAccounts<'info> + AccountCount> {
    pub accounts: T,
    pub bumps: T::Bumps,
    pub program_id: &'info [u8; 32],
    pub data: &'info [u8],
    remaining_ptr: *mut u8,
    declared: &'info [AccountView],
    accounts_boundary: *const u8,
}

impl<'info, T: ParseAccounts<'info> + AccountCount> Ctx<'info, T> {
    #[inline(always)]
    pub fn new(ctx: Context<'info>) -> Result<Self, ProgramError> {
        let (accounts, bumps) = T::parse(ctx.accounts)?;
        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
            remaining_ptr: ctx.remaining_ptr,
            declared: ctx.accounts,
            accounts_boundary: ctx.accounts_boundary,
        })
    }

    /// Access remaining accounts. Zero cost until called.
    #[inline(always)]
    pub fn remaining_accounts(&self) -> RemainingAccounts<'info> {
        RemainingAccounts::new(self.remaining_ptr, self.accounts_boundary, self.declared)
    }
}
