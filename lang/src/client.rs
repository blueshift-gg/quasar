//! Off-chain instruction building utilities.
//!
//! This module re-exports [`wincode`] for bincode-compatible serialization and
//! provides wrapper types that encode Quasar's dynamic wire format:
//!
//! | Type | Wire format |
//! |------|-------------|
//! | `DynBytes<P>` | `P` LE length prefix + raw bytes (`P` defaults to `u32`) |
//! | `DynVec<T, P>` | `P` LE count prefix + each item serialized |
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

    /// Number of bytes in the payload (excludes the prefix).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the payload is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The raw byte payload.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl<P> From<Vec<u8>> for DynBytes<P> {
    fn from(data: Vec<u8>) -> Self {
        Self::new(data)
    }
}

impl<P> From<alloc::string::String> for DynBytes<P> {
    fn from(s: alloc::string::String) -> Self {
        Self::new(s.into_bytes())
    }
}

impl<P> From<&str> for DynBytes<P> {
    fn from(s: &str) -> Self {
        Self::new(s.as_bytes().to_vec())
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
// DynString<P> — length-prefixed string for client instruction args
// ---------------------------------------------------------------------------

/// A dynamically-sized UTF-8 string with a little-endian length prefix.
///
/// This is the client-side type for `String<N>` instruction arguments.
/// The wire format is `[prefix][utf8 bytes]` — identical to `DynBytes`,
/// but with string-native ergonomics.
///
/// ```ignore
/// let ix = MyInstruction {
///     name: "hello".into(),          // From<&str>
///     name: my_string.into(),        // From<String>
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DynString<P = u8>(DynBytes<P>);

impl<P> DynString<P> {
    pub fn new(s: &str) -> Self {
        Self(DynBytes::new(s.as_bytes().to_vec()))
    }

    /// Number of UTF-8 bytes in the string (excludes the prefix).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The raw UTF-8 bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl<P> From<&str> for DynString<P> {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl<P> From<alloc::string::String> for DynString<P> {
    fn from(s: alloc::string::String) -> Self {
        Self(DynBytes::new(s.into_bytes()))
    }
}

impl<P> From<Vec<u8>> for DynString<P> {
    fn from(bytes: Vec<u8>) -> Self {
        Self(DynBytes::new(bytes))
    }
}

unsafe impl<P, C: ConfigCore> SchemaWrite<C> for DynString<P>
where
    UseIntLen<P>: SeqLen<C>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        DynBytes::<P>::size_of(&src.0)
    }

    fn write(writer: impl Writer, src: &Self) -> WriteResult<()> {
        DynBytes::<P>::write(writer, &src.0)
    }
}

unsafe impl<'de, P, C: ConfigCore> SchemaRead<'de, C> for DynString<P>
where
    UseIntLen<P>: SeqLen<C>,
{
    type Dst = Self;

    fn read(reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        // SAFETY: DynString is repr-compatible with DynBytes (single field).
        let inner = dst as *mut MaybeUninit<Self> as *mut MaybeUninit<DynBytes<P>>;
        DynBytes::<P>::read(reader, unsafe { &mut *inner })
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

    /// Number of elements (excludes the prefix).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over the elements.
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T, P> From<Vec<T>> for DynVec<T, P> {
    fn from(data: Vec<T>) -> Self {
        Self::new(data)
    }
}

impl<T: Clone, P> From<&[T]> for DynVec<T, P> {
    fn from(data: &[T]) -> Self {
        Self::new(data.to_vec())
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

impl<P> SerializeArg for DynString<P>
where
    UseIntLen<P>: SeqLen<wincode::config::DefaultConfig>,
{
    fn serialize_arg(&self) -> Vec<u8> {
        wincode::serialize(self).expect("DynString serialization")
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

/// Pass pre-serialized bytes through as-is. Used by borrowed struct args (e.g.
/// `MintArgs<'_>`) where the off-chain client serializes the struct manually.
impl SerializeArg for Vec<u8> {
    fn serialize_arg(&self) -> Vec<u8> {
        self.clone()
    }
}

// OptionZc<Z> is now a type alias for PodOption<Z>, which already has
// SchemaWrite/SchemaRead impls in zeropod's wincode module.
