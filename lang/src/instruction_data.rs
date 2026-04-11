//! Instruction data deserialization helpers for dynamic fields.
//!
//! These functions extract variable-length instruction arguments (strings,
//! byte vectors, tail data) from raw instruction bytes. They are called by
//! proc-macro-generated `#[instruction]` code.
//!
//! Each function uses const generics for the prefix byte width, enabling
//! monomorphization per variant (no runtime dispatch).

use solana_program_error::ProgramError;

// Prefix reads use native byte order via read_unaligned. Solana instruction
// data is little-endian. This is correct on all Solana-supported targets
// (SBF, x86-64, aarch64) but would silently produce wrong results on BE.
const _: () = assert!(cfg!(target_endian = "little"));

/// Read a length-prefixed UTF-8 string from instruction data.
///
/// Returns `(parsed_str, new_offset)`. The `PREFIX` const generic (1, 2, or 4)
/// determines the byte width of the length prefix. Monomorphized per variant.
#[inline(always)]
pub fn read_dynamic_str<const PREFIX: usize>(
    data: &[u8],
    offset: usize,
    max_len: usize,
) -> Result<(&str, usize), ProgramError> {
    if data.len() < offset + PREFIX {
        return Err(ProgramError::InvalidInstructionData);
    }

    let len = read_prefix::<PREFIX>(data, offset);
    let offset = offset + PREFIX;

    if len > max_len {
        return Err(ProgramError::InvalidInstructionData);
    }

    if data.len() < offset + len {
        return Err(ProgramError::InvalidInstructionData);
    }

    let bytes = &data[offset..offset + len];
    let s = core::str::from_utf8(bytes).map_err(|_| ProgramError::InvalidInstructionData)?;

    Ok((s, offset + len))
}

/// Read a length-prefixed typed slice from instruction data.
///
/// Returns `(parsed_slice, new_offset)`. The `PREFIX` const generic (1, 2, or
/// 4) determines the byte width of the count prefix.
///
/// # Safety contract
///
/// `T` must have alignment 1. The derive macro enforces this with a
/// compile-time assertion that MUST remain in the macro after extraction.
/// This function does not check alignment at runtime.
#[inline(always)]
pub fn read_dynamic_vec<T, const PREFIX: usize>(
    data: &[u8],
    offset: usize,
    max_count: usize,
) -> Result<(&[T], usize), ProgramError> {
    if data.len() < offset + PREFIX {
        return Err(ProgramError::InvalidInstructionData);
    }

    let count = read_prefix::<PREFIX>(data, offset);
    let offset = offset + PREFIX;

    if count > max_count {
        return Err(ProgramError::InvalidInstructionData);
    }

    let byte_len = count
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(ProgramError::InvalidInstructionData)?;

    if data.len() < offset + byte_len {
        return Err(ProgramError::InvalidInstructionData);
    }

    // SAFETY: Bounds checked above. The caller (derive macro) ensures T has
    // alignment 1 via a compile-time assertion. The pointer from `data` is
    // valid for `byte_len` bytes.
    let slice =
        unsafe { core::slice::from_raw_parts(data.as_ptr().add(offset) as *const T, count) };

    Ok((slice, offset + byte_len))
}

/// Read a length prefix from instruction data. The `PREFIX` const generic
/// determines the byte width (1, 2, or 4). Monomorphized per variant so the
/// match is eliminated at compile time.
#[inline(always)]
fn read_prefix<const PREFIX: usize>(data: &[u8], offset: usize) -> usize {
    match PREFIX {
        1 => data[offset] as usize,
        2 => {
            // SAFETY: Bounds checked by caller. read_unaligned handles align-1 data.
            unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset) as *const u16) as usize }
        }
        4 => {
            // SAFETY: Same as above.
            unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset) as *const u32) as usize }
        }
        // SAFETY: PREFIX is a const generic only instantiated as 1, 2, or 4
        // by the derive macro. This branch is dead code.
        _ => unsafe { core::hint::unreachable_unchecked() },
    }
}
