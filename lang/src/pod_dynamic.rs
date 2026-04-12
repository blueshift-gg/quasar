//! Runtime helpers for Pod-based dynamic account fields.
//!
//! Provides the memmove + realloc logic for accounts with `PodString<N>` /
//! `PodVec<T, N>` fields that use dynamic sizing (account allocates only
//! actual content, not max capacity).
//!
//! Generated code calls these helpers to keep codegen thin.

use {crate::prelude::ProgramError, solana_account_view::AccountView};

/// Rewrite a variable-length field in-place, shifting trailing data if the
/// content length changes.
///
/// - `view`: mutable account view
/// - `field_offset`: byte offset of the field's prefix in account data
/// - `prefix_bytes`: size of the length prefix (1 for PodString, 2 for PodVec)
/// - `old_data_bytes`: current data byte length (after prefix)
/// - `new_prefix`: encoded prefix bytes to write (1 or 2 bytes)
/// - `new_data`: raw data bytes to write after the prefix
/// - `payer`: account that pays for realloc
/// - `rent_lpb`: lamports per byte from rent sysvar
/// - `rent_threshold`: exemption threshold from rent sysvar
///
/// On grow: reallocs first, then shifts tail right.
/// On shrink: shifts tail left, then reallocs.
/// On same size: just overwrites data.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn pod_field_rewrite(
    view: &mut AccountView,
    field_offset: usize,
    prefix_bytes: usize,
    old_data_bytes: usize,
    new_prefix: &[u8],
    new_data: &[u8],
    payer: &AccountView,
    rent_lpb: u64,
    rent_threshold: u64,
) -> Result<(), ProgramError> {
    let new_data_bytes = new_data.len();
    let data_len = view.data_len();

    if new_data_bytes != old_data_bytes {
        let delta = new_data_bytes as isize - old_data_bytes as isize;
        let new_total = (data_len as isize + delta) as usize;
        let tail_start = field_offset + prefix_bytes + old_data_bytes;
        let tail_size = data_len - tail_start;

        if delta > 0 {
            // Growing — realloc first to make room, then shift tail right.
            crate::accounts::account::realloc_account_raw(
                view,
                new_total,
                payer,
                rent_lpb,
                rent_threshold,
            )?;
        }

        // SAFETY: `tail_start` and destination are within the (possibly
        // realloc'd) account buffer. `core::ptr::copy` handles overlap.
        // SVM realloc is in-place (10KB realloc zone pre-allocated per account),
        // so `view.data_mut_ptr()` returns the same base address after realloc.
        if tail_size > 0 {
            unsafe {
                let ptr = view.data_mut_ptr();
                core::ptr::copy(
                    ptr.add(tail_start),
                    ptr.add((tail_start as isize + delta) as usize),
                    tail_size,
                );
            }
        }

        if delta < 0 {
            // Shrinking — tail already shifted left, now shrink the account.
            crate::accounts::account::realloc_account_raw(
                view,
                new_total,
                payer,
                rent_lpb,
                rent_threshold,
            )?;
        }
    }

    // Write prefix + data.
    // SAFETY: field_offset + prefix_bytes + new_data_bytes <= view.data_len()
    // (guaranteed by the realloc above for growth, or same-size for no change).
    unsafe {
        let ptr = view.data_mut_ptr().add(field_offset);
        core::ptr::copy_nonoverlapping(new_prefix.as_ptr(), ptr, prefix_bytes);
        core::ptr::copy_nonoverlapping(new_data.as_ptr(), ptr.add(prefix_bytes), new_data_bytes);
    }

    Ok(())
}
