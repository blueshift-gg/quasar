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
            // SAFETY: tag != 0 means the value was written by to_zc()
            // or populated by the SVM instruction data buffer (fully initialized).
            Some(T::from_zc(unsafe { zc.value.assume_init_ref() }))
        }
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
}
