//! Trait for types that can be used as fixed-size instruction arguments.
//!
//! Each type provides an alignment-1 zero-copy companion (`Zc`) and a
//! conversion function (`from_zc`) used by `#[instruction]` codegen.
//! Primitive integers map to their Pod equivalents (e.g. `u64` → `PodU64`),
//! while custom structs derive their companion via
//! `#[derive(QuasarSerialize)]`.

use crate::pod::*;

/// A type that can appear as a fixed-size `#[instruction]` argument.
///
/// The associated `Zc` type must be `#[repr(C)]` with alignment 1 so that
/// the instruction data ZC struct can be read via zero-copy pointer cast
/// from `&[u8]`.
pub trait InstructionArg: Sized {
    /// The alignment-1 companion type for zero-copy deserialization.
    type Zc: Copy;
    /// Reconstruct the native value from its ZC representation.
    fn from_zc(zc: &Self::Zc) -> Self;
    /// Convert the native value into its alignment-1 ZC representation.
    fn to_zc(&self) -> Self::Zc;

    /// Validate the raw ZC bytes before calling `from_zc`.
    ///
    /// Called by `#[instruction]` codegen on untrusted instruction data.
    /// The default is a no-op. Override for types with validity constraints
    /// on their ZC representation (e.g. `Option<T>` rejects tag values > 1).
    #[inline(always)]
    fn validate_zc(_zc: &Self::Zc) -> Result<(), crate::prelude::ProgramError> {
        Ok(())
    }
}

// --- Identity impls (already alignment 1) ---

impl InstructionArg for u8 {
    type Zc = u8;
    #[inline(always)]
    fn from_zc(zc: &u8) -> u8 {
        *zc
    }
    #[inline(always)]
    fn to_zc(&self) -> u8 {
        *self
    }
}

impl InstructionArg for i8 {
    type Zc = i8;
    #[inline(always)]
    fn from_zc(zc: &i8) -> i8 {
        *zc
    }
    #[inline(always)]
    fn to_zc(&self) -> i8 {
        *self
    }
}

impl<const N: usize> InstructionArg for [u8; N] {
    type Zc = [u8; N];
    #[inline(always)]
    fn from_zc(zc: &[u8; N]) -> [u8; N] {
        *zc
    }
    #[inline(always)]
    fn to_zc(&self) -> [u8; N] {
        *self
    }
}

impl InstructionArg for solana_address::Address {
    type Zc = solana_address::Address;
    #[inline(always)]
    fn from_zc(zc: &solana_address::Address) -> solana_address::Address {
        *zc
    }
    #[inline(always)]
    fn to_zc(&self) -> solana_address::Address {
        *self
    }
}

// --- Pod-mapped impls (native → Pod companion) ---

macro_rules! impl_instruction_arg_pod {
    ($native:ty, $pod:ty) => {
        impl InstructionArg for $native {
            type Zc = $pod;
            #[inline(always)]
            fn from_zc(zc: &$pod) -> $native {
                zc.get()
            }
            #[inline(always)]
            fn to_zc(&self) -> $pod {
                <$pod>::from(*self)
            }
        }
    };
}

impl_instruction_arg_pod!(u16, PodU16);
impl_instruction_arg_pod!(u32, PodU32);
impl_instruction_arg_pod!(u64, PodU64);
impl_instruction_arg_pod!(u128, PodU128);
impl_instruction_arg_pod!(i16, PodI16);
impl_instruction_arg_pod!(i32, PodI32);
impl_instruction_arg_pod!(i64, PodI64);
impl_instruction_arg_pod!(i128, PodI128);

impl InstructionArg for bool {
    type Zc = PodBool;
    #[inline(always)]
    fn from_zc(zc: &PodBool) -> bool {
        zc.get()
    }
    #[inline(always)]
    fn to_zc(&self) -> PodBool {
        PodBool::from(*self)
    }
}

// --- Pod types map to themselves ---

macro_rules! impl_instruction_arg_identity {
    ($($t:ty),*) => {$(
        impl InstructionArg for $t {
            type Zc = $t;
            #[inline(always)]
            fn from_zc(zc: &$t) -> $t { *zc }
            #[inline(always)]
            fn to_zc(&self) -> $t { *self }
        }
    )*}
}

impl_instruction_arg_identity!(
    PodU16, PodU32, PodU64, PodU128, PodI16, PodI32, PodI64, PodI128, PodBool
);

// --- PodString / PodVec: identity InstructionArg (Zc = Self) ---

impl<const N: usize, const PFX: usize> InstructionArg for crate::pod::PodString<N, PFX> {
    type Zc = Self;
    #[inline(always)]
    fn from_zc(zc: &Self) -> Self {
        *zc
    }
    #[inline(always)]
    fn to_zc(&self) -> Self {
        *self
    }
    #[inline(always)]
    fn validate_zc(zc: &Self) -> Result<(), crate::prelude::ProgramError> {
        if zc.decode_len() > N {
            return Err(crate::prelude::ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}

impl<T: Copy, const N: usize, const PFX: usize> InstructionArg for crate::pod::PodVec<T, N, PFX> {
    type Zc = Self;
    #[inline(always)]
    fn from_zc(zc: &Self) -> Self {
        *zc
    }
    #[inline(always)]
    fn to_zc(&self) -> Self {
        *self
    }
    #[inline(always)]
    fn validate_zc(zc: &Self) -> Result<(), crate::prelude::ProgramError> {
        if zc.decode_len() > N {
            return Err(crate::prelude::ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}

// --- InstructionArgDecode<'a>: unified decode for fixed and borrowed args ---

/// Unified decode trait for instruction arguments.
///
/// Fixed types (`T: InstructionArg`) get a blanket impl that uses the ZC
/// pointer-cast path. Borrowed structs with `&'a str` / `&'a [T]` fields get an
/// explicit impl generated by `#[derive(QuasarSerialize)]`.
///
/// The `Output` associated type decouples the decoded form from the static
/// type: fixed types decode to `Self`, borrowed structs decode to `Self<'a>`.
pub trait InstructionArgDecode<'a>: Sized {
    type Output: 'a;
    fn decode_from_cursor(
        cursor: &mut crate::instruction_data::InstructionCursor<'a>,
    ) -> Result<Self::Output, crate::prelude::ProgramError>;

    #[inline(always)]
    fn decode(
        data: &'a [u8],
        offset: usize,
    ) -> Result<(Self::Output, usize), crate::prelude::ProgramError> {
        let mut cursor = crate::instruction_data::InstructionCursor::with_offset(data, offset);
        let value = Self::decode_from_cursor(&mut cursor)?;
        Ok((value, cursor.offset()))
    }
}

/// Blanket impl: all fixed-size `InstructionArg` types decode via ZC
/// pointer-cast.
impl<'a, T: InstructionArg + 'a> InstructionArgDecode<'a> for T {
    type Output = T;
    #[inline(always)]
    fn decode_from_cursor(
        cursor: &mut crate::instruction_data::InstructionCursor<'a>,
    ) -> Result<T, crate::prelude::ProgramError> {
        cursor.read_arg::<T>()
    }
}

// --- Option<T> blanket impl ---

/// Zero-copy companion for `Option<T>`.
///
/// Tag byte (0 = None, 1 = Some) followed by the inner ZC value.
/// For None, payload bytes are zeroed but wrapped in `MaybeUninit`
/// to avoid soundness issues with types that have validity constraints.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct OptionZc<Z: Copy> {
    pub tag: u8,
    pub value: core::mem::MaybeUninit<Z>,
}

// Compile-time alignment and size checks.
const _: () = assert!(core::mem::align_of::<OptionZc<[u8; 1]>>() == 1);
const _: () = assert!(core::mem::size_of::<OptionZc<[u8; 1]>>() == 2);

impl<T: InstructionArg> InstructionArg for Option<T> {
    type Zc = OptionZc<T::Zc>;

    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        if zc.tag == 0 {
            None
        } else {
            // SAFETY: tag was validated as 0 or 1 by validate_zc() (called by
            // codegen before from_zc). Tag == 1 means value was written by
            // to_zc() or populated by the SVM instruction data buffer.
            Some(T::from_zc(unsafe { zc.value.assume_init_ref() }))
        }
    }

    /// Reject tag values other than 0 (None) or 1 (Some), and recurse
    /// into `T::validate_zc` when the value is present.
    #[inline(always)]
    fn validate_zc(zc: &Self::Zc) -> Result<(), crate::prelude::ProgramError> {
        if zc.tag > 1 {
            return Err(crate::prelude::ProgramError::InvalidInstructionData);
        }
        if zc.tag == 1 {
            // SAFETY: tag == 1 means the value was written by to_zc() or
            // populated by the SVM instruction data buffer.
            T::validate_zc(unsafe { zc.value.assume_init_ref() })?;
        }
        Ok(())
    }

    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        match self {
            None => OptionZc {
                tag: 0,
                // MaybeUninit::zeroed() -- payload is never read when tag == 0.
                // Zeroed for determinism in serialized instruction data.
                value: core::mem::MaybeUninit::zeroed(),
            },
            Some(v) => OptionZc {
                tag: 1,
                value: core::mem::MaybeUninit::new(v.to_zc()),
            },
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
        assert_eq!(zc.tag, 1);
        let decoded = Option::<u64>::from_zc(&zc);
        assert_eq!(decoded, Some(42));
    }

    #[test]
    fn option_u64_none_round_trip() {
        let val: Option<u64> = None;
        let zc = val.to_zc();
        assert_eq!(zc.tag, 0);
        let decoded = Option::<u64>::from_zc(&zc);
        assert_eq!(decoded, None);
    }

    #[test]
    fn option_address_some_round_trip() {
        let addr = solana_address::Address::from([42u8; 32]);
        let val: Option<solana_address::Address> = Some(addr);
        let zc = val.to_zc();
        assert_eq!(zc.tag, 1);
        let decoded = Option::<solana_address::Address>::from_zc(&zc);
        assert_eq!(decoded, Some(addr));
    }

    #[test]
    fn option_address_none_round_trip() {
        let val: Option<solana_address::Address> = None;
        let zc = val.to_zc();
        assert_eq!(zc.tag, 0);
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
        // OptionZc<PodU64> = 1 (tag) + 8 (MaybeUninit<PodU64>) = 9
        assert_eq!(
            core::mem::size_of::<OptionZc<crate::pod::PodU64>>(),
            1 + core::mem::size_of::<crate::pod::PodU64>()
        );
        // OptionZc<Address> = 1 (tag) + 32 (MaybeUninit<Address>) = 33
        assert_eq!(
            core::mem::size_of::<OptionZc<solana_address::Address>>(),
            1 + core::mem::size_of::<solana_address::Address>()
        );
    }

    #[test]
    fn option_tag_invalid_rejected() {
        let zc = OptionZc {
            tag: 2,
            value: core::mem::MaybeUninit::new(crate::pod::PodU64::from(42)),
        };
        assert!(Option::<u64>::validate_zc(&zc).is_err());
    }

    #[test]
    fn option_tag_0xff_rejected() {
        let zc = OptionZc {
            tag: 0xFF,
            value: core::mem::MaybeUninit::new(crate::pod::PodU64::from(42)),
        };
        assert!(Option::<u64>::validate_zc(&zc).is_err());
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
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &zc.value as *const _ as *const u8,
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
        // OptionZc<OptionZc<PodU64>> = 1 (outer tag) + 1 (inner tag) + 8 (PodU64) = 10
        assert_eq!(
            core::mem::size_of::<OptionZc<OptionZc<crate::pod::PodU64>>>(),
            10,
        );
    }

    #[test]
    fn option_nested_validate_outer_invalid() {
        // Outer tag invalid, inner valid
        let zc = OptionZc {
            tag: 3,
            value: core::mem::MaybeUninit::new(Some(42u64).to_zc()),
        };
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
        // Primitives always pass validation (default no-op)
        assert!(u64::validate_zc(&crate::pod::PodU64::from(42)).is_ok());
        assert!(u8::validate_zc(&0u8).is_ok());
        assert!(bool::validate_zc(&crate::pod::PodBool::from(true)).is_ok());
    }

    #[test]
    fn option_validate_all_boundary_tags() {
        // Tag 0 and 1 are valid
        for tag in 0..=1u8 {
            let zc = OptionZc {
                tag,
                value: core::mem::MaybeUninit::new(crate::pod::PodU64::from(0)),
            };
            assert!(
                Option::<u64>::validate_zc(&zc).is_ok(),
                "tag={tag} should be valid"
            );
        }
        // Tags 2..=255 are invalid
        for tag in 2..=255u8 {
            let zc = OptionZc {
                tag,
                value: core::mem::MaybeUninit::new(crate::pod::PodU64::from(0)),
            };
            assert!(
                Option::<u64>::validate_zc(&zc).is_err(),
                "tag={tag} should be invalid"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove `validate_zc` accepts tags 0 and 1 and rejects all others,
    /// for any symbolic tag byte.
    #[kani::proof]
    fn option_validate_zc_tag_boundary() {
        let tag: u8 = kani::any();
        let zc = OptionZc {
            tag,
            value: core::mem::MaybeUninit::new(PodU64::from(0u64)),
        };
        let result = Option::<u64>::validate_zc(&zc);
        assert!(result.is_ok() == (tag <= 1));
    }

    /// Prove the `Option<u64>` roundtrip: to_zc then from_zc preserves
    /// the value for all symbolic inputs.
    #[kani::proof]
    fn option_roundtrip_some() {
        let v: u64 = kani::any();
        let opt = Some(v);
        let zc = opt.to_zc();
        assert!(Option::<u64>::validate_zc(&zc).is_ok());
        let decoded = Option::<u64>::from_zc(&zc);
        assert!(decoded == Some(v));
    }

    #[kani::proof]
    fn option_roundtrip_none() {
        let opt: Option<u64> = None;
        let zc = opt.to_zc();
        assert!(Option::<u64>::validate_zc(&zc).is_ok());
        let decoded = Option::<u64>::from_zc(&zc);
        assert!(decoded.is_none());
    }

    /// Prove that `InstructionArg` roundtrip (to_zc then from_zc) is
    /// correct for all u64 values.
    #[kani::proof]
    fn instruction_arg_u64_roundtrip() {
        let v: u64 = kani::any();
        let zc = v.to_zc();
        let decoded = u64::from_zc(&zc);
        assert!(decoded == v);
    }

    /// Prove that `InstructionArg` roundtrip is correct for bool.
    #[kani::proof]
    fn instruction_arg_bool_roundtrip() {
        let v: bool = kani::any();
        let zc = v.to_zc();
        let decoded = bool::from_zc(&zc);
        assert!(decoded == v);
    }
}
