use zeropod::{
    pod::{PodOption, PodU64},
    ZeroPod, ZeroPodFixed,
};

#[derive(ZeroPod)]
#[allow(dead_code)]
struct GenericValue<T: zeropod::ZcField> {
    value: T,
}

#[derive(ZeroPod)]
#[allow(dead_code)]
struct GenericOption<T: zeropod::ZcField> {
    maybe: Option<T>,
}

#[test]
fn generic_fixed_roundtrip_u64() {
    let mut bytes = [0u8; <GenericValue<u64> as ZeroPodFixed>::SIZE];
    let zc = <GenericValue<u64> as ZeroPodFixed>::from_bytes_mut(&mut bytes).unwrap();
    zc.value = PodU64::from(42);

    let zc = <GenericValue<u64> as ZeroPodFixed>::from_bytes(&bytes).unwrap();
    assert_eq!(zc.value.get(), 42);
}

#[test]
fn generic_fixed_option_roundtrip() {
    let mut bytes = [0u8; <GenericOption<u64> as ZeroPodFixed>::SIZE];
    let zc = <GenericOption<u64> as ZeroPodFixed>::from_bytes_mut(&mut bytes).unwrap();
    zc.maybe = PodOption::some(PodU64::from(7));

    let zc = <GenericOption<u64> as ZeroPodFixed>::from_bytes(&bytes).unwrap();
    assert_eq!(zc.maybe.get(), Some(PodU64::from(7)));
}
