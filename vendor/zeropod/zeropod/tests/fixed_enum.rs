use zeropod::{ZeroPod, ZeroPodFixed};

#[derive(ZeroPod, Debug, PartialEq)]
#[repr(u8)]
enum Status {
    Active = 0,
    Paused = 1,
    Closed = 2,
}

#[test]
fn enum_size() {
    assert_eq!(<Status as zeropod::ZeroPodFixed>::SIZE, 1);
}

#[test]
fn enum_from_bytes_valid() {
    let buf = [1u8];
    let zc = Status::from_bytes(&buf).unwrap();
    assert_eq!(zc.get(), 1u8);
}

#[test]
fn enum_validate_rejects_invalid() {
    let buf = [5u8];
    assert!(Status::from_bytes(&buf).is_err());
}

#[test]
fn enum_from_into() {
    let pod: u8 = Status::Paused.into();
    assert_eq!(pod, 1u8);
}

#[derive(ZeroPod, Debug, PartialEq)]
#[repr(u16)]
enum LargeEnum {
    A = 0,
    B = 256,
    C = 1000,
}

#[test]
fn enum_u16_size() {
    assert_eq!(<LargeEnum as zeropod::ZeroPodFixed>::SIZE, 2);
}

#[test]
fn enum_u16_from_bytes() {
    let buf = 256u16.to_le_bytes();
    let zc = LargeEnum::from_bytes(&buf).unwrap();
    assert_eq!(zc.get(), 256u16);
}

#[test]
fn enum_u16_rejects_invalid() {
    let buf = 999u16.to_le_bytes();
    assert!(LargeEnum::from_bytes(&buf).is_err());
}

#[test]
fn enum_zc_is() {
    let buf = [1u8]; // Paused
    let zc = Status::from_bytes(&buf).unwrap();
    assert!(zc.is(Status::Paused));
    assert!(!zc.is(Status::Active));
}

#[test]
fn enum_zc_display() {
    let buf = [0u8]; // Active
    let zc = Status::from_bytes(&buf).unwrap();
    let s = format!("{}", zc);
    assert_eq!(s, "Active");
}

#[test]
fn enum_zc_debug() {
    let buf = [2u8]; // Closed
    let zc = Status::from_bytes(&buf).unwrap();
    let s = format!("{:?}", zc);
    assert!(s.contains("Closed"));
}

#[test]
fn enum_zc_eq_repr() {
    let buf = [1u8];
    let zc = Status::from_bytes(&buf).unwrap();
    assert!(*zc == 1u8); // PartialEq with repr type
}

#[test]
fn error_invalid_discriminant_variant() {
    let buf = [99u8]; // bad discriminant
    let err = Status::validate(&buf);
    assert_eq!(err, Err(zeropod::ZeroPodError::InvalidDiscriminant));
}

#[test]
fn enum_zc_is_zc_elem() {
    fn assert_zc_elem<T: zeropod::ZcElem>() {}
    assert_zc_elem::<StatusZc>();
    assert_zc_elem::<LargeEnumZc>();
}
