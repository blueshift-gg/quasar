//! Off-chain instruction building utilities.
//!
//! This module re-exports [`wincode`] for bincode-compatible serialization and
//! provides wrapper types that encode Quasar's dynamic wire format:
//!
//! | Type | Wire format |
//! |------|-------------|
//! | [`DynBytes<P>`] | `P` LE length prefix + raw bytes (`P` defaults to `u32`) |
//! | [`DynVec<T, P>`] | `P` LE count prefix + each item serialized |
//!
//! The prefix type `P` (u8, u16, or u32) must match the on-chain declaration.
//! For example, `String<u8, 100>` on-chain requires `DynBytes<u8>` off-chain.
//!
//! **This is the only module in `quasar-lang` that allocates** — it uses
//! `alloc::vec::Vec` for instruction data buffers since off-chain code runs
//! in a standard allocator environment.

extern crate alloc;

// Re-export instruction types used by generated client code.
pub use solana_instruction::{AccountMeta, Instruction};
// Re-export wincode for downstream derive macro codegen.
pub use wincode;
use {
    alloc::vec::Vec,
    core::{marker::PhantomData, mem::MaybeUninit},
    wincode::{
        config::ConfigCore,
        error::{ReadResult, WriteResult},
        io::{Reader, Writer},
        len::{SeqLen, UseIntLen},
        SchemaRead, SchemaWrite,
    },
};

// ---------------------------------------------------------------------------
// SerializeArg — compile-time dispatch for instruction arg serialization
// ---------------------------------------------------------------------------

/// Instruction argument serialization for the off-chain client.
///
/// Fixed-size types (`u64`, `bool`, `Option<T>`, custom `QuasarSerialize`)
/// go through `InstructionArg::to_zc()` → raw bytes, guaranteeing the wire
/// format matches the on-chain zero-copy layout exactly. Dynamic types
/// (`DynBytes`, `DynVec`) use wincode's standard encoding.
pub trait SerializeArg {
    fn serialize_arg(&self) -> Vec<u8>;
}

/// Blanket impl for all fixed-size InstructionArg types.
impl<T: crate::instruction_arg::InstructionArg> SerializeArg for T
where
    T::Zc: SchemaWrite<wincode::config::DefaultConfig, Src = T::Zc>,
{
    fn serialize_arg(&self) -> Vec<u8> {
        let zc = self.to_zc();
        wincode::serialize(&zc).expect("instruction arg serialization")
    }
}

// ---------------------------------------------------------------------------
// DynBytes<P> — length-prefixed raw byte buffer
// ---------------------------------------------------------------------------

/// A dynamically-sized byte buffer with a little-endian length prefix.
///
/// The prefix type `P` determines the width of the length field:
/// - `DynBytes<u8>` — 1-byte prefix (max 255 bytes)
/// - `DynBytes<u16>` — 2-byte prefix
/// - `DynBytes` / `DynBytes<u32>` — 4-byte prefix (default)
#[derive(Debug, Clone, PartialEq)]
pub struct DynBytes<P = u32>(pub Vec<u8>, PhantomData<P>);

impl<P> DynBytes<P> {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data, PhantomData)
    }
}

impl<P> From<Vec<u8>> for DynBytes<P> {
    fn from(data: Vec<u8>) -> Self {
        Self::new(data)
    }
}

unsafe impl<P, C: ConfigCore> SchemaWrite<C> for DynBytes<P>
where
    UseIntLen<P>: SeqLen<C>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(<UseIntLen<P>>::write_bytes_needed(src.0.len())? + src.0.len())
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        <UseIntLen<P>>::write(writer.by_ref(), src.0.len())?;
        writer.write(&src.0)?;
        Ok(())
    }
}

unsafe impl<'de, P, C: ConfigCore> SchemaRead<'de, C> for DynBytes<P>
where
    UseIntLen<P>: SeqLen<C>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let len = <UseIntLen<P>>::read(reader.by_ref())?;
        let bytes = reader.take_scoped(len)?;
        dst.write(DynBytes(bytes.to_vec(), PhantomData));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DynVec<T, P> — length-prefixed sequence of T
// ---------------------------------------------------------------------------

/// A dynamically-sized vector of `T` with a little-endian element count prefix.
///
/// The prefix type `P` determines the width of the count field:
/// - `DynVec<T, u8>` — 1-byte prefix (max 255 elements)
/// - `DynVec<T, u16>` — 2-byte prefix
/// - `DynVec<T>` / `DynVec<T, u32>` — 4-byte prefix (default)
#[derive(Debug, Clone, PartialEq)]
pub struct DynVec<T, P = u32>(pub Vec<T>, PhantomData<P>);

impl<T, P> DynVec<T, P> {
    pub fn new(data: Vec<T>) -> Self {
        Self(data, PhantomData)
    }
}

impl<T, P> From<Vec<T>> for DynVec<T, P> {
    fn from(data: Vec<T>) -> Self {
        Self::new(data)
    }
}

unsafe impl<T, P, C: ConfigCore> SchemaWrite<C> for DynVec<T, P>
where
    T: SchemaWrite<C, Src = T>,
    UseIntLen<P>: SeqLen<C>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        let mut total = <UseIntLen<P>>::write_bytes_needed(src.0.len())?;
        for item in &src.0 {
            total += T::size_of(item)?;
        }
        Ok(total)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        <UseIntLen<P>>::write(writer.by_ref(), src.0.len())?;
        for item in &src.0 {
            T::write(writer.by_ref(), item)?;
        }
        Ok(())
    }
}

unsafe impl<'de, T, P, C: ConfigCore> SchemaRead<'de, C> for DynVec<T, P>
where
    T: SchemaRead<'de, C, Dst = T>,
    UseIntLen<P>: SeqLen<C>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let len = <UseIntLen<P>>::read(reader.by_ref())?;
        // Cap pre-allocation to avoid OOM from untrusted length prefixes.
        // The actual read loop will fail early if the reader runs out of data.
        let mut vec = Vec::with_capacity(len.min(4096));
        for _ in 0..len {
            vec.push(T::get(reader.by_ref())?);
        }
        dst.write(DynVec(vec, PhantomData));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SerializeArg impls for dynamic types (bypass InstructionArg blanket impl)
// ---------------------------------------------------------------------------

impl<P> SerializeArg for DynBytes<P>
where
    UseIntLen<P>: SeqLen<wincode::config::DefaultConfig>,
{
    fn serialize_arg(&self) -> Vec<u8> {
        wincode::serialize(self).expect("DynBytes serialization")
    }
}

impl<T, P> SerializeArg for DynVec<T, P>
where
    T: SchemaWrite<wincode::config::DefaultConfig, Src = T>,
    UseIntLen<P>: SeqLen<wincode::config::DefaultConfig>,
{
    fn serialize_arg(&self) -> Vec<u8> {
        wincode::serialize(self).expect("DynVec serialization")
    }
}

// ---------------------------------------------------------------------------
// OptionZc<Z> — fixed-size Option serialization matching on-chain ZC layout
// ---------------------------------------------------------------------------

use crate::instruction_arg::OptionZc;

unsafe impl<Z, C: ConfigCore> SchemaWrite<C> for OptionZc<Z>
where
    Z: Copy,
{
    type Src = Self;

    fn size_of(_src: &Self) -> WriteResult<usize> {
        Ok(core::mem::size_of::<Self>())
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        let bytes = unsafe {
            core::slice::from_raw_parts(
                src as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        };
        writer.write(bytes)?;
        Ok(())
    }
}

unsafe impl<'de, Z, C: ConfigCore> SchemaRead<'de, C> for OptionZc<Z>
where
    Z: Copy,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let bytes = reader.take_scoped(core::mem::size_of::<Self>())?;
        // SAFETY: OptionZc<Z> is #[repr(C)] with alignment 1 (tag: u8 +
        // MaybeUninit<Z>). The bytes from the reader are fully initialized.
        let zc = unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const Self) };
        dst.write(zc);
        Ok(())
    }
}
