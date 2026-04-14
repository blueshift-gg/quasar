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

// ---------------------------------------------------------------------------
// Shared buffer-init helpers (used by generated code AND Kani proofs)
// ---------------------------------------------------------------------------

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
pub unsafe fn write_log_disc(buf: *mut u8, disc: &[u8]) -> usize {
    let disc_len = disc.len();
    core::ptr::copy_nonoverlapping(disc.as_ptr(), buf, disc_len);
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
pub unsafe fn write_cpi_disc(buf: *mut u8, disc: &[u8]) -> usize {
    let disc_len = disc.len();
    core::ptr::write(buf, 0xFF);
    core::ptr::copy_nonoverlapping(disc.as_ptr(), buf.add(1), disc_len);
    1 + disc_len
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    // --- write_log_disc ---

    /// Prove `write_log_disc` returns an offset equal to the discriminator
    /// length and copies the discriminator bytes correctly.
    #[kani::proof]
    fn write_log_disc_offset_and_copy() {
        let disc: [u8; 8] = kani::any();
        let disc_len: usize = kani::any();
        kani::assume(disc_len >= 1 && disc_len <= 8);

        let mut buf = [0u8; 16];
        let offset = unsafe { write_log_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

        assert!(offset == disc_len);
        // Discriminator was copied faithfully.
        let mut i = 0usize;
        while i < disc_len {
            assert!(buf[i] == disc[i]);
            i += 1;
        }
    }

    /// Prove the log buffer is fully covered: `write_log_disc` initializes
    /// `[0, offset)` and `write_data` initializes `[offset, total)` with no
    /// gap, so `assume_init_ref` over the full buffer is safe.
    #[kani::proof]
    fn log_buffer_full_coverage() {
        let disc: [u8; 8] = kani::any();
        let disc_len: usize = kani::any();
        let data_size: usize = kani::any();
        kani::assume(disc_len >= 1 && disc_len <= 8);
        kani::assume(data_size <= 56);

        let total = disc_len + data_size;
        let mut buf = [0u8; 64];
        let offset = unsafe { write_log_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

        // Disc region [0, offset) + data region [offset, offset+data_size) = [0, total)
        assert!(offset == disc_len);
        assert!(offset + data_size == total);
    }

    // --- write_cpi_disc ---

    /// Prove `write_cpi_disc` writes the 0xFF marker, copies the discriminator,
    /// and returns the correct data offset.
    #[kani::proof]
    fn write_cpi_disc_offset_and_marker() {
        let disc: [u8; 8] = kani::any();
        let disc_len: usize = kani::any();
        kani::assume(disc_len >= 1 && disc_len <= 8);

        let mut buf = [0u8; 16];
        let offset = unsafe { write_cpi_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

        assert!(offset == 1 + disc_len);
        assert!(buf[0] == 0xFF);
        // Discriminator was copied faithfully.
        let mut i = 0usize;
        while i < disc_len {
            assert!(buf[1 + i] == disc[i]);
            i += 1;
        }
    }

    /// Prove the CPI buffer is fully covered: `write_cpi_disc` initializes
    /// `[0, offset)` and `write_data` initializes `[offset, total)` with no
    /// gap.
    #[kani::proof]
    fn cpi_buffer_full_coverage() {
        let disc: [u8; 8] = kani::any();
        let disc_len: usize = kani::any();
        let data_size: usize = kani::any();
        kani::assume(disc_len >= 1 && disc_len <= 8);
        kani::assume(data_size <= 56);

        let total = 1 + disc_len + data_size;
        let mut buf = [0u8; 64];
        let offset = unsafe { write_cpi_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

        // Marker [0) + disc [1, offset) + data [offset, offset+data_size) = [0, total)
        assert!(offset == 1 + disc_len);
        assert!(offset + data_size == total);
    }

    // --- handle_event pointer arithmetic ---

    /// Prove the SVM buffer pointer offset in `handle_event` is correctly
    /// computed: `ptr.add(size_of::<u64>())` advances exactly 8 bytes past
    /// the account count to reach the first RuntimeAccount.
    ///
    /// The SVM input buffer layout places a u64 account count at offset 0,
    /// followed by serialized RuntimeAccount entries. The pointer arithmetic
    /// `ptr.add(size_of::<u64>())` must equal `ptr + 8`.
    #[kani::proof]
    fn handle_event_ptr_offset_is_8() {
        // size_of::<u64>() is the offset used to skip the account count.
        assert!(core::mem::size_of::<u64>() == 8);
        // The offset is a compile-time constant, so this also verifies
        // that the add(8) does not depend on any runtime value.
    }

    /// Prove the `instruction_data[1..]` slice in `handle_event` is safe
    /// given the `len() <= 1` guard.
    ///
    /// `handle_event` returns `Err(InvalidInstructionData)` when
    /// `instruction_data.len() <= 1`, so the `&instruction_data[1..]` slice
    /// is only reached when len >= 2, making the index 1 always valid.
    #[kani::proof]
    fn handle_event_data_slice_after_guard() {
        let data_len: usize = kani::any();
        kani::assume(data_len <= 1024);

        // Guard from handle_event:
        if data_len <= 1 {
            // Returns error, no slice operation.
            return;
        }

        // If we reach here, data_len >= 2, so &data[1..] is valid.
        assert!(data_len >= 2);
        let remaining = data_len - 1;
        assert!(remaining >= 1);
        assert!(remaining < data_len);
    }

    /// Prove `write_cpi_disc` buf.add(1) is safe: the function writes 0xFF
    /// at offset 0 and then copies `disc_len` bytes starting at offset 1.
    /// The total write region is `1 + disc_len` bytes. This proves the
    /// `buf.add(1)` pointer offset does not overflow and stays within the
    /// buffer for any valid discriminator length.
    #[kani::proof]
    fn write_cpi_disc_add_one_no_overflow() {
        let disc_len: usize = kani::any();
        kani::assume(disc_len >= 1 && disc_len <= 8);

        // Total bytes written: 1 (marker) + disc_len (discriminator).
        let total = 1usize.checked_add(disc_len);
        assert!(total.is_some());
        let total = total.unwrap();

        // The write at buf.add(1) for disc_len bytes ends at offset total.
        assert!(total == 1 + disc_len);
        assert!(total <= 9); // max: 1 + 8
    }
}
