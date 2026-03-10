//! Self-CPI event emission for spoofing-resistant on-chain events.
//!
//! - **Log-based** (`emit!`) — ~100 CU, fast but spoofable.
//! - **Self-CPI** (`emit_cpi!`) — ~1,000 CU, unforgeable (program ID in trace).

use crate::cpi::{cpi_account_from_view, invoke_raw, InstructionAccount, Seed, Signer};
use solana_account_view::AccountView;
use solana_program_error::ProgramError;

#[inline(always)]
pub fn emit_event_cpi(
    program: &AccountView,
    event_authority: &AccountView,
    instruction_data: &[u8],
    bump: u8,
) -> Result<(), ProgramError> {
    let instruction_account = InstructionAccount::readonly_signer(event_authority.address());
    let cpi_account = cpi_account_from_view(event_authority);

    let bump_ref = [bump];
    let seeds = [
        Seed::from(b"__event_authority" as &[u8]),
        Seed::from(&bump_ref as &[u8]),
    ];
    let signer = Signer::from(&seeds as &[Seed]);

    let result = unsafe {
        invoke_raw(
            program.address(),
            &instruction_account as *const InstructionAccount,
            1,
            instruction_data.as_ptr(),
            instruction_data.len(),
            &cpi_account as *const _,
            1,
            &[signer],
        )
    };

    if result == 0 {
        Ok(())
    } else {
        Err(ProgramError::from(result))
    }
}
