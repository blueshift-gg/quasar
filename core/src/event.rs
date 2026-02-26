use crate::cpi::{InstructionAccount, Seed, Signer};
use solana_account_view::AccountView;
use solana_instruction_view::InstructionView;
use solana_program_error::ProgramError;

#[inline(always)]
pub fn emit_event_cpi(
    program: &AccountView,
    event_authority: &AccountView,
    instruction_data: &[u8],
    bump: u8,
) -> Result<(), ProgramError> {
    let instruction_account = InstructionAccount::readonly_signer(event_authority.address());

    let bump_ref = [bump];
    let seeds = [
        Seed::from(b"__event_authority" as &[u8]),
        Seed::from(&bump_ref as &[u8]),
    ];
    let signer = Signer::from(&seeds as &[Seed]);

    let instruction = InstructionView {
        program_id: program.address(),
        accounts: core::slice::from_ref(&instruction_account),
        data: instruction_data,
    };

    solana_instruction_view::cpi::invoke_signed::<1>(&instruction, &[event_authority], &[signer])
}
