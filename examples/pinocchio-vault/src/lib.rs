#![no_std]

use pinocchio::{no_allocator, program_entrypoint, AccountView, Address, ProgramResult};

mod constants;
mod errors;
mod instructions;
mod utils;

#[cfg(test)]
mod tests;

use utils::Context;
pub use {
    constants::{ID, ID_BYTES},
    errors::VaultError,
};

program_entrypoint!(process_instruction);
no_allocator!();

#[cfg(all(not(test), target_os = "solana"))]
pinocchio::nostd_panic_handler!();

fn process_instruction(
    _program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let (discriminator, data) = instruction_data
        .split_first()
        .ok_or(VaultError::InvalidInstructionData)?;

    let ctx = Context { accounts, data };

    match *discriminator {
        0 => instructions::Deposit::process(ctx),
        1 => instructions::Withdraw::process(ctx),
        _ => Err(VaultError::InvalidInstructionData.into()),
    }
}
