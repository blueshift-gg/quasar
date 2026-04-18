//! Alignment-1 Pod integer types for zero-copy account access.

use core::fmt;

macro_rules! define_pod_unsigned {
    ($name:ident, $native:ty, $size:expr) => {
        define_pod_common!($name, $native, $size);
        define_pod_arithmetic!($name, $native);
    };
}

macro_rules! define_pod_signed {
    ($name:ident, $native:ty, $size:expr) => {
        define_pod_common!($name, $native, $size);
        define_pod_arithmetic!($name, $native);

        impl core::ops::Neg for $name {
            type Output = Self;
            #[inline(always)]
            fn neg(self) -> Self {
                #[cfg(debug_assertions)]
                {
                    Self::from(
                        self.get()
                            .checked_neg()
                            .expect("attempt to negate with overflow"),
                    )
                }
                #[cfg(not(debug_assertions))]
                {
                    Self::from(self.get().wrapping_neg())
                }
            }
        }
    };
}

macro_rules! define_pod_common {
    ($name:ident, $native:ty, $size:expr) => {
        #[repr(transparent)]
        #[derive(Copy, Clone, Default)]
        #[cfg_attr(feature = "wincode", derive(wincode::SchemaWrite, wincode::SchemaRead))]
        pub struct $name([u8; $size]);

        impl $name {
            /// The zero value.
            pub const ZERO: Self = Self([0u8; $size]);

            pub const MAX: Self = Self(<$native>::MAX.to_le_bytes());

            pub const MIN: Self = Self(<$native>::MIN.to_le_bytes());

            #[inline(always)]
            pub fn get(&self) -> $native {
                <$native>::from_le_bytes(self.0)
            }

            /// Returns `true` if the value is zero.
            #[inline(always)]
            pub fn is_zero(&self) -> bool {
                self.0 == [0u8; $size]
            }

            /// Checked addition. Returns `None` on overflow.
            #[inline(always)]
            pub fn checked_add(self, rhs: impl Into<$name>) -> Option<Self> {
                self.get().checked_add(rhs.into().get()).map(Self::from)
            }

            /// Checked subtraction. Returns `None` on underflow.
            #[inline(always)]
            pub fn checked_sub(self, rhs: impl Into<$name>) -> Option<Self> {
                self.get().checked_sub(rhs.into().get()).map(Self::from)
            }

            /// Checked multiplication. Returns `None` on overflow.
            #[inline(always)]
            pub fn checked_mul(self, rhs: impl Into<$name>) -> Option<Self> {
                self.get().checked_mul(rhs.into().get()).map(Self::from)
            }

            /// Checked division. Returns `None` if `rhs` is zero.
            #[inline(always)]
            pub fn checked_div(self, rhs: impl Into<$name>) -> Option<Self> {
                self.get().checked_div(rhs.into().get()).map(Self::from)
            }

            /// Saturating addition. Clamps at the numeric bounds instead of
            /// overflowing.
            #[inline(always)]
            pub fn saturating_add(self, rhs: impl Into<$name>) -> Self {
                Self::from(self.get().saturating_add(rhs.into().get()))
            }

            /// Saturating subtraction. Clamps at zero (for unsigned) or the numeric
            /// bound (for signed).
            #[inline(always)]
            pub fn saturating_sub(self, rhs: impl Into<$name>) -> Self {
                Self::from(self.get().saturating_sub(rhs.into().get()))
            }

            /// Saturating multiplication. Clamps at the numeric bounds instead of
            /// overflowing.
            #[inline(always)]
            pub fn saturating_mul(self, rhs: impl Into<$name>) -> Self {
                Self::from(self.get().saturating_mul(rhs.into().get()))
            }

            /// Sets the value in place.
            #[inline(always)]
            pub fn set(&mut self, value: $native) {
                self.0 = value.to_le_bytes();
            }

            /// Wrapping addition. Wraps around on overflow.
            #[inline(always)]
            pub fn wrapping_add(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().wrapping_add(rhs.into().get()))
            }

            /// Wrapping subtraction. Wraps around on underflow.
            #[inline(always)]
            pub fn wrapping_sub(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().wrapping_sub(rhs.into().get()))
            }

            /// Wrapping multiplication. Wraps around on overflow.
            #[inline(always)]
            pub fn wrapping_mul(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().wrapping_mul(rhs.into().get()))
            }
        }

        impl core::hash::Hash for $name {
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                self.get().hash(state);
            }
        }

        impl fmt::Binary for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Binary::fmt(&self.get(), f)
            }
        }

        impl fmt::LowerHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::LowerHex::fmt(&self.get(), f)
            }
        }

        impl fmt::UpperHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::UpperHex::fmt(&self.get(), f)
            }
        }

        impl From<$native> for $name {
            #[inline(always)]
            fn from(v: $native) -> Self {
                Self(v.to_le_bytes())
            }
        }

        impl From<$name> for $native {
            #[inline(always)]
            fn from(v: $name) -> Self {
                v.get()
            }
        }

        impl PartialEq for $name {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl Eq for $name {}

        impl PartialEq<$native> for $name {
            #[inline(always)]
            fn eq(&self, other: &$native) -> bool {
                self.get() == *other
            }
        }

        impl PartialOrd for $name {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.get().cmp(&other.get())
            }
        }

        impl PartialOrd<$native> for $name {
            #[inline(always)]
            fn partial_cmp(&self, other: &$native) -> Option<core::cmp::Ordering> {
                self.get().partial_cmp(other)
            }
        }

        // --- Reverse-direction: native vs Pod ---

        impl PartialEq<$name> for $native {
            #[inline(always)]
            fn eq(&self, other: &$name) -> bool {
                *self == other.get()
            }
        }

        impl PartialOrd<$name> for $native {
            #[inline(always)]
            fn partial_cmp(&self, other: &$name) -> Option<core::cmp::Ordering> {
                self.partial_cmp(&other.get())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.get().fmt(f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.get(), f)
            }
        }
    };
}

macro_rules! define_pod_arithmetic {
    ($name:ident, $native:ty) => {
        // --- Pod + native ---

        impl core::ops::Add<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn add(self, rhs: $native) -> Self {
                #[cfg(debug_assertions)]
                {
                    Self::from(
                        self.get()
                            .checked_add(rhs)
                            .expect("attempt to add with overflow"),
                    )
                }
                #[cfg(not(debug_assertions))]
                {
                    Self::from(self.get().wrapping_add(rhs))
                }
            }
        }

        impl core::ops::Sub<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn sub(self, rhs: $native) -> Self {
                #[cfg(debug_assertions)]
                {
                    Self::from(
                        self.get()
                            .checked_sub(rhs)
                            .expect("attempt to subtract with overflow"),
                    )
                }
                #[cfg(not(debug_assertions))]
                {
                    Self::from(self.get().wrapping_sub(rhs))
                }
            }
        }

        impl core::ops::Mul<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn mul(self, rhs: $native) -> Self {
                #[cfg(debug_assertions)]
                {
                    Self::from(
                        self.get()
                            .checked_mul(rhs)
                            .expect("attempt to multiply with overflow"),
                    )
                }
                #[cfg(not(debug_assertions))]
                {
                    Self::from(self.get().wrapping_mul(rhs))
                }
            }
        }

        impl core::ops::Div<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn div(self, rhs: $native) -> Self {
                Self::from(self.get() / rhs)
            }
        }

        impl core::ops::Rem<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn rem(self, rhs: $native) -> Self {
                Self::from(self.get() % rhs)
            }
        }

        // --- native + Pod (reverse direction) ---

        impl core::ops::Add<$name> for $native {
            type Output = $name;
            #[inline(always)]
            fn add(self, rhs: $name) -> $name {
                rhs + self
            }
        }

        impl core::ops::Sub<$name> for $native {
            type Output = $name;
            #[inline(always)]
            fn sub(self, rhs: $name) -> $name {
                #[cfg(debug_assertions)]
                {
                    $name::from(
                        self.checked_sub(rhs.get())
                            .expect("attempt to subtract with overflow"),
                    )
                }
                #[cfg(not(debug_assertions))]
                {
                    $name::from(self.wrapping_sub(rhs.get()))
                }
            }
        }

        impl core::ops::Mul<$name> for $native {
            type Output = $name;
            #[inline(always)]
            fn mul(self, rhs: $name) -> $name {
                rhs * self
            }
        }

        impl core::ops::Div<$name> for $native {
            type Output = $name;
            #[inline(always)]
            fn div(self, rhs: $name) -> $name {
                $name::from(self / rhs.get())
            }
        }

        impl core::ops::Rem<$name> for $native {
            type Output = $name;
            #[inline(always)]
            fn rem(self, rhs: $name) -> $name {
                $name::from(self % rhs.get())
            }
        }

        // --- Pod + Pod ---

        impl core::ops::Add for $name {
            type Output = Self;
            #[inline(always)]
            fn add(self, rhs: Self) -> Self {
                self + rhs.get()
            }
        }

        impl core::ops::Sub for $name {
            type Output = Self;
            #[inline(always)]
            fn sub(self, rhs: Self) -> Self {
                self - rhs.get()
            }
        }

        impl core::ops::Mul for $name {
            type Output = Self;
            #[inline(always)]
            fn mul(self, rhs: Self) -> Self {
                self * rhs.get()
            }
        }

        impl core::ops::Div for $name {
            type Output = Self;
            #[inline(always)]
            fn div(self, rhs: Self) -> Self {
                self / rhs.get()
            }
        }

        impl core::ops::Rem for $name {
            type Output = Self;
            #[inline(always)]
            fn rem(self, rhs: Self) -> Self {
                self % rhs.get()
            }
        }

        // --- Assign with native ---

        impl core::ops::AddAssign<$native> for $name {
            #[inline(always)]
            fn add_assign(&mut self, rhs: $native) {
                *self = *self + rhs;
            }
        }

        impl core::ops::SubAssign<$native> for $name {
            #[inline(always)]
            fn sub_assign(&mut self, rhs: $native) {
                *self = *self - rhs;
            }
        }

        impl core::ops::MulAssign<$native> for $name {
            #[inline(always)]
            fn mul_assign(&mut self, rhs: $native) {
                *self = *self * rhs;
            }
        }

        impl core::ops::DivAssign<$native> for $name {
            #[inline(always)]
            fn div_assign(&mut self, rhs: $native) {
                *self = *self / rhs;
            }
        }

        impl core::ops::RemAssign<$native> for $name {
            #[inline(always)]
            fn rem_assign(&mut self, rhs: $native) {
                *self = *self % rhs;
            }
        }

        // --- Assign with Pod ---

        impl core::ops::AddAssign for $name {
            #[inline(always)]
            fn add_assign(&mut self, rhs: Self) {
                *self = *self + rhs;
            }
        }

        impl core::ops::SubAssign for $name {
            #[inline(always)]
            fn sub_assign(&mut self, rhs: Self) {
                *self = *self - rhs;
            }
        }

        impl core::ops::MulAssign for $name {
            #[inline(always)]
            fn mul_assign(&mut self, rhs: Self) {
                *self = *self * rhs;
            }
        }

        impl core::ops::DivAssign for $name {
            #[inline(always)]
            fn div_assign(&mut self, rhs: Self) {
                *self = *self / rhs;
            }
        }

        impl core::ops::RemAssign for $name {
            #[inline(always)]
            fn rem_assign(&mut self, rhs: Self) {
                *self = *self % rhs;
            }
        }

        // --- Bitwise ---

        impl core::ops::BitAnd<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn bitand(self, rhs: $native) -> Self {
                Self::from(self.get() & rhs)
            }
        }

        impl core::ops::BitOr<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn bitor(self, rhs: $native) -> Self {
                Self::from(self.get() | rhs)
            }
        }

        impl core::ops::BitXor<$native> for $name {
            type Output = Self;
            #[inline(always)]
            fn bitxor(self, rhs: $native) -> Self {
                Self::from(self.get() ^ rhs)
            }
        }

        impl core::ops::BitAnd for $name {
            type Output = Self;
            #[inline(always)]
            fn bitand(self, rhs: Self) -> Self {
                self & rhs.get()
            }
        }

        impl core::ops::BitOr for $name {
            type Output = Self;
            #[inline(always)]
            fn bitor(self, rhs: Self) -> Self {
                self | rhs.get()
            }
        }

        impl core::ops::BitXor for $name {
            type Output = Self;
            #[inline(always)]
            fn bitxor(self, rhs: Self) -> Self {
                self ^ rhs.get()
            }
        }

        // --- Bitwise assign with native ---

        impl core::ops::BitAndAssign<$native> for $name {
            #[inline(always)]
            fn bitand_assign(&mut self, rhs: $native) {
                *self = *self & rhs;
            }
        }

        impl core::ops::BitOrAssign<$native> for $name {
            #[inline(always)]
            fn bitor_assign(&mut self, rhs: $native) {
                *self = *self | rhs;
            }
        }

        impl core::ops::BitXorAssign<$native> for $name {
            #[inline(always)]
            fn bitxor_assign(&mut self, rhs: $native) {
                *self = *self ^ rhs;
            }
        }

        // --- Bitwise assign with Pod ---

        impl core::ops::BitAndAssign for $name {
            #[inline(always)]
            fn bitand_assign(&mut self, rhs: Self) {
                *self = *self & rhs;
            }
        }

        impl core::ops::BitOrAssign for $name {
            #[inline(always)]
            fn bitor_assign(&mut self, rhs: Self) {
                *self = *self | rhs;
            }
        }

        impl core::ops::BitXorAssign for $name {
            #[inline(always)]
            fn bitxor_assign(&mut self, rhs: Self) {
                *self = *self ^ rhs;
            }
        }

        // --- Shift assign ---

        impl core::ops::ShlAssign<u32> for $name {
            #[inline(always)]
            fn shl_assign(&mut self, rhs: u32) {
                *self = *self << rhs;
            }
        }

        impl core::ops::ShrAssign<u32> for $name {
            #[inline(always)]
            fn shr_assign(&mut self, rhs: u32) {
                *self = *self >> rhs;
            }
        }

        impl core::ops::Shl<u32> for $name {
            type Output = Self;
            #[inline(always)]
            fn shl(self, rhs: u32) -> Self {
                Self::from(self.get() << rhs)
            }
        }

        impl core::ops::Shr<u32> for $name {
            type Output = Self;
            #[inline(always)]
            fn shr(self, rhs: u32) -> Self {
                Self::from(self.get() >> rhs)
            }
        }

        impl core::ops::Not for $name {
            type Output = Self;
            #[inline(always)]
            fn not(self) -> Self {
                Self::from(!self.get())
            }
        }
    };
}

define_pod_unsigned!(PodU128, u128, 16);
define_pod_unsigned!(PodU64, u64, 8);
define_pod_unsigned!(PodU32, u32, 4);
define_pod_unsigned!(PodU16, u16, 2);
define_pod_signed!(PodI128, i128, 16);
define_pod_signed!(PodI64, i64, 8);
define_pod_signed!(PodI32, i32, 4);
define_pod_signed!(PodI16, i16, 2);

// Compile-time invariant: all Pod types must have alignment 1 and correct size.
const _: () = assert!(core::mem::align_of::<PodU128>() == 1);
const _: () = assert!(core::mem::size_of::<PodU128>() == 16);
const _: () = assert!(core::mem::align_of::<PodU64>() == 1);
const _: () = assert!(core::mem::size_of::<PodU64>() == 8);
const _: () = assert!(core::mem::align_of::<PodU32>() == 1);
const _: () = assert!(core::mem::size_of::<PodU32>() == 4);
const _: () = assert!(core::mem::align_of::<PodU16>() == 1);
const _: () = assert!(core::mem::size_of::<PodU16>() == 2);
const _: () = assert!(core::mem::align_of::<PodI128>() == 1);
const _: () = assert!(core::mem::size_of::<PodI128>() == 16);
const _: () = assert!(core::mem::align_of::<PodI64>() == 1);
const _: () = assert!(core::mem::size_of::<PodI64>() == 8);
const _: () = assert!(core::mem::align_of::<PodI32>() == 1);
const _: () = assert!(core::mem::size_of::<PodI32>() == 4);
const _: () = assert!(core::mem::align_of::<PodI16>() == 1);
const _: () = assert!(core::mem::size_of::<PodI16>() == 2);

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    // Core Pod proofs: roundtrip (byte encoding <-> native), Ord consistency,
    // and is_zero. Every numeric Pod type gets these via kani_pod_core!.
    macro_rules! kani_pod_core {
        ($pod:ident, $native:ty, $mod_name:ident) => {
            mod $mod_name {
                use super::super::*;

                #[kani::proof]
                fn roundtrip() {
                    let v: $native = kani::any();
                    let pod = $pod::from(v);
                    assert!(pod.get() == v, "roundtrip must preserve value");
                }

                #[kani::proof]
                fn cmp_consistency() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let pa = $pod::from(a);
                    let pb = $pod::from(b);
                    assert!((pa < pb) == (a < b), "ordering must match native");
                    assert!((pa == pb) == (a == b), "equality must match native");
                    assert!((pa > pb) == (a > b), "ordering must match native");
                }

                #[kani::proof]
                fn is_zero_correctness() {
                    let v: $native = kani::any();
                    assert!(
                        $pod::from(v).is_zero() == (v == 0),
                        "is_zero must match native zero check"
                    );
                }
            }
        };
    }

    kani_pod_core!(PodU16, u16, pod_u16);
    kani_pod_core!(PodU32, u32, pod_u32);
    kani_pod_core!(PodU64, u64, pod_u64);
    kani_pod_core!(PodI16, i16, pod_i16);
    kani_pod_core!(PodI32, i32, pod_i32);
    kani_pod_core!(PodI64, i64, pod_i64);

    // 128-bit core proofs use z3 for cmp/is_zero (CaDiCaL is slow on
    // wide comparisons).
    mod pod_u128 {
        use super::super::*;

        #[kani::proof]
        fn roundtrip() {
            let v: u128 = kani::any();
            let pod = PodU128::from(v);
            assert!(pod.get() == v, "roundtrip must preserve value");
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn cmp_consistency() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            let pa = PodU128::from(a);
            let pb = PodU128::from(b);
            assert!((pa < pb) == (a < b), "ordering must match native");
            assert!((pa == pb) == (a == b), "equality must match native");
            assert!((pa > pb) == (a > b), "ordering must match native");
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn is_zero_correctness() {
            let v: u128 = kani::any();
            assert!(
                PodU128::from(v).is_zero() == (v == 0),
                "is_zero must match native zero check"
            );
        }
    }

    mod pod_i128 {
        use super::super::*;

        #[kani::proof]
        fn roundtrip() {
            let v: i128 = kani::any();
            let pod = PodI128::from(v);
            assert!(pod.get() == v, "roundtrip must preserve value");
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn cmp_consistency() {
            let a: i128 = kani::any();
            let b: i128 = kani::any();
            let pa = PodI128::from(a);
            let pb = PodI128::from(b);
            assert!((pa < pb) == (a < b), "ordering must match native");
            assert!((pa == pb) == (a == b), "equality must match native");
            assert!((pa > pb) == (a > b), "ordering must match native");
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn is_zero_correctness() {
            let v: i128 = kani::any();
            assert!(
                PodI128::from(v).is_zero() == (v == 0),
                "is_zero must match native zero check"
            );
        }
    }

    // Operator proofs (arithmetic, bitwise, assign) for unsigned types.
    // Kani compiles with debug_assertions, so +/-/* panic on overflow.
    macro_rules! kani_operator_proofs_for {
        ($pod:ident, $native:ty, $mod_name:ident) => {
            mod $mod_name {
                use super::super::*;

                #[kani::proof]
                fn add_no_overflow() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    kani::assume(a.checked_add(b).is_some());
                    let result = ($pod::from(a) + $pod::from(b)).get();
                    assert!(result == a + b);
                }

                #[kani::proof]
                fn sub_no_overflow() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    kani::assume(a.checked_sub(b).is_some());
                    let result = ($pod::from(a) - $pod::from(b)).get();
                    assert!(result == a - b);
                }

                #[kani::proof]
                fn bitand_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    assert!(($pod::from(a) & $pod::from(b)).get() == (a & b));
                }

                #[kani::proof]
                fn bitor_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    assert!(($pod::from(a) | $pod::from(b)).get() == (a | b));
                }

                #[kani::proof]
                fn bitxor_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    assert!(($pod::from(a) ^ $pod::from(b)).get() == (a ^ b));
                }

                #[kani::proof]
                fn not_matches_native() {
                    let a: $native = kani::any();
                    assert!((!$pod::from(a)).get() == !a);
                }

                #[kani::proof]
                fn shl_matches_native() {
                    let a: $native = kani::any();
                    let rhs: u32 = kani::any();
                    kani::assume(rhs < <$native>::BITS);
                    assert!(($pod::from(a) << rhs).get() == (a << rhs));
                }

                #[kani::proof]
                fn shr_matches_native() {
                    let a: $native = kani::any();
                    let rhs: u32 = kani::any();
                    kani::assume(rhs < <$native>::BITS);
                    assert!(($pod::from(a) >> rhs).get() == (a >> rhs));
                }

                #[kani::proof]
                fn add_assign_matches_add() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    kani::assume(a.checked_add(b).is_some());
                    let expected = $pod::from(a) + $pod::from(b);
                    let mut pod = $pod::from(a);
                    pod += $pod::from(b);
                    assert!(pod == expected);
                }

                #[kani::proof]
                fn sub_assign_matches_sub() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    kani::assume(a.checked_sub(b).is_some());
                    let expected = $pod::from(a) - $pod::from(b);
                    let mut pod = $pod::from(a);
                    pod -= $pod::from(b);
                    assert!(pod == expected);
                }

                #[kani::proof]
                fn bitand_assign_matches_bitand() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let expected = $pod::from(a) & $pod::from(b);
                    let mut pod = $pod::from(a);
                    pod &= $pod::from(b);
                    assert!(pod == expected);
                }
            }
        };
    }

    kani_operator_proofs_for!(PodU16, u16, ops_u16);
    kani_operator_proofs_for!(PodU32, u32, ops_u32);

    // Full u64 operator proofs. Multiplication, division, and remainder are
    // covered by checked_u64 below (all use z3). This module covers add, sub,
    // bitwise, shift, and assign operators (all fast on CaDiCaL).
    mod ops_u64 {
        use super::super::*;

        #[kani::proof]
        fn add_no_overflow() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            kani::assume(a.checked_add(b).is_some());
            let result = (PodU64::from(a) + PodU64::from(b)).get();
            assert!(result == a + b);
        }

        #[kani::proof]
        fn sub_no_overflow() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            kani::assume(a.checked_sub(b).is_some());
            let result = (PodU64::from(a) - PodU64::from(b)).get();
            assert!(result == a - b);
        }

        #[kani::proof]
        fn bitand_matches_native() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            assert!((PodU64::from(a) & PodU64::from(b)).get() == (a & b));
        }

        #[kani::proof]
        fn bitor_matches_native() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            assert!((PodU64::from(a) | PodU64::from(b)).get() == (a | b));
        }

        #[kani::proof]
        fn bitxor_matches_native() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            assert!((PodU64::from(a) ^ PodU64::from(b)).get() == (a ^ b));
        }

        #[kani::proof]
        fn not_matches_native() {
            let a: u64 = kani::any();
            assert!((!PodU64::from(a)).get() == !a);
        }

        #[kani::proof]
        fn shl_matches_native() {
            let a: u64 = kani::any();
            let shift: u32 = kani::any();
            kani::assume(shift < 64);
            assert!((PodU64::from(a) << shift).get() == (a << shift));
        }

        #[kani::proof]
        fn shr_matches_native() {
            let a: u64 = kani::any();
            let shift: u32 = kani::any();
            kani::assume(shift < 64);
            assert!((PodU64::from(a) >> shift).get() == (a >> shift));
        }

        #[kani::proof]
        fn add_assign_matches_add() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            kani::assume(a.checked_add(b).is_some());
            let expected = PodU64::from(a) + PodU64::from(b);
            let mut pod = PodU64::from(a);
            pod += PodU64::from(b);
            assert!(pod == expected);
        }

        #[kani::proof]
        fn sub_assign_matches_sub() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            kani::assume(a.checked_sub(b).is_some());
            let expected = PodU64::from(a) - PodU64::from(b);
            let mut pod = PodU64::from(a);
            pod -= PodU64::from(b);
            assert!(pod == expected);
        }

        #[kani::proof]
        fn bitand_assign_matches_bitand() {
            let a: u64 = kani::any();
            let b: u64 = kani::any();
            let expected = PodU64::from(a) & PodU64::from(b);
            let mut pod = PodU64::from(a);
            pod &= PodU64::from(b);
            assert!(pod == expected);
        }
    }

    // 128-bit operator proofs: all use z3 since CaDiCaL's SAT encoding is
    // exponentially slow for wide arithmetic.
    mod ops_u128 {
        use super::super::*;

        #[kani::proof]
        #[kani::solver(z3)]
        fn add_no_overflow() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            kani::assume(a.checked_add(b).is_some());
            let result = (PodU128::from(a) + PodU128::from(b)).get();
            assert!(result == a + b);
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn sub_no_overflow() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            kani::assume(a.checked_sub(b).is_some());
            let result = (PodU128::from(a) - PodU128::from(b)).get();
            assert!(result == a - b);
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn bitand_matches_native() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            assert!((PodU128::from(a) & PodU128::from(b)).get() == (a & b));
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn bitor_matches_native() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            assert!((PodU128::from(a) | PodU128::from(b)).get() == (a | b));
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn bitxor_matches_native() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            assert!((PodU128::from(a) ^ PodU128::from(b)).get() == (a ^ b));
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn not_matches_native() {
            let a: u128 = kani::any();
            assert!((!PodU128::from(a)).get() == !a);
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn shl_matches_native() {
            let a: u128 = kani::any();
            let shift: u32 = kani::any();
            kani::assume(shift < 128);
            assert!((PodU128::from(a) << shift).get() == (a << shift));
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn shr_matches_native() {
            let a: u128 = kani::any();
            let shift: u32 = kani::any();
            kani::assume(shift < 128);
            assert!((PodU128::from(a) >> shift).get() == (a >> shift));
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn add_assign_matches_add() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            kani::assume(a.checked_add(b).is_some());
            let expected = PodU128::from(a) + PodU128::from(b);
            let mut pod = PodU128::from(a);
            pod += PodU128::from(b);
            assert!(pod == expected);
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn sub_assign_matches_sub() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            kani::assume(a.checked_sub(b).is_some());
            let expected = PodU128::from(a) - PodU128::from(b);
            let mut pod = PodU128::from(a);
            pod -= PodU128::from(b);
            assert!(pod == expected);
        }

        #[kani::proof]
        #[kani::solver(z3)]
        fn bitand_assign_matches_bitand() {
            let a: u128 = kani::any();
            let b: u128 = kani::any();
            let expected = PodU128::from(a) & PodU128::from(b);
            let mut pod = PodU128::from(a);
            pod &= PodU128::from(b);
            assert!(pod == expected);
        }
    }

    // Checked arithmetic, saturating arithmetic, and division/remainder proofs.
    // All use z3 because CaDiCaL's SAT encoding is slow for multiplication and
    // division even at 32-bit width.
    macro_rules! kani_checked_sat_div_proofs {
        ($pod:ident, $native:ty, $mod_name:ident) => {
            mod $mod_name {
                use super::super::*;

                #[kani::proof]
                #[kani::solver(z3)]
                fn checked_add_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let pod_result = $pod::from(a).checked_add($pod::from(b));
                    let native_result = a.checked_add(b);
                    match (pod_result, native_result) {
                        (Some(p), Some(n)) => assert!(p.get() == n),
                        (None, None) => {}
                        _ => panic!("checked_add mismatch"),
                    }
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn checked_sub_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let pod_result = $pod::from(a).checked_sub($pod::from(b));
                    let native_result = a.checked_sub(b);
                    match (pod_result, native_result) {
                        (Some(p), Some(n)) => assert!(p.get() == n),
                        (None, None) => {}
                        _ => panic!("checked_sub mismatch"),
                    }
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn checked_mul_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let pod_result = $pod::from(a).checked_mul($pod::from(b));
                    let native_result = a.checked_mul(b);
                    match (pod_result, native_result) {
                        (Some(p), Some(n)) => assert!(p.get() == n),
                        (None, None) => {}
                        _ => panic!("checked_mul mismatch"),
                    }
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn checked_div_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let pod_result = $pod::from(a).checked_div($pod::from(b));
                    let native_result = a.checked_div(b);
                    match (pod_result, native_result) {
                        (Some(p), Some(n)) => assert!(p.get() == n),
                        (None, None) => {}
                        _ => panic!("checked_div mismatch"),
                    }
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn saturating_add_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let result = $pod::from(a).saturating_add($pod::from(b)).get();
                    assert!(result == a.saturating_add(b));
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn saturating_sub_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let result = $pod::from(a).saturating_sub($pod::from(b)).get();
                    assert!(result == a.saturating_sub(b));
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn saturating_mul_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    let result = $pod::from(a).saturating_mul($pod::from(b)).get();
                    assert!(result == a.saturating_mul(b));
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn mul_no_overflow() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    kani::assume(a.checked_mul(b).is_some());
                    let result = ($pod::from(a) * $pod::from(b)).get();
                    assert!(result == a * b);
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn div_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    kani::assume(b != 0);
                    let result = ($pod::from(a) / $pod::from(b)).get();
                    assert!(result == a / b);
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn rem_matches_native() {
                    let a: $native = kani::any();
                    let b: $native = kani::any();
                    kani::assume(b != 0);
                    let result = ($pod::from(a) % $pod::from(b)).get();
                    assert!(result == a % b);
                }
            }
        };
    }

    kani_checked_sat_div_proofs!(PodU16, u16, checked_u16);
    kani_checked_sat_div_proofs!(PodU32, u32, checked_u32);
    kani_checked_sat_div_proofs!(PodU64, u64, checked_u64);
    kani_checked_sat_div_proofs!(PodU128, u128, checked_u128);
}
