//! Instruction data: the bytes after the discriminator, one value of one
//! size in Borsh's encoding, decoded before any account loads.

use zeropod::ZeroPod;

use crate::{ProgramError, Result};

/// The arguments of an instruction. Implemented by `#[derive(InstructionData)]`,
/// field by field, and by `()` for an instruction without any.
pub trait InstructionData: Sized {
    /// The encoding's length, which every value has.
    const SIZE: usize;

    /// Decodes exactly `SIZE` bytes.
    fn decode(bytes: &[u8]) -> Result<Self>;

    /// `out` is exactly `SIZE` long.
    fn encode(&self, out: &mut [u8]);

    /// Each field's name and type, as the source writes them.
    #[cfg(not(target_os = "solana"))]
    const IDL: &'static [crate::client::IdlField];
}

impl InstructionData for () {
    const SIZE: usize = 0;

    #[inline(always)]
    fn decode(bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() { Ok(()) } else { Err(ProgramError::InvalidInstructionData) }
    }

    fn encode(&self, _: &mut [u8]) {}

    #[cfg(not(target_os = "solana"))]
    const IDL: &'static [crate::client::IdlField] = &[];
}

/// A field's size: instruction data is fixed, so decoding it never
/// allocates. Evaluated when compiling, where it fails for `String` or `Vec`.
pub const fn field_size<T: ZeroPod>() -> usize {
    match T::SIZE {
        Some(size) => size,
        None => panic!("instruction data has one size: use `ArrayString<N>` or `ArrayVec<T, N>`"),
    }
}

/// Reads the field at `*at` and moves past it.
#[inline(always)]
pub fn take<T: ZeroPod>(bytes: &[u8], at: &mut usize) -> Result<T> {
    let end = *at + field_size::<T>();
    let field = bytes.get(*at..end).ok_or(ProgramError::InvalidInstructionData)?;
    *at = end;
    zeropod::from_slice(field).map_err(|_| ProgramError::InvalidInstructionData)
}

/// Writes `value` at `*at` and moves past it.
pub fn put<T: ZeroPod>(value: &T, out: &mut [u8], at: &mut usize) {
    let end = *at + field_size::<T>();
    // A fixed-size value fits its size and holds nothing a limit rejects.
    if zeropod::write(value, &mut out[*at..end]).is_err() {
        unreachable!("a value of one size fits it");
    }
    *at = end;
}

/// The discriminator, then `data`.
pub fn encode<D: InstructionData, const N: usize>(discriminator: &[u8], data: &D) -> [u8; N] {
    let mut bytes = [0; N];
    bytes[..discriminator.len()].copy_from_slice(discriminator);
    data.encode(&mut bytes[discriminator.len()..]);
    bytes
}

/// Sets a handler's result as the instruction's return data; `()` sets none.
#[inline(always)]
pub fn set_return<T: ZeroPod>(value: &T) {
    let size = field_size::<T>();
    if size == 0 {
        return;
    }
    if T::NATIVE {
        // SAFETY: a native type's bytes in memory are its encoding, `size` long.
        let bytes = unsafe { core::slice::from_raw_parts(core::ptr::from_ref(value).cast(), size) };
        return crate::effects::set_return_data(bytes);
    }
    let mut bytes = [0; crate::effects::MAX_RETURN_DATA];
    let mut at = 0;
    put(value, &mut bytes, &mut at);
    crate::effects::set_return_data(&bytes[..size]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_is_read_where_the_last_ended() {
        let bytes = [7, 1, 0, 0, 0];
        let mut at = 0;
        assert_eq!(take::<u8>(&bytes, &mut at), Ok(7));
        assert_eq!(take::<u32>(&bytes, &mut at), Ok(1));
        assert_eq!(take::<u8>(&bytes, &mut at), Err(ProgramError::InvalidInstructionData));
    }

    #[test]
    fn no_arguments_means_no_bytes() {
        assert_eq!(<()>::decode(&[]), Ok(()));
        assert_eq!(<()>::decode(&[0]), Err(ProgramError::InvalidInstructionData));
    }
}
