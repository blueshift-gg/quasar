//! Instruction data deserialization helpers for dynamic fields.
//!
//! These functions extract variable-length instruction arguments (strings and
//! byte vectors) from raw instruction bytes. They are called by
//! proc-macro-generated `#[instruction]` code.
//!
//! Each function uses const generics for the prefix byte width, enabling
//! monomorphization per variant (no runtime dispatch).

use solana_program_error::ProgramError;

// Prefix reads use native byte order via read_unaligned. Solana instruction
// data is little-endian. This is correct on all Solana-supported targets
// (SBF, x86-64, aarch64) but would silently produce wrong results on BE.
const _: () = assert!(cfg!(target_endian = "little"));

/// Sequential cursor over raw instruction data.
///
/// This is the canonical low-level decode primitive for instruction-wire
/// parsing. It intentionally models the wire as declaration-ordered bytes,
/// not as a split fixed-header + dynamic-tail layout.
pub struct InstructionCursor<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> InstructionCursor<'a> {
    #[inline(always)]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    #[inline(always)]
    pub const fn with_offset(data: &'a [u8], offset: usize) -> Self {
        Self { data, offset }
    }

    #[inline(always)]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    #[inline(always)]
    pub fn read_arg<T: crate::instruction_arg::InstructionArg>(
        &mut self,
    ) -> Result<T, ProgramError> {
        let size = core::mem::size_of::<T::Zc>();
        if self.data.len() < self.offset + size {
            return Err(ProgramError::InvalidInstructionData);
        }
        let zc = unsafe { &*(self.data.as_ptr().add(self.offset) as *const T::Zc) };
        T::validate_zc(zc)?;
        self.offset += size;
        Ok(T::from_zc(zc))
    }

    #[inline(always)]
    pub fn read_dynamic_str<const PREFIX: usize>(
        &mut self,
        max_len: usize,
    ) -> Result<&'a str, ProgramError> {
        let (value, new_offset) = read_dynamic_str::<PREFIX>(self.data, self.offset, max_len)?;
        self.offset = new_offset;
        Ok(value)
    }

    #[inline(always)]
    pub fn read_dynamic_vec<T, const PREFIX: usize>(
        &mut self,
        max_count: usize,
    ) -> Result<&'a [T], ProgramError> {
        let (value, new_offset) = read_dynamic_vec::<T, PREFIX>(self.data, self.offset, max_count)?;
        self.offset = new_offset;
        Ok(value)
    }
}

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
        8 => {
            // SAFETY: Same as above.
            unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset) as *const u64) as usize }
        }
        // SAFETY: PREFIX is a const generic only instantiated as 1, 2, 4, or 8
        // by the derive macro. This branch is dead code.
        _ => unsafe { core::hint::unreachable_unchecked() },
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove `read_prefix::<1>` returns `data[offset] as usize`.
    #[kani::proof]
    fn read_prefix_u8_correctness() {
        let data: [u8; 4] = kani::any();
        let offset: usize = kani::any();
        kani::assume(offset < 4);
        let result = read_prefix::<1>(&data, offset);
        assert!(result == data[offset] as usize);
    }

    /// Prove `read_prefix::<2>` returns the little-endian u16 at offset.
    #[kani::proof]
    fn read_prefix_u16_correctness() {
        let data: [u8; 4] = kani::any();
        let offset: usize = kani::any();
        kani::assume(offset <= 2);
        let result = read_prefix::<2>(&data, offset);
        let expected = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        assert!(result == expected);
    }

    /// Prove `read_prefix::<4>` returns the little-endian u32 at offset.
    #[kani::proof]
    fn read_prefix_u32_correctness() {
        let data: [u8; 8] = kani::any();
        let offset: usize = kani::any();
        kani::assume(offset <= 4);
        let result = read_prefix::<4>(&data, offset);
        let expected = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        assert!(result == expected);
    }

    /// Prove `read_dynamic_str` never returns an offset beyond the buffer.
    /// Buffer reduced to 8 bytes (from 16) to keep UTF-8 validation tractable
    /// for CBMC's SAT solver — `core::str::from_utf8` creates a complex
    /// branching state machine that scales poorly with buffer size.
    #[kani::proof]
    #[kani::unwind(10)]
    fn read_dynamic_str_bounds() {
        let data: [u8; 8] = kani::any();
        let offset: usize = kani::any();
        kani::assume(offset <= 8);
        let max_len: usize = kani::any();
        kani::assume(max_len <= 8);

        if let Ok((_, new_offset)) = read_dynamic_str::<1>(&data, offset, max_len) {
            assert!(new_offset <= data.len(), "new_offset must be within buffer");
        }
    }

    /// Prove `read_dynamic_vec::<u8>` never returns an offset beyond the
    /// buffer.
    #[kani::proof]
    #[kani::unwind(18)]
    fn read_dynamic_vec_bounds() {
        let data: [u8; 16] = kani::any();
        let offset: usize = kani::any();
        kani::assume(offset <= 16);
        let max_count: usize = kani::any();
        kani::assume(max_count <= 16);

        if let Ok((_, new_offset)) = read_dynamic_vec::<u8, 1>(&data, offset, max_count) {
            assert!(new_offset <= data.len(), "new_offset must be within buffer");
        }
    }
}
