extern crate alloc;

use alloc::vec::Vec;

pub use solana_instruction::{AccountMeta, Instruction};

/// Trait for serializing instruction data fields to little-endian bytes.
pub trait WriteBytes {
    fn write_bytes(&self, buf: &mut Vec<u8>);
}

macro_rules! impl_write_bytes_int {
    ($($t:ty),*) => {$(
        impl WriteBytes for $t {
            #[inline(always)]
            fn write_bytes(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_le_bytes());
            }
        }
    )*}
}

impl_write_bytes_int!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

impl WriteBytes for bool {
    #[inline(always)]
    fn write_bytes(&self, buf: &mut Vec<u8>) {
        buf.push(*self as u8);
    }
}

impl WriteBytes for solana_address::Address {
    #[inline(always)]
    fn write_bytes(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.as_ref());
    }
}

impl<const N: usize> WriteBytes for [u8; N] {
    #[inline(always)]
    fn write_bytes(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self);
    }
}

#[inline(always)]
pub fn build_instruction_data(disc: &[u8], write_args: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut data = Vec::from(disc);
    write_args(&mut data);
    data
}
