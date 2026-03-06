//! Self-CPI event emission for spoofing-resistant on-chain events.
//!
//! Quasar events support two emission strategies:
//!
//! - **Log-based** (`emit!()` / `sol_log_data`) — ~100 CU. Fast but any program
//!   can emit arbitrary log data, so indexers cannot prove which program produced
//!   a given log entry.
//!
//! - **Self-CPI** (`emit_cpi!()`) — ~1,000 CU. The program invokes itself with
//!   the event data, signing via the `__event_authority` PDA. Indexers can verify
//!   the invoking program ID in the instruction trace, making the event unforgeable.
//!
//! This module provides `emit_event_cpi`, the low-level self-CPI helper used by
//! the `emit_cpi!()` macro. Most programs should use the macro directly.

use crate::cpi::{invoke_raw, InstructionAccount, RawCpiAccount, Seed, Signer};
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
    let cpi_account = RawCpiAccount::from_view(event_authority);

    let bump_ref = [bump];
    let seeds = [
        Seed::from(b"__event_authority" as &[u8]),
        Seed::from(&bump_ref as &[u8]),
    ];
    let signer = Signer::from(&seeds as &[Seed]);

    // SAFETY: instruction_account and cpi_account are valid for the duration of the call.
    // Pointers are derived from references with matching lifetimes.
    let result = unsafe {
        invoke_raw(
            program.address(),
            &instruction_account as *const InstructionAccount,
            1,
            instruction_data.as_ptr(),
            instruction_data.len(),
            &cpi_account as *const RawCpiAccount,
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
