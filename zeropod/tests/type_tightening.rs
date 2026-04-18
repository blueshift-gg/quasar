//! Acceptance tests for zeropod type tightening.
//! Verifies ZcElem boundary, error specificity, and compact contract.

use zeropod::{pod::*, ZeroPod, ZeroPodError, ZeroPodFixed};

// --- ZcElem boundary: all pod types ---

#[test]
fn zc_elem_for_all_pod_types() {
    fn assert_elem<T: zeropod::ZcElem>() {}
    assert_elem::<u8>();
    assert_elem::<i8>();
    // bool is NOT ZcElem — constructing &bool from arbitrary bytes is UB.
    // The derive path correctly lowers bool → PodBool.
    assert_elem::<PodU16>();
    assert_elem::<PodU32>();
    assert_elem::<PodU64>();
    assert_elem::<PodU128>();
    assert_elem::<PodI16>();
    assert_elem::<PodI32>();
    assert_elem::<PodI64>();
    assert_elem::<PodI128>();
    assert_elem::<PodBool>();
    assert_elem::<PodOption<PodU64>>();
    assert_elem::<PodOption<PodBool>>();
    assert_elem::<[u8; 32]>();
}

// --- ZcElem for generated types ---

#[derive(ZeroPod, Debug, PartialEq)]
#[repr(u8)]
enum Color {
    Red = 0,
    Green = 1,
    Blue = 2,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Pixel {
    pub color: Color,
    pub alpha: u8,
}

#[test]
fn generated_types_are_zc_elem() {
    fn assert_elem<T: zeropod::ZcElem>() {}
    assert_elem::<ColorZc>();
    assert_elem::<PixelZc>();
}

// --- PodVec with ZcElem types ---

#[test]
fn pod_vec_of_enum_zc() {
    let mut v = PodVec::<ColorZc, 5>::default();
    v.try_push(Color::Red.into()).unwrap();
    v.try_push(Color::Blue.into()).unwrap();
    assert_eq!(v.len(), 2);
    assert!(v.as_slice()[0] == Color::Red);
    assert!(v.as_slice()[1] == Color::Blue);
}

#[test]
fn pod_vec_of_fixed_struct_zc() {
    let mut v = PodVec::<PixelZc, 3>::default();
    // Create a PixelZc from bytes
    let mut buf = [0u8; 2]; // ColorZc(1) + u8(1)
    buf[0] = 1; // Green
    buf[1] = 128; // alpha
    let pixel = unsafe { *(buf.as_ptr() as *const PixelZc) };
    v.try_push(pixel).unwrap();
    assert_eq!(v.len(), 1);
}

#[test]
fn pod_vec_of_pod_option_zc_elem() {
    let mut v = PodVec::<PodOption<PodU32>, 4>::default();
    v.try_push(PodOption::some(PodU32::from(100u32))).unwrap();
    v.try_push(PodOption::<PodU32>::none()).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!(v.as_slice()[0].get(), Some(PodU32::from(100u32)));
    assert!(v.as_slice()[1].is_none());
}

// --- Error specificity ---

#[test]
fn error_invalid_bool() {
    let buf = [2u8];
    let val = unsafe { &*(buf.as_ptr() as *const PodBool) };
    let err = <PodBool as zeropod::ZcValidate>::validate_ref(val);
    assert_eq!(err, Err(ZeroPodError::InvalidBool));
}

#[test]
fn error_invalid_discriminant() {
    let buf = [99u8];
    let err = Color::validate(&buf);
    assert_eq!(err, Err(ZeroPodError::InvalidDiscriminant));
}

#[test]
fn error_invalid_tag() {
    let buf = [5u8, 0u8];
    let opt = unsafe { &*(buf.as_ptr() as *const PodOption<u8>) };
    let err = <PodOption<u8> as zeropod::ZcValidate>::validate_ref(opt);
    assert_eq!(err, Err(ZeroPodError::InvalidTag));
}

#[test]
fn error_buffer_too_small() {
    let buf = [0u8; 0]; // empty buffer
    let err = Color::validate(&buf);
    assert_eq!(err, Err(ZeroPodError::BufferTooSmall));
}

#[test]
fn error_overflow_on_push() {
    let mut v = PodVec::<u8, 1>::default();
    v.try_push(1).unwrap();
    let err = v.try_push(2);
    assert_eq!(err, Err(ZeroPodError::Overflow));
}

// --- Trait taxonomy: relationships hold ---

#[test]
fn zc_elem_implies_zc_validate() {
    // ZcElem: Copy + ZcValidate, so any ZcElem can be validated
    fn validate<T: zeropod::ZcElem>(val: &T) -> Result<(), ZeroPodError> {
        <T as zeropod::ZcValidate>::validate_ref(val)
    }
    let v = PodU64::from(42u64);
    assert!(validate(&v).is_ok());
}

#[test]
fn zc_field_and_zc_elem_are_independent() {
    // Color has ZcField (maps Color -> ColorZc)
    // ColorZc has ZcElem (safe in PodVec)
    // But Color does NOT have ZcElem, and ColorZc does NOT have ZcField
    fn assert_zc_field<T: zeropod::ZcField>() {}
    fn assert_zc_elem<T: zeropod::ZcElem>() {}

    assert_zc_field::<Color>(); // Color is ZcField
    assert_zc_elem::<ColorZc>(); // ColorZc is ZcElem
                                 // Color is NOT ZcElem — correct, it's a schema
                                 // type
                                 // ColorZc is NOT ZcField — correct, it's a
                                 // storage type
}
