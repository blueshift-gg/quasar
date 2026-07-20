//! Traits for instruction arguments.
//!
//! Zeropod owns all storage layout and validation. Quasar provides the
//! native-to-pod conversion bridge so framework types participate in
//! instruction decoding.
//!
//! - Fixed args: `InstructionArg` (zero-copy pointer cast)
//! - Dynamic args: zeropod compact `Ref` views (zero-copy borrowed access)

use crate::pod::*;

/// A type that can appear as a fixed-size `#[instruction]` argument.
///
/// The associated `Zc` type must satisfy [`zeropod::ZcElem`]. This is the
/// safety boundary that permits generated decoders to borrow untrusted
/// instruction bytes before semantic validation: the type has alignment 1,
/// contains no padding, and accepts every initialized bit pattern as a valid
/// Rust value.
pub trait InstructionArg: Sized {
    /// The zero-copy companion type for deserialization.
    type Zc: zeropod::ZcElem;
    /// Reconstruct the native value from its ZC representation.
    fn from_zc(zc: &Self::Zc) -> Self;
    /// Convert the native value into its alignment-1 ZC representation.
    fn to_zc(&self) -> Self::Zc;

    /// Validate the raw ZC bytes before calling `from_zc`.
    ///
    /// Called by `#[instruction]` codegen on untrusted instruction data.
    /// The default delegates to [`zeropod::ZcValidate`]. Override only when
    /// the native conversion requires stricter validation.
    #[inline(always)]
    fn validate_zc(zc: &Self::Zc) -> Result<(), crate::prelude::ProgramError> {
        <Self::Zc as zeropod::ZcValidate>::validate_ref(zc)
            .map_err(|_| crate::prelude::ProgramError::InvalidInstructionData)
    }
}

/// Bridge trait for instruction-arg types that can also appear as zeropod
/// schema fields.
pub trait InstructionArgField:
    InstructionArg + zeropod::ZcField<Pod = <Self as InstructionArg>::Zc>
{
}

impl<T> InstructionArgField for T where
    T: InstructionArg + zeropod::ZcField<Pod = <T as InstructionArg>::Zc>
{
}

mod sealed {
    pub trait BuiltinPod {}
}
use sealed::BuiltinPod;

// Primitives
impl BuiltinPod for u8 {}
impl BuiltinPod for i8 {}
impl BuiltinPod for u16 {}
impl BuiltinPod for u32 {}
impl BuiltinPod for u64 {}
impl BuiltinPod for u128 {}
impl BuiltinPod for i16 {}
impl BuiltinPod for i32 {}
impl BuiltinPod for i64 {}
impl BuiltinPod for i128 {}
impl BuiltinPod for bool {}
impl BuiltinPod for solana_address::Address {}
impl<const N: usize> BuiltinPod for [u8; N] {}

// Pod types (identity)
impl BuiltinPod for PodU16 {}
impl BuiltinPod for PodU32 {}
impl BuiltinPod for PodU64 {}
impl BuiltinPod for PodU128 {}
impl BuiltinPod for PodI16 {}
impl BuiltinPod for PodI32 {}
impl BuiltinPod for PodI64 {}
impl BuiltinPod for PodI128 {}
impl BuiltinPod for PodBool {}

/// Instruction-wire representation for [`PodString`].
///
/// The wrapper exists because zeropod's bounded string is a valid zero-copy
/// element but cannot be given the foreign [`zeropod::ZcElem`] implementation
/// from this crate.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PodStringZc<const N: usize, const PFX: usize = 1>(PodString<N, PFX>);

impl<const N: usize, const PFX: usize> zeropod::ZcValidate for PodStringZc<N, PFX> {
    #[inline(always)]
    fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
        <PodString<N, PFX> as zeropod::ZcValidate>::validate_ref(&value.0)
    }
}

// SAFETY: `PodStringZc` is transparent over zeropod's `repr(C)` sequence of
// an alignment-1 length prefix and `MaybeUninit<u8>` storage. It has no
// padding, every initialized bit pattern is a valid Rust value, and the
// delegated validator checks both the encoded length and UTF-8 payload.
unsafe impl<const N: usize, const PFX: usize> zeropod::ZcElem for PodStringZc<N, PFX> {}

impl<const N: usize, const PFX: usize> zeropod::ZcField for PodStringZc<N, PFX> {
    type Pod = Self;
    const POD_SIZE: usize = core::mem::size_of::<Self>();
}

/// Instruction-wire representation for [`PodVec`].
///
/// Like [`PodStringZc`], this supplies a local, auditable `ZcElem` boundary
/// without changing the bounded vector's wire layout.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PodVecZc<T: zeropod::ZcElem, const N: usize, const PFX: usize = 2>(PodVec<T, N, PFX>);

impl<T: zeropod::ZcElem, const N: usize, const PFX: usize> zeropod::ZcValidate
    for PodVecZc<T, N, PFX>
{
    #[inline(always)]
    fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
        <PodVec<T, N, PFX> as zeropod::ZcValidate>::validate_ref(&value.0)
    }
}

// SAFETY: `PodVecZc` is transparent over zeropod's `repr(C)` sequence of an
// alignment-1 length prefix and `MaybeUninit<T>` storage. `T: ZcElem`
// guarantees the element layout, and the delegated validator checks the
// encoded length and every active element.
unsafe impl<T: zeropod::ZcElem, const N: usize, const PFX: usize> zeropod::ZcElem
    for PodVecZc<T, N, PFX>
{
}

impl<T: zeropod::ZcElem, const N: usize, const PFX: usize> zeropod::ZcField
    for PodVecZc<T, N, PFX>
{
    type Pod = Self;
    const POD_SIZE: usize = core::mem::size_of::<Self>();
}

impl<const N: usize, const PFX: usize> InstructionArg for PodString<N, PFX> {
    type Zc = PodStringZc<N, PFX>;

    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        if <Self::Zc as zeropod::ZcValidate>::validate_ref(zc).is_ok() {
            zc.0
        } else {
            Self::default()
        }
    }

    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        // SAFETY: every all-zero bit pattern is valid for `PodStringZc` and
        // encodes an empty string. Starting from zero also initializes the
        // inactive tail before the fixed-size value is exposed as bytes.
        let mut zc = unsafe { core::mem::zeroed::<Self::Zc>() };
        let _stored = zc.0.set(self.as_str());
        zc
    }
}

impl<T: zeropod::ZcElem, const N: usize, const PFX: usize> InstructionArg for PodVec<T, N, PFX> {
    type Zc = PodVecZc<T, N, PFX>;

    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        if <Self::Zc as zeropod::ZcValidate>::validate_ref(zc).is_ok() {
            zc.0
        } else {
            Self::default()
        }
    }

    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        // SAFETY: every all-zero bit pattern is valid for `PodVecZc` and
        // encodes an empty vector. Starting from zero also initializes the
        // inactive tail before the fixed-size value is exposed as bytes.
        let mut zc = unsafe { core::mem::zeroed::<Self::Zc>() };
        let _stored = zc.0.set_from_slice(self.as_slice());
        zc
    }
}

/// Blanket `InstructionArg` for all builtin pod types via `ZcField` + `From`.
///
/// QuasarSerialize structs/enums are NOT `BuiltinPod` (sealed), so they
/// generate direct `InstructionArg` impls; no E0119 overlap.
impl<T> InstructionArg for T
where
    T: Copy + BuiltinPod + zeropod::ZcField,
    T::Pod: zeropod::ZcElem + From<T>,
    T: From<T::Pod>,
{
    type Zc = T::Pod;

    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        T::from(*zc)
    }

    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        T::Pod::from(*self)
    }

    #[inline(always)]
    fn validate_zc(zc: &Self::Zc) -> Result<(), crate::prelude::ProgramError> {
        <T::Pod as zeropod::ZcValidate>::validate_ref(zc)
            .map_err(|_| crate::prelude::ProgramError::InvalidInstructionData)
    }
}

/// Zero-copy companion for `Option<T>`.
pub type OptionZc<Z> = crate::pod::PodOption<Z>;

const _: () = assert!(core::mem::align_of::<OptionZc<[u8; 1]>>() == 1);
const _: () = assert!(core::mem::size_of::<OptionZc<[u8; 1]>>() == 2);

impl<T: InstructionArg> InstructionArg for Option<T> {
    type Zc = OptionZc<T::Zc>;

    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        // Only tag == 1 is `Some` (see `PodOption::some`). Tag 0 is `None`; any
        // other tag is invalid and rejected by `validate_zc`, but decode it as
        // `None` here so a stray tag never reaches `assume_init_ref` on a
        // possibly-uninitialized payload.
        if zc.raw_tag() == 1 {
            // SAFETY: Tag 1 is the initialized `Some` variant. Untrusted
            // instruction data must call `validate_zc` before this conversion.
            Some(T::from_zc(unsafe { zc.assume_init_ref() }))
        } else {
            None
        }
    }

    #[inline(always)]
    fn validate_zc(zc: &Self::Zc) -> Result<(), crate::prelude::ProgramError> {
        let tag = zc.raw_tag();
        if tag > 1 {
            return Err(crate::prelude::ProgramError::InvalidInstructionData);
        }
        if tag == 1 {
            // SAFETY: Tag 1 is the initialized `Some` variant.
            T::validate_zc(unsafe { zc.assume_init_ref() })?;
        }
        Ok(())
    }

    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        match self {
            None => OptionZc::none(),
            Some(v) => OptionZc::some(v.to_zc()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_u64_some_round_trip() {
        let val: Option<u64> = Some(42);
        let zc = val.to_zc();
        assert_eq!(zc.raw_tag(), 1);
        let decoded = Option::<u64>::from_zc(&zc);
        assert_eq!(decoded, Some(42));
    }

    #[test]
    fn option_u64_none_round_trip() {
        let val: Option<u64> = None;
        let zc = val.to_zc();
        assert_eq!(zc.raw_tag(), 0);
        let decoded = Option::<u64>::from_zc(&zc);
        assert_eq!(decoded, None);
    }

    #[test]
    fn option_address_some_round_trip() {
        let addr = solana_address::Address::from([42u8; 32]);
        let val: Option<solana_address::Address> = Some(addr);
        let zc = val.to_zc();
        assert_eq!(zc.raw_tag(), 1);
        let decoded = Option::<solana_address::Address>::from_zc(&zc);
        assert_eq!(decoded, Some(addr));
    }

    #[test]
    fn option_address_none_round_trip() {
        let val: Option<solana_address::Address> = None;
        let zc = val.to_zc();
        assert_eq!(zc.raw_tag(), 0);
        let decoded = Option::<solana_address::Address>::from_zc(&zc);
        assert_eq!(decoded, None);
    }

    #[test]
    fn option_zc_alignment_is_one() {
        assert_eq!(core::mem::align_of::<OptionZc<[u8; 8]>>(), 1);
        assert_eq!(core::mem::align_of::<OptionZc<[u8; 32]>>(), 1);
        assert_eq!(core::mem::align_of::<OptionZc<crate::pod::PodU64>>(), 1);
    }

    #[test]
    fn option_zc_size_is_fixed() {
        assert_eq!(
            core::mem::size_of::<OptionZc<crate::pod::PodU64>>(),
            1 + core::mem::size_of::<crate::pod::PodU64>()
        );
        assert_eq!(
            core::mem::size_of::<OptionZc<solana_address::Address>>(),
            1 + core::mem::size_of::<solana_address::Address>()
        );
    }

    fn option_zc_with_tag<Z: Copy>(tag: u8, inner: Z) -> OptionZc<Z> {
        let mut zc = OptionZc::some(inner);
        // SAFETY: The first byte of `PodOption` is its tag; tests deliberately
        // forge invalid tags to exercise validation.
        unsafe {
            *((&mut zc) as *mut OptionZc<Z> as *mut u8) = tag;
        }
        zc
    }

    #[test]
    fn option_tag_invalid_rejected() {
        let zc = option_zc_with_tag(2, crate::pod::PodU64::from(42));
        assert!(Option::<u64>::validate_zc(&zc).is_err());
    }

    #[test]
    fn option_from_zc_decodes_stray_tag_as_none() {
        // Only tag == 1 is `Some`; any other tag decodes to `None` (defense in
        // depth so a stray tag never reaches `assume_init_ref`).
        for tag in [2u8, 0x0F, 0xFF] {
            let zc = option_zc_with_tag(tag, crate::pod::PodU64::from(42));
            assert_eq!(Option::<u64>::from_zc(&zc), None, "tag {tag:#x}");
        }
    }

    #[test]
    fn option_tag_valid_accepted() {
        let none_zc = None::<u64>.to_zc();
        assert!(Option::<u64>::validate_zc(&none_zc).is_ok());
        let some_zc = Some(42u64).to_zc();
        assert!(Option::<u64>::validate_zc(&some_zc).is_ok());
    }

    #[test]
    fn option_none_payload_is_zeroed() {
        let zc = None::<u64>.to_zc();
        // SAFETY: Skip the one-byte tag and read exactly the PodU64 payload
        // bytes from the stack-local `OptionZc`.
        let bytes = unsafe {
            core::slice::from_raw_parts(
                (&zc as *const _ as *const u8).add(1),
                core::mem::size_of::<crate::pod::PodU64>(),
            )
        };
        assert!(bytes.iter().all(|&b| b == 0x00));
    }

    #[test]
    fn option_nested_round_trip() {
        let some_some: Option<Option<u64>> = Some(Some(42));
        let zc = some_some.to_zc();
        assert_eq!(Option::<Option<u64>>::from_zc(&zc), Some(Some(42)));

        let some_none: Option<Option<u64>> = Some(None);
        let zc = some_none.to_zc();
        assert_eq!(Option::<Option<u64>>::from_zc(&zc), Some(None));

        let none: Option<Option<u64>> = None;
        let zc = none.to_zc();
        assert_eq!(Option::<Option<u64>>::from_zc(&zc), None);
    }

    #[test]
    fn option_nested_size() {
        assert_eq!(
            core::mem::size_of::<OptionZc<OptionZc<crate::pod::PodU64>>>(),
            10,
        );
    }

    #[test]
    fn option_nested_validate_outer_invalid() {
        let zc = option_zc_with_tag(3, Some(42u64).to_zc());
        assert!(Option::<Option<u64>>::validate_zc(&zc).is_err());
    }

    #[test]
    fn option_nested_validate_both_valid() {
        let some_some = Some(Some(42u64)).to_zc();
        assert!(Option::<Option<u64>>::validate_zc(&some_some).is_ok());
        let some_none = Some(None::<u64>).to_zc();
        assert!(Option::<Option<u64>>::validate_zc(&some_none).is_ok());
        let none = None::<Option<u64>>.to_zc();
        assert!(Option::<Option<u64>>::validate_zc(&none).is_ok());
    }

    #[test]
    fn validate_zc_noop_for_primitives() {
        assert!(u64::validate_zc(&crate::pod::PodU64::from(42)).is_ok());
        assert!(u8::validate_zc(&0u8).is_ok());
        assert!(bool::validate_zc(&crate::pod::PodBool::from(true)).is_ok());
    }

    #[test]
    fn option_validate_all_boundary_tags() {
        for tag in 0..=1u8 {
            let zc = option_zc_with_tag(tag, crate::pod::PodU64::from(0));
            assert!(
                Option::<u64>::validate_zc(&zc).is_ok(),
                "tag={tag} should be valid"
            );
        }
        for tag in 2..=255u8 {
            let zc = option_zc_with_tag(tag, crate::pod::PodU64::from(0));
            assert!(
                Option::<u64>::validate_zc(&zc).is_err(),
                "tag={tag} should be invalid"
            );
        }
    }
}

#[cfg(kani)]
#[path = "../kani/instruction_arg.rs"]
mod kani_proofs;
