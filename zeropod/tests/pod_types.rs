use zeropod::pod::*;

// ---- PodOption tests ----

#[test]
fn pod_option_none() {
    let opt = PodOption::<u8>::none();
    assert!(opt.is_none());
    assert!(!opt.is_some());
    assert_eq!(opt.get(), None);
    assert_eq!(opt.raw_tag(), 0);
}

#[test]
fn pod_option_some() {
    let opt = PodOption::some(42u8);
    assert!(opt.is_some());
    assert!(!opt.is_none());
    assert_eq!(opt.get(), Some(42u8));
    assert_eq!(opt.raw_tag(), 1);
}

#[test]
fn pod_option_set() {
    let mut opt = PodOption::<u8>::none();
    opt.set(Some(10));
    assert_eq!(opt.get(), Some(10));
    opt.set(None);
    assert!(opt.is_none());
}

#[test]
fn pod_option_alignment() {
    assert_eq!(core::mem::align_of::<PodOption<u8>>(), 1);
    assert_eq!(core::mem::align_of::<PodOption<[u8; 32]>>(), 1);
}

#[test]
fn pod_option_size() {
    // tag (1) + value size
    assert_eq!(core::mem::size_of::<PodOption<u8>>(), 2);
    assert_eq!(core::mem::size_of::<PodOption<[u8; 32]>>(), 33);
}

#[test]
fn pod_option_default() {
    let opt = PodOption::<u8>::default();
    assert!(opt.is_none());
}

#[test]
fn pod_option_eq() {
    let a = PodOption::some(5u8);
    let b = PodOption::some(5u8);
    let c = PodOption::some(6u8);
    let d = PodOption::<u8>::none();
    let e = PodOption::<u8>::none();
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
    assert_eq!(d, e);
}

#[test]
fn pod_option_debug() {
    let some = PodOption::some(42u8);
    let none = PodOption::<u8>::none();
    assert_eq!(format!("{:?}", some), "Some(42)");
    assert_eq!(format!("{:?}", none), "None");
}

// ---- Numeric pod type smoke tests ----

#[test]
fn numeric_alignment() {
    assert_eq!(core::mem::align_of::<PodU16>(), 1);
    assert_eq!(core::mem::align_of::<PodU32>(), 1);
    assert_eq!(core::mem::align_of::<PodU64>(), 1);
    assert_eq!(core::mem::align_of::<PodU128>(), 1);
    assert_eq!(core::mem::align_of::<PodI16>(), 1);
    assert_eq!(core::mem::align_of::<PodI32>(), 1);
    assert_eq!(core::mem::align_of::<PodI64>(), 1);
    assert_eq!(core::mem::align_of::<PodI128>(), 1);
    assert_eq!(core::mem::align_of::<PodBool>(), 1);
}

#[test]
fn numeric_size() {
    assert_eq!(core::mem::size_of::<PodU16>(), 2);
    assert_eq!(core::mem::size_of::<PodU32>(), 4);
    assert_eq!(core::mem::size_of::<PodU64>(), 8);
    assert_eq!(core::mem::size_of::<PodU128>(), 16);
    assert_eq!(core::mem::size_of::<PodI16>(), 2);
    assert_eq!(core::mem::size_of::<PodI32>(), 4);
    assert_eq!(core::mem::size_of::<PodI64>(), 8);
    assert_eq!(core::mem::size_of::<PodI128>(), 16);
    assert_eq!(core::mem::size_of::<PodBool>(), 1);
}

// ---- PodU64 roundtrip, arithmetic, comparison ----

#[test]
fn pod_u64_roundtrip() {
    let val = 123456789u64;
    let pod = PodU64::from(val);
    assert_eq!(pod.get(), val);
    let back: u64 = pod.into();
    assert_eq!(back, val);
}

#[test]
fn pod_u64_arithmetic() {
    let a = PodU64::from(100u64);
    let b = PodU64::from(42u64);
    assert_eq!((a + b).get(), 142);
    assert_eq!((a - b).get(), 58);
    assert_eq!((a * b).get(), 4200);
    assert_eq!((a / b).get(), 2);
    assert_eq!((a % b).get(), 16);
}

#[test]
fn pod_u64_comparison() {
    let a = PodU64::from(100u64);
    let b = PodU64::from(200u64);
    assert!(a < b);
    assert!(b > a);
    assert!(a == PodU64::from(100u64));
    assert!(a != b);
    assert!(a == 100u64);
}

#[test]
fn pod_u64_checked_arithmetic() {
    let max = PodU64::MAX;
    assert!(max.checked_add(PodU64::from(1u64)).is_none());
    assert_eq!(
        PodU64::from(10u64).checked_add(PodU64::from(5u64)),
        Some(PodU64::from(15u64))
    );
    assert!(PodU64::ZERO.checked_sub(PodU64::from(1u64)).is_none());
    assert!(PodU64::from(5u64).checked_div(PodU64::ZERO).is_none());
}

#[test]
fn pod_u64_is_zero() {
    assert!(PodU64::ZERO.is_zero());
    assert!(!PodU64::from(1u64).is_zero());
}

// ---- PodBool tests ----

#[test]
fn pod_bool_roundtrip() {
    assert!(PodBool::from(true).get());
    assert!(!PodBool::from(false).get());
    assert!(PodBool::from(true) == true);
    assert!(PodBool::from(false) == false);
}

// ---- PodString basic operations ----

#[test]
fn pod_string_basic() {
    let mut s = PodString::<32>::default();
    assert!(s.is_empty());
    assert!(s.set("hello"));
    assert_eq!(s.as_str(), "hello");
    assert_eq!(s.len(), 5);
}

#[test]
fn pod_string_alignment() {
    assert_eq!(core::mem::align_of::<PodString<32>>(), 1);
    assert_eq!(core::mem::align_of::<PodString<0>>(), 1);
    assert_eq!(core::mem::align_of::<PodString<32, 2>>(), 1);
}

#[test]
fn pod_string_overflow() {
    let mut s = PodString::<3>::default();
    assert!(!s.set("abcd"));
    assert!(s.is_empty());
}

#[test]
fn pod_string_push_str() {
    let mut s = PodString::<10>::default();
    assert!(s.set("hello"));
    assert!(s.push_str(" wor"));
    assert_eq!(s.as_str(), "hello wor");
    assert!(!s.push_str("ld")); // would exceed capacity
}

// ---- PodVec basic operations ----

#[test]
fn pod_vec_basic() {
    let mut v = PodVec::<u8, 10>::default();
    assert!(v.is_empty());
    assert!(v.push(1));
    assert!(v.push(2));
    assert!(v.push(3));
    assert_eq!(v.len(), 3);
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn pod_vec_alignment() {
    assert_eq!(core::mem::align_of::<PodVec<u8, 10>>(), 1);
    assert_eq!(core::mem::align_of::<PodVec<[u8; 32], 5>>(), 1);
    assert_eq!(core::mem::align_of::<PodVec<u8, 10, 1>>(), 1);
}

#[test]
fn pod_vec_push_pop() {
    let mut v = PodVec::<u8, 3>::default();
    assert!(v.push(10));
    assert!(v.push(20));
    assert!(v.push(30));
    assert!(!v.push(40)); // full
    assert_eq!(v.pop(), Some(30));
    assert_eq!(v.pop(), Some(20));
    assert_eq!(v.pop(), Some(10));
    assert_eq!(v.pop(), None);
}

#[test]
fn pod_vec_set_from_slice() {
    let mut v = PodVec::<u8, 5>::default();
    assert!(v.set_from_slice(&[1, 2, 3, 4, 5]));
    assert_eq!(v.as_slice(), &[1, 2, 3, 4, 5]);
    assert!(!v.set_from_slice(&[1, 2, 3, 4, 5, 6])); // too many
}

// ---- Reverse-direction operator tests ----

#[test]
fn reverse_comparison() {
    let v = PodU64::from(42u64);
    assert!(100u64 > v);
    assert!(42u64 == v);
    assert!(10u64 < v);
}

#[test]
fn reverse_arithmetic() {
    let v = PodU64::from(10u64);
    assert_eq!((100u64 + v).get(), 110);
    assert_eq!((100u64 - v).get(), 90);
    assert_eq!((5u64 * v).get(), 50);
    assert_eq!((100u64 / v).get(), 10);
    assert_eq!((105u64 % v).get(), 5);
}

#[test]
fn reverse_comparison_signed() {
    let v = PodI32::from(-5i32);
    assert!(0i32 > v);
    assert!(-5i32 == v);
    assert!(-10i32 < v);
}

#[test]
fn reverse_arithmetic_signed() {
    let v = PodI32::from(10i32);
    assert_eq!((100i32 + v).get(), 110);
    assert_eq!((100i32 - v).get(), 90);
    assert_eq!((5i32 * v).get(), 50);
}

use core::hash::{Hash, Hasher};

// A minimal hasher for testing
struct TestHasher(u64);
impl Hasher for TestHasher {
    fn finish(&self) -> u64 { self.0 }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes { self.0 = self.0.wrapping_mul(31).wrapping_add(b as u64); }
    }
}

#[test]
fn pod_u64_hash() {
    let a = PodU64::from(42u64);
    let b = PodU64::from(42u64);
    let mut ha = TestHasher(0);
    let mut hb = TestHasher(0);
    a.hash(&mut ha);
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

#[test]
fn pod_u64_formatting() {
    let v = PodU64::from(255u64);
    assert_eq!(format!("{:b}", v), format!("{:b}", 255u64));
    assert_eq!(format!("{:x}", v), format!("{:x}", 255u64));
    assert_eq!(format!("{:X}", v), format!("{:X}", 255u64));
}

#[test]
fn pod_u64_wrapping() {
    let v = PodU64::from(u64::MAX);
    assert_eq!(v.wrapping_add(1u64).get(), 0u64);
    assert_eq!(PodU64::from(0u64).wrapping_sub(1u64).get(), u64::MAX);
    assert_eq!(PodU64::from(u64::MAX).wrapping_mul(2u64).get(), u64::MAX.wrapping_mul(2));
}

#[test]
fn pod_u64_set() {
    let mut v = PodU64::from(0u64);
    v.set(42u64);
    assert_eq!(v.get(), 42u64);
}

#[test]
fn pod_i64_wrapping() {
    let v = PodI64::from(i64::MAX);
    assert_eq!(v.wrapping_add(1i64).get(), i64::MIN);
}
