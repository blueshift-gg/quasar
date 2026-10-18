//! Current Quasar's vault written directly against Pinocchio, with the same
//! checks: the floor the benchmark measures framework overhead against.
#![no_std]

use pinocchio::{
    AccountView, Address, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    no_allocator, nostd_panic_handler, program_entrypoint,
};
use pinocchio_system::instructions::Transfer;

const ID: Address = Address::from_str_const("33333333333333333333333333333333333333333333");
const SYSTEM: Address = Address::new_from_array([0; 32]);

program_entrypoint!(process);
no_allocator!();
nostd_panic_handler!();

fn process(_: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let (&discriminator, amount) = data.split_first().ok_or(ProgramError::InvalidInstructionData)?;
    let amount: [u8; 8] = amount.try_into().map_err(|_| ProgramError::InvalidInstructionData)?;
    let amount = u64::from_le_bytes(amount);
    let [user, vault, system, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !user.is_writable() || !vault.is_writable() {
        return Err(ProgramError::Immutable);
    }
    if !system.executable() || system.address() != &SYSTEM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let (address, bump) = Address::find_program_address(&[b"vault", user.address().as_ref()], &ID);
    if vault.address() != &address {
        return Err(ProgramError::InvalidSeeds);
    }
    match discriminator {
        0 => Transfer { from: user, to: vault, lamports: amount }.invoke(),
        1 => {
            let bump = [bump];
            let seeds = [Seed::from(b"vault"), Seed::from(user.address().as_ref()), Seed::from(&bump)];
            Transfer { from: vault, to: user, lamports: amount }.invoke_signed(&[Signer::from(&seeds)])
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
