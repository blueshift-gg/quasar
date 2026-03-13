use {
    quasar_pod::*,
    std::mem::{align_of, size_of},
};

// ---------------------------------------------------------------------------
// Representation invariants
// ---------------------------------------------------------------------------

#[test]
fn alignment_is_one() {
    assert_eq!(align_of::<PodU128>(), 1);
    assert_eq!(align_of::<PodU64>(), 1);
    assert_eq!(align_of::<PodU32>(), 1);
    assert_eq!(align_of::<PodU16>(), 1);
    assert_eq!(align_of::<PodI128>(), 1);
    assert_eq!(align_of::<PodI64>(), 1);
    assert_eq!(align_of::<PodI32>(), 1);
    assert_eq!(align_of::<PodI16>(), 1);
    assert_eq!(align_of::<PodBool>(), 1);
}

#[test]
fn size_matches_native() {
    assert_eq!(size_of::<PodU128>(), 16);
    assert_eq!(size_of::<PodU64>(), 8);
    assert_eq!(size_of::<PodU32>(), 4);
    assert_eq!(size_of::<PodU16>(), 2);
    assert_eq!(size_of::<PodI128>(), 16);
    assert_eq!(size_of::<PodI64>(), 8);
    assert_eq!(size_of::<PodI32>(), 4);
    assert_eq!(size_of::<PodI16>(), 2);
    assert_eq!(size_of::<PodBool>(), 1);
}

#[test]
fn repr_transparent_same_size_as_byte_array() {
    assert_eq!(size_of::<PodU64>(), size_of::<[u8; 8]>());
    assert_eq!(size_of::<PodU128>(), size_of::<[u8; 16]>());
    assert_eq!(size_of::<PodU32>(), size_of::<[u8; 4]>());
    assert_eq!(size_of::<PodU16>(), size_of::<[u8; 2]>());
}

// ---------------------------------------------------------------------------
// Endianness (little-endian byte layout)
// ---------------------------------------------------------------------------

#[test]
fn endianness_u64() {
    let pod = PodU64::from(1u64);
    let bytes: [u8; 8] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [1, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn endianness_u128() {
    let pod = PodU128::from(1u128);
    let bytes: [u8; 16] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn endianness_u32() {
    let pod = PodU32::from(1u32);
    let bytes: [u8; 4] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [1, 0, 0, 0]);
}

#[test]
fn endianness_u16() {
    let pod = PodU16::from(0x0102u16);
    let bytes: [u8; 2] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [0x02, 0x01]);
}

#[test]
fn endianness_i64() {
    let pod = PodI64::from(1i64);
    let bytes: [u8; 8] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [1, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn endianness_i128() {
    let pod = PodI128::from(-1i128);
    let bytes: [u8; 16] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [0xFF; 16]);
}

#[test]
fn endianness_i32() {
    let pod = PodI32::from(1i32);
    let bytes: [u8; 4] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [1, 0, 0, 0]);
}

#[test]
fn endianness_i16() {
    let pod = PodI16::from(0x0102i16);
    let bytes: [u8; 2] = unsafe { std::mem::transmute(pod) };
    assert_eq!(bytes, [0x02, 0x01]);
}

// ---------------------------------------------------------------------------
// Boundary arithmetic — checked
// ---------------------------------------------------------------------------

#[test]
fn checked_add_identity_at_max() {
    assert_eq!(
        PodU64::MAX.checked_add(PodU64::from(0u64)),
        Some(PodU64::MAX)
    );
}

#[test]
fn checked_sub_zero_minus_zero() {
    assert_eq!(
        PodU64::from(0u64).checked_sub(PodU64::from(0u64)),
        Some(PodU64::from(0u64))
    );
}

#[test]
fn checked_add_overflow() {
    assert_eq!(PodU64::MAX.checked_add(1u64), None);
}

#[test]
fn checked_sub_underflow() {
    assert_eq!(PodU64::from(0u64).checked_sub(1u64), None);
}

#[test]
fn checked_mul_overflow() {
    assert_eq!(PodU64::MAX.checked_mul(2u64), None);
}

#[test]
fn checked_div_by_zero() {
    assert_eq!(PodU64::from(1u64).checked_div(0u64), None);
}

// ---------------------------------------------------------------------------
// Boundary arithmetic — saturating
// ---------------------------------------------------------------------------

#[test]
fn saturating_add_clamps_to_max() {
    assert_eq!(PodU64::MAX.saturating_add(1u64), PodU64::MAX);
}

#[test]
fn saturating_sub_clamps_to_zero() {
    assert_eq!(PodU64::from(0u64).saturating_sub(1u64), PodU64::ZERO);
}

#[test]
fn saturating_mul_clamps_to_max() {
    assert_eq!(PodU64::MAX.saturating_mul(2u64), PodU64::MAX);
}

// ---------------------------------------------------------------------------
// Signed edge cases
// ---------------------------------------------------------------------------

#[test]
fn signed_min_roundtrip() {
    assert_eq!(PodI64::from(i64::MIN).get(), i64::MIN);
}

#[test]
fn signed_negative_roundtrip() {
    assert_eq!(PodI64::from(-1i64).get(), -1);
}

#[test]
fn signed_checked_add_overflow_at_min() {
    assert_eq!(PodI64::from(i64::MIN).checked_add(-1i64), None);
}

#[test]
fn signed_negation() {
    assert_eq!((-PodI64::from(1i64)).get(), -1);
}

#[test]
#[should_panic(expected = "overflow")]
fn signed_negate_min_panics_debug() {
    let _ = -PodI64::from(i64::MIN);
}

// ---------------------------------------------------------------------------
// Cross-type operations
// ---------------------------------------------------------------------------

#[test]
fn pod_plus_native() {
    assert_eq!(PodU64::from(5u64) + 3u64, PodU64::from(8u64));
}

#[test]
fn pod_plus_pod() {
    assert_eq!(PodU64::from(5u64) + PodU64::from(3u64), PodU64::from(8u64));
}

#[test]
fn add_assign_native() {
    let mut x = PodU64::from(5u64);
    x += 3u64;
    assert_eq!(x, PodU64::from(8u64));
}

#[test]
fn add_assign_pod() {
    let mut x = PodU64::from(5u64);
    x += PodU64::from(3u64);
    assert_eq!(x, PodU64::from(8u64));
}

#[test]
fn sub_assign_native() {
    let mut x = PodU64::from(10u64);
    x -= 3u64;
    assert_eq!(x, PodU64::from(7u64));
}

#[test]
fn mul_assign_native() {
    let mut x = PodU64::from(5u64);
    x *= 3u64;
    assert_eq!(x, PodU64::from(15u64));
}

#[test]
fn div_assign_native() {
    let mut x = PodU64::from(15u64);
    x /= 3u64;
    assert_eq!(x, PodU64::from(5u64));
}

#[test]
fn rem_assign_native() {
    let mut x = PodU64::from(17u64);
    x %= 5u64;
    assert_eq!(x, PodU64::from(2u64));
}

#[test]
fn cross_comparison_eq() {
    assert!(PodU64::from(5u64) == 5u64);
}

#[test]
fn cross_comparison_ord() {
    assert!(PodU64::from(5u64) > 3u64);
    assert!(PodU64::from(3u64) < 5u64);
}

// ---------------------------------------------------------------------------
// Bitwise operations
// ---------------------------------------------------------------------------

#[test]
fn bitand_native() {
    assert_eq!(PodU64::from(0xFFu64) & 0x0Fu64, PodU64::from(0x0Fu64));
}

#[test]
fn bitor_native() {
    assert_eq!(PodU64::from(0u64) | 0xFFu64, PodU64::from(0xFFu64));
}

#[test]
fn bitxor_native() {
    assert_eq!(PodU64::from(0xFFu64) ^ 0xFFu64, PodU64::from(0u64));
}

#[test]
fn shl() {
    assert_eq!(PodU64::from(1u64) << 3, PodU64::from(8u64));
}

#[test]
fn shr() {
    assert_eq!(PodU64::from(8u64) >> 3, PodU64::from(1u64));
}

#[test]
fn not_zero_is_max() {
    assert_eq!(!PodU64::from(0u64), PodU64::MAX);
}

#[test]
fn bitand_pod() {
    assert_eq!(
        PodU64::from(0xFFu64) & PodU64::from(0x0Fu64),
        PodU64::from(0x0Fu64)
    );
}

#[test]
fn bitor_pod() {
    assert_eq!(
        PodU64::from(0u64) | PodU64::from(0xFFu64),
        PodU64::from(0xFFu64)
    );
}

#[test]
fn bitxor_pod() {
    assert_eq!(
        PodU64::from(0xFFu64) ^ PodU64::from(0xFFu64),
        PodU64::from(0u64)
    );
}

// ---------------------------------------------------------------------------
// PodBool edge cases
// ---------------------------------------------------------------------------

#[test]
fn pod_bool_true_roundtrip() {
    assert!(PodBool::from(true).get());
}

#[test]
fn pod_bool_false_roundtrip() {
    assert!(!PodBool::from(false).get());
}

#[test]
fn pod_bool_non_canonical_is_true() {
    let raw: PodBool = unsafe { std::mem::transmute([0xFFu8]) };
    assert!(raw.get());
}

#[test]
fn pod_bool_not() {
    assert_eq!(!PodBool::from(true), PodBool::from(false));
    assert_eq!(!PodBool::from(false), PodBool::from(true));
}

#[test]
fn pod_bool_default_is_false() {
    assert!(!PodBool::default().get());
}

#[test]
fn pod_bool_eq_native() {
    assert!(PodBool::from(true) == true);
    assert!(PodBool::from(false) == false);
}

// ---------------------------------------------------------------------------
// Display/Debug format stability
// ---------------------------------------------------------------------------

#[test]
fn display_u64() {
    assert_eq!(format!("{}", PodU64::from(42u64)), "42");
}

#[test]
fn debug_u64() {
    assert_eq!(format!("{:?}", PodU64::from(42u64)), "PodU64(42)");
}

#[test]
fn display_bool_true() {
    assert_eq!(format!("{}", PodBool::from(true)), "true");
}

#[test]
fn debug_bool_true() {
    assert_eq!(format!("{:?}", PodBool::from(true)), "PodBool(true)");
}

#[test]
fn display_i64_negative() {
    assert_eq!(format!("{}", PodI64::from(-1i64)), "-1");
}

#[test]
fn debug_i64() {
    assert_eq!(format!("{:?}", PodI64::from(-1i64)), "PodI64(-1)");
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[test]
fn constants_u64() {
    assert_eq!(PodU64::ZERO, PodU64::from(0u64));
    assert_eq!(PodU64::MAX, PodU64::from(u64::MAX));
    assert_eq!(PodU64::MIN, PodU64::from(u64::MIN));
}

#[test]
fn constants_i64() {
    assert_eq!(PodI64::MIN, PodI64::from(i64::MIN));
    assert_eq!(PodI64::MAX, PodI64::from(i64::MAX));
}

#[test]
fn constants_u128() {
    assert_eq!(PodU128::ZERO, PodU128::from(0u128));
    assert_eq!(PodU128::MAX, PodU128::from(u128::MAX));
}

#[test]
fn constants_i128() {
    assert_eq!(PodI128::MIN, PodI128::from(i128::MIN));
    assert_eq!(PodI128::MAX, PodI128::from(i128::MAX));
}

// ---------------------------------------------------------------------------
// is_zero
// ---------------------------------------------------------------------------

#[test]
fn is_zero_true() {
    assert!(PodU64::ZERO.is_zero());
}

#[test]
fn is_zero_false_one() {
    assert!(!PodU64::from(1u64).is_zero());
}

#[test]
fn is_zero_false_max() {
    assert!(!PodU64::MAX.is_zero());
}

// ---------------------------------------------------------------------------
// Checked operations on other types
// ---------------------------------------------------------------------------

#[test]
fn checked_ops_u32() {
    assert_eq!(PodU32::MAX.checked_add(1u32), None);
    assert_eq!(PodU32::from(0u32).checked_sub(1u32), None);
    assert_eq!(PodU32::MAX.checked_mul(2u32), None);
    assert_eq!(PodU32::from(1u32).checked_div(0u32), None);
    assert_eq!(
        PodU32::from(10u32).checked_add(5u32),
        Some(PodU32::from(15u32))
    );
}

#[test]
fn checked_ops_u128() {
    assert_eq!(PodU128::MAX.checked_add(1u128), None);
    assert_eq!(PodU128::from(0u128).checked_sub(1u128), None);
}

#[test]
fn checked_ops_i64() {
    assert_eq!(PodI64::MAX.checked_add(1i64), None);
    assert_eq!(PodI64::MIN.checked_sub(1i64), None);
}

#[test]
fn saturating_ops_u32() {
    assert_eq!(PodU32::MAX.saturating_add(1u32), PodU32::MAX);
    assert_eq!(PodU32::from(0u32).saturating_sub(1u32), PodU32::ZERO);
    assert_eq!(PodU32::MAX.saturating_mul(2u32), PodU32::MAX);
}

#[test]
fn saturating_ops_i64() {
    assert_eq!(PodI64::MAX.saturating_add(1i64), PodI64::MAX);
    assert_eq!(PodI64::MIN.saturating_sub(1i64), PodI64::MIN);
}

// ---------------------------------------------------------------------------
// Default
// ---------------------------------------------------------------------------

#[test]
fn default_is_zero() {
    assert_eq!(PodU64::default(), PodU64::ZERO);
    assert_eq!(PodU32::default(), PodU32::ZERO);
    assert_eq!(PodI64::default(), PodI64::ZERO);
    assert!(!PodBool::default().get());
}

// ---------------------------------------------------------------------------
// From conversions roundtrip
// ---------------------------------------------------------------------------

#[test]
fn from_native_roundtrip_u64() {
    let val = 12345678u64;
    let pod = PodU64::from(val);
    let back: u64 = pod.into();
    assert_eq!(back, val);
}

#[test]
fn from_native_roundtrip_i64() {
    let val = -12345678i64;
    let pod = PodI64::from(val);
    let back: i64 = pod.into();
    assert_eq!(back, val);
}

#[test]
fn from_native_roundtrip_bool() {
    let pod = PodBool::from(true);
    let back: bool = pod.into();
    assert!(back);
}
