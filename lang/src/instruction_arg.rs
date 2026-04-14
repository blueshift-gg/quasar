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

    /// Reject tag values other than 0 (None) or 1 (Some).
    #[inline(always)]
    fn validate_zc(zc: &Self::Zc) -> Result<(), crate::prelude::ProgramError> {
        if zc.tag > 1 {
            return Err(crate::prelude::ProgramError::InvalidInstructionData);
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
