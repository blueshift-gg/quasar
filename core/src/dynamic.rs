/// Dynamic string field for `#[account]` and `#[instruction]` structs.
///
/// `String<P, N>` is a marker type recognized by the `#[account]` and
/// `#[instruction]` macros. In the emitted code, it becomes `&'a str`
/// (accounts) or `&str` (instructions).
///
/// - `P`: length prefix type — `u8`, `u16`, or `u32`. Determines the wire
///   format of the inline length prefix. Defaults to `u32` (Borsh-compatible).
/// - `N`: maximum byte length. Validated at write time. Defaults to `1024`.
///
/// The prefix encodes the **byte length** of the string data. The actual
/// UTF-8 bytes follow the prefix inline in the account/instruction data.
///
/// # Examples
///
/// ```ignore
/// #[account(discriminator = 5)]
/// pub struct Profile<'a> {
///     pub owner: Address,
///     pub name: String,               // u32 prefix, max 1024
///     pub bio: String<u16, 4096>,      // u16 prefix, max 4096
///     pub tag: String<u8, 32>,         // u8 prefix, max 32
/// }
/// ```
pub struct String<P = u32, const MAX: usize = 1024>(core::marker::PhantomData<P>);

/// Dynamic array field for `#[account]` and `#[instruction]` structs.
///
/// `Vec<T, P, N>` is a marker type recognized by the `#[account]` and
/// `#[instruction]` macros. In the emitted code, it becomes `&'a [T]`
/// (accounts) or `&[T]` (instructions).
///
/// - `T`: element type. Must be fixed-size with alignment 1 (e.g. `Address`,
///   `PodU64`). Enforced at compile time.
/// - `P`: count prefix type — `u8`, `u16`, or `u32`. Determines the wire
///   format of the inline count prefix. Defaults to `u32` (Borsh-compatible).
/// - `N`: maximum element count. Validated at write time. Defaults to `8`.
///
/// The prefix encodes the **element count** (not byte length). The elements
/// are packed contiguously after the prefix.
///
/// # Examples
///
/// ```ignore
/// #[account(discriminator = 5)]
/// pub struct Profile<'a> {
///     pub owner: Address,
///     pub tags: Vec<Address>,              // u32 prefix, max 8
///     pub scores: Vec<PodU64, u8, 4>,      // u8 prefix, max 4
/// }
/// ```
pub struct Vec<T, P = u32, const MAX: usize = 8>(core::marker::PhantomData<(T, P)>);

/// Maximum stack buffer size for dynamic field updates when `alloc` is disabled.
///
/// Used by the current `set_dynamic_fields` codegen. Will be removed when
/// the derive macros are updated to use sequential in-place writes.
pub const MAX_DYNAMIC_TAIL: usize = 2048;

/// Raw encoded bytes with codec metadata for zero-copy CPI pass-through.
///
/// When source and target codecs match, the CPI builder can memcpy the raw
/// prefix + data bytes without decode/re-encode. When codecs differ, only
/// the prefix is re-written.
///
/// Created by `_raw()` accessors on account view types.
pub struct RawEncoded<'a, const PREFIX_BYTES: usize> {
    /// The raw bytes including the length/count prefix.
    pub bytes: &'a [u8],
}

impl<'a, const PREFIX_BYTES: usize> RawEncoded<'a, PREFIX_BYTES> {
    /// Create from a slice that includes the prefix.
    #[inline(always)]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Total wire size (prefix + data).
    #[inline(always)]
    pub const fn wire_len(&self) -> usize {
        self.bytes.len()
    }

    /// The data bytes (after the prefix).
    #[inline(always)]
    pub fn data(&self) -> &'a [u8] {
        const { assert!(PREFIX_BYTES <= 4) };
        unsafe { self.bytes.get_unchecked(PREFIX_BYTES..) }
    }

    /// The data length encoded in the prefix.
    #[inline(always)]
    pub fn prefix_value(&self) -> u32 {
        const { assert!(PREFIX_BYTES <= 4) };
        unsafe {
            match PREFIX_BYTES {
                1 => *self.bytes.get_unchecked(0) as u32,
                2 => core::ptr::read_unaligned(self.bytes.as_ptr() as *const u16) as u32,
                4 => core::ptr::read_unaligned(self.bytes.as_ptr() as *const u32),
                _ => core::hint::unreachable_unchecked(),
            }
        }
    }
}
