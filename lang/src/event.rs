//! Self-CPI event emission for spoofing-resistant on-chain events.
//!
//! - **Log-based** (`emit!`) — ~100 CU, fast but spoofable.
//! - **Self-CPI** (`emit_cpi!`) — ~1,000 CU, unforgeable (program ID in trace).

use {
    crate::cpi::{
        cpi_account_from_view, invoke_raw, result_from_raw, InstructionAccount, Seed, Signer,
    },
    solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Validate and log an inbound event CPI.
///
/// Called by the generated `__handle_event` dispatch stub. Checks that the
/// first account is a signer matching the program's event authority PDA,
/// then logs the instruction data (minus the `0xFF` prefix).
///
/// # Safety
///
/// `ptr` must point to the start of a valid SVM input buffer (account count
/// at offset 0, followed by serialized `RuntimeAccount` entries).
#[inline(always)]
pub unsafe fn handle_event(
    ptr: *mut u8,
    instruction_data: &[u8],
    event_authority: &solana_address::Address,
) -> Result<(), ProgramError> {
    // SAFETY: The SVM places the account count (u64) at offset 0.
    if *(ptr as *const u64) == 0 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    // SAFETY: Pointer arithmetic follows the SVM input buffer layout.
    let raw = ptr.add(core::mem::size_of::<u64>()) as *const crate::__internal::RuntimeAccount;

    if (*raw).is_signer == 0 {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !crate::keys_eq(&(*raw).address, event_authority) {
        return Err(ProgramError::InvalidSeeds);
    }

    if instruction_data.len() <= 1 {
        return Err(ProgramError::InvalidInstructionData);
    }

    crate::log::log_data(&[&instruction_data[1..]]);
    Ok(())
}

/// Emit an event via self-CPI to the program's own `__event_authority` PDA.
///
/// The self-CPI proves the event was emitted by the program (the program ID
/// appears in the transaction trace), preventing log spoofing by other
/// programs.
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

    // SAFETY: All pointer/length arguments are derived from stack-local
    // values that outlive the syscall. Single account (count = 1) ensures
    // the pointer-to-element casts are valid.
    let result = unsafe {
        invoke_raw(
            program.address(),
            &instruction_account as *const _,
            1,
            instruction_data.as_ptr(),
            instruction_data.len(),
            &cpi_account as *const _,
            1,
            &[signer],
        )
    };

    result_from_raw(result)
}
