use crate::{error::ZeroPodError, pod::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    Fixed,
    Compact,
}

/// Validation trait for stored (pod) types.
/// Each pod type knows how to validate itself.
pub trait ZcValidate: Copy {
    /// Validate that this value's bytes represent a valid state.
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError>;
}

// --- ZcValidate: trivially valid types (all bit patterns valid) ---

impl ZcValidate for u8 {
    #[inline(always)]
    fn validate_ref(_: &Self) -> Result<(), ZeroPodError> {
        Ok(())
    }
}

impl ZcValidate for i8 {
    #[inline(always)]
    fn validate_ref(_: &Self) -> Result<(), ZeroPodError> {
        Ok(())
    }
}

macro_rules! impl_zc_validate_trivial {
    ($($ty:ty),*) => {
        $(
            impl ZcValidate for $ty {
                #[inline(always)]
                fn validate_ref(_: &Self) -> Result<(), ZeroPodError> { Ok(()) }
            }
        )*
    };
}

impl_zc_validate_trivial!(PodU16, PodU32, PodU64, PodU128, PodI16, PodI32, PodI64, PodI128);

impl<const N: usize> ZcValidate for [u8; N] {
    #[inline(always)]
    fn validate_ref(_: &Self) -> Result<(), ZeroPodError> {
        Ok(())
    }
}

// --- ZcValidate: PodBool (byte must be 0 or 1) ---

impl ZcValidate for PodBool {
    #[inline(always)]
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
        // SAFETY: PodBool is #[repr(transparent)] over [u8; 1], alignment 1.
        // Dereferencing as *const u8 reads the single stored byte.
        let byte = unsafe { *(value as *const PodBool as *const u8) };
        if byte > 1 {
            Err(ZeroPodError::InvalidBool)
        } else {
            Ok(())
        }
    }
}

// --- ZcValidate: PodString (len <= N, active bytes valid UTF-8) ---

impl<const N: usize, const PFX: usize> ZcValidate for PodString<N, PFX> {
    #[inline(always)]
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
        let raw_len = value.decode_len();
        if raw_len > N {
            return Err(ZeroPodError::InvalidLength);
        }
        // SAFETY: raw_len <= N, and data is a [MaybeUninit<u8>; N] array.
        // The bytes come from account data (initialized memory), not
        // MaybeUninit::uninit().
        let bytes =
            unsafe { core::slice::from_raw_parts(value.data.as_ptr() as *const u8, raw_len) };
        if core::str::from_utf8(bytes).is_err() {
            return Err(ZeroPodError::InvalidUtf8);
        }
        Ok(())
    }
}

// --- ZcValidate: PodVec (len <= N) ---

impl<T: ZcElem, const N: usize, const PFX: usize> ZcValidate for PodVec<T, N, PFX> {
    #[inline(always)]
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
        if value.decode_len() > N {
            return Err(ZeroPodError::InvalidLength);
        }
        for item in value.as_slice() {
            T::validate_ref(item)?;
        }
        Ok(())
    }
}

// --- ZcValidate: PodOption (tag 0 or 1, inner valid if Some) ---

impl<T: Copy + ZcValidate> ZcValidate for PodOption<T> {
    #[inline(always)]
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
        match value.raw_tag() {
            0 => Ok(()),
            1 => {
                // SAFETY: Tag validated as == 1 above, so the MaybeUninit value was
                // initialized by PodOption::some() or deserialization.
                let inner = unsafe { value.assume_init_ref() };
                T::validate_ref(inner)
            }
            _ => Err(ZeroPodError::InvalidTag),
        }
    }
}

/// # Safety
///
/// Implementors must guarantee:
/// - `core::mem::align_of::<Self>() == 1`
/// - The type is safe to view from packed, unaligned bytes via pointer cast
/// - `ZcValidate::validate_ref` correctly rejects all invalid bit patterns
pub unsafe trait ZcElem: Copy + ZcValidate {}

// SAFETY: u8 and i8 are single bytes, trivially align 1, all bit patterns
// valid.
unsafe impl ZcElem for u8 {}
unsafe impl ZcElem for i8 {}

// SAFETY: All Pod integer types are #[repr(transparent)] over [u8; N], align 1.
unsafe impl ZcElem for PodU16 {}
unsafe impl ZcElem for PodU32 {}
unsafe impl ZcElem for PodU64 {}
unsafe impl ZcElem for PodU128 {}
unsafe impl ZcElem for PodI16 {}
unsafe impl ZcElem for PodI32 {}
unsafe impl ZcElem for PodI64 {}
unsafe impl ZcElem for PodI128 {}

// SAFETY: PodBool is #[repr(transparent)] over [u8; 1], align 1.
unsafe impl ZcElem for PodBool {}

// SAFETY: [u8; N] is align 1, all bit patterns valid.
unsafe impl<const N: usize> ZcElem for [u8; N] {}

// SAFETY: PodOption<T: ZcElem> is #[repr(C)] with tag: u8 + MaybeUninit<T>.
// T: ZcElem guarantees T is align 1, so PodOption<T> is also align 1.
unsafe impl<T: ZcElem> ZcElem for PodOption<T> {}

// --- Feature-gated impls for external types ---

#[cfg(feature = "solana-address")]
mod solana_address_impls {
    use super::*;

    const _: () = assert!(core::mem::align_of::<solana_address::Address>() == 1);

    // SAFETY: solana_address::Address is #[repr(transparent)] over [u8; 32],
    // align 1, all bit patterns valid.
    impl ZcValidate for solana_address::Address {
        #[inline(always)]
        fn validate_ref(_: &Self) -> Result<(), ZeroPodError> {
            Ok(())
        }
    }

    // SAFETY: Address is Copy, align 1, all bit patterns valid.
    unsafe impl ZcElem for solana_address::Address {}

    impl ZcField for solana_address::Address {
        type Pod = solana_address::Address;
        const POD_SIZE: usize = 32;
    }
}

/// Declares whether a type uses a fixed or compact zero-copy layout.
pub trait ZeroPodSchema: Sized {
    const LAYOUT: LayoutKind;
}

/// Zero-copy access for fixed-size types (all fields are `Copy`, no dynamic
/// tails).
pub trait ZeroPodFixed: ZeroPodSchema {
    type Zc: Copy;
    const SIZE: usize;
    fn from_bytes(data: &[u8]) -> Result<&Self::Zc, ZeroPodError>;
    fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, ZeroPodError>;
    fn validate(data: &[u8]) -> Result<(), ZeroPodError>;
    /// # Safety
    /// Caller must ensure `data` is at least `Self::SIZE` bytes and contains
    /// valid content.
    unsafe fn from_bytes_unchecked(data: &[u8]) -> &Self::Zc {
        &*(data.as_ptr() as *const Self::Zc)
    }
    /// # Safety
    /// Caller must ensure `data` is at least `Self::SIZE` bytes and contains
    /// valid content.
    unsafe fn from_bytes_mut_unchecked(data: &mut [u8]) -> &mut Self::Zc {
        &mut *(data.as_mut_ptr() as *mut Self::Zc)
    }
}

/// Zero-copy access for compact (variable-length) types with a fixed header and
/// dynamic tails.
pub trait ZeroPodCompact: ZeroPodSchema {
    type Header: Copy;
    const HEADER_SIZE: usize;
    fn header(data: &[u8]) -> Result<&Self::Header, ZeroPodError>;
    fn header_mut(data: &mut [u8]) -> Result<&mut Self::Header, ZeroPodError>;
    fn validate(data: &[u8]) -> Result<(), ZeroPodError>;
}

/// Maps a native Rust type to its pod (zero-copy) companion and byte size.
pub trait ZcField: Sized {
    type Pod: Copy;
    const POD_SIZE: usize;
}

// Built-in ZcField impls
macro_rules! impl_zc_field {
    ($native:ty, $pod:ty, $size:expr) => {
        impl ZcField for $native {
            type Pod = $pod;
            const POD_SIZE: usize = $size;
        }
    };
}

impl_zc_field!(u8, u8, 1);
impl_zc_field!(u16, PodU16, 2);
impl_zc_field!(u32, PodU32, 4);
impl_zc_field!(u64, PodU64, 8);
impl_zc_field!(u128, PodU128, 16);
impl_zc_field!(i8, i8, 1);
impl_zc_field!(i16, PodI16, 2);
impl_zc_field!(i32, PodI32, 4);
impl_zc_field!(i64, PodI64, 8);
impl_zc_field!(i128, PodI128, 16);
impl_zc_field!(bool, PodBool, 1);

impl<const N: usize> ZcField for [u8; N] {
    type Pod = [u8; N];
    const POD_SIZE: usize = N;
}

macro_rules! impl_zc_field_identity {
    ($($ty:ty),*) => {
        $(
            impl ZcField for $ty {
                type Pod = Self;
                const POD_SIZE: usize = core::mem::size_of::<Self>();
            }
        )*
    };
}

impl_zc_field_identity!(PodU16, PodU32, PodU64, PodU128, PodI16, PodI32, PodI64, PodI128, PodBool);

impl<const N: usize, const PFX: usize> ZcField for PodString<N, PFX> {
    type Pod = Self;
    const POD_SIZE: usize = core::mem::size_of::<Self>();
}

impl<T: ZcElem, const N: usize, const PFX: usize> ZcField for PodVec<T, N, PFX> {
    type Pod = Self;
    const POD_SIZE: usize = core::mem::size_of::<Self>();
}

impl<T: Copy> ZcField for PodOption<T> {
    type Pod = Self;
    const POD_SIZE: usize = core::mem::size_of::<Self>();
}

impl<T> ZcField for Option<T>
where
    T: ZcField,
    T::Pod: Copy,
{
    type Pod = PodOption<T::Pod>;
    const POD_SIZE: usize = core::mem::size_of::<PodOption<T::Pod>>();
}
