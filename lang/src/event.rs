//! Self-CPI event emission for spoofing-resistant on-chain events.
//!
//! - **Log-based** (`emit!`): ~100 CU, fast but spoofable.
//! - **Self-CPI** (`emit_cpi!`): ~1,000 CU, unforgeable (program ID in trace).

use {
    crate::cpi::{
        cpi_account_from_view, invoke_raw, result_from_raw, InstructionAccount, Seed, Signer,
    },
    solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Typed self-CPI event emission for an accounts struct.
///
/// `#[derive(Accounts)]` implements this for any struct that carries both an
/// event-authority field (typed `EventAuthority` or named `event_authority`)
/// and a program field. **The program field is detected by TYPE, not name:**
/// any field typed `Program<T>` supplies the signer, so it may be called
/// `program`, `emitter`, or anything else. If an event-authority field is
/// present but no `Program<T>` field is, the derive raises a spanned error
/// rather than silently skipping the impl.
///
/// `emit_cpi!` dispatches through this trait (`EventCpi::emit(self, &event)`),
/// so the macro hard-codes no field names. The default [`EventCpi::emit`] body
/// mirrors [`crate::accounts::Program::emit_event`] exactly, so the self-CPI
/// monomorphizes to identical code and CU.
pub trait EventCpi {
    /// This program's `EventAuthority` PDA bump (`EventAuthority::BUMP`).
    const EVENT_AUTHORITY_BUMP: u8;

    /// The program account that signs the self-CPI (this program itself).
    fn event_program(&self) -> &AccountView;

    /// The `__event_authority` PDA account.
    fn event_authority(&self) -> &AccountView;

    /// Emit `event` via self-CPI to this program's `__event_authority` PDA.
    #[inline(always)]
    fn emit<E: crate::traits::Event>(&self, event: &E) -> Result<(), ProgramError> {
        let program = self.event_program();
        let ea = self.event_authority();
        event.emit(|data| emit_event_cpi(program, ea, data, Self::EVENT_AUTHORITY_BUMP))
    }
}

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
#[doc(hidden)]
pub unsafe fn handle_event(
    ptr: *mut u8,
    instruction_data: &[u8],
    event_authority: &solana_address::Address,
) -> Result<(), ProgramError> {
    // SAFETY: `ptr` is the SVM input buffer start, 8-byte aligned per the SVM
    // ABI, and holds `num_accounts` (u64) at offset 0, so the aligned read is
    // sound.
    if unsafe { *(ptr as *const u64) } == 0 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    // SAFETY: `num_accounts >= 1` (checked above), so the first account entry
    // follows the count word; 8 bytes past the 8-aligned buffer start is that
    // entry's 8-aligned `RuntimeAccount` header.
    let raw = unsafe {
        &*(ptr.add(core::mem::size_of::<u64>()) as *const crate::__internal::RuntimeAccount)
    };

    if raw.is_signer == 0 {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !crate::keys_eq(&raw.address, event_authority) {
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

/// Write the discriminator into the start of a log-event buffer.
///
/// Returns the byte offset where the data region begins (equal to
/// `disc.len()`). After calling, bytes `[0, disc.len())` contain the
/// discriminator. The caller must then write `data_size` bytes at the
/// returned offset to fully initialize the buffer before `assume_init_ref`.
///
/// # Safety
///
/// `buf` must point to at least `disc.len()` writable bytes.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn write_log_disc(buf: *mut u8, disc: &[u8]) -> usize {
    let disc_len = disc.len();
    // SAFETY: Caller guarantees `buf` has at least `disc.len()` writable
    // bytes, and `disc` cannot overlap that output buffer.
    unsafe { core::ptr::copy_nonoverlapping(disc.as_ptr(), buf, disc_len) };
    disc_len
}

/// Write the `0xFF` marker and discriminator into a CPI-event buffer.
///
/// Returns the byte offset where the data region begins (equal to
/// `1 + disc.len()`). After calling, byte 0 is `0xFF` and bytes
/// `[1, 1 + disc.len())` contain the discriminator. The caller must then
/// write `data_size` bytes at the returned offset to fully initialize
/// the buffer before `assume_init_ref`.
///
/// # Safety
///
/// `buf` must point to at least `1 + disc.len()` writable bytes.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn write_cpi_disc(buf: *mut u8, disc: &[u8]) -> usize {
    let disc_len = disc.len();
    // SAFETY: Caller guarantees `buf` has at least `1 + disc.len()` writable
    // bytes, and `disc` cannot overlap that output buffer.
    unsafe {
        core::ptr::write(buf, 0xFF);
        core::ptr::copy_nonoverlapping(disc.as_ptr(), buf.add(1), disc_len);
    }
    1 + disc_len
}

#[cfg(kani)]
#[path = "../kani/event.rs"]
mod kani_proofs;
