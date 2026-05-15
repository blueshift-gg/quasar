use super::*;

#[kani::proof]
fn option_validate_zc_tag_boundary() {
    let tag: u8 = kani::any();
    let mut zc = OptionZc::some(PodU64::from(0u64));
    // SAFETY: The first byte of `PodOption` is its tag; this proof checks
    // that validation accepts only tags 0 and 1.
    unsafe {
        *((&mut zc) as *mut OptionZc<PodU64> as *mut u8) = tag;
    }
    let result = Option::<u64>::validate_zc(&zc);
    assert!(result.is_ok() == (tag <= 1));
}

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

#[kani::proof]
fn instruction_arg_u64_roundtrip() {
    let v: u64 = kani::any();
    let zc = v.to_zc();
    let decoded = u64::from_zc(&zc);
    assert!(decoded == v);
}

#[kani::proof]
fn instruction_arg_bool_roundtrip() {
    let v: bool = kani::any();
    let zc = v.to_zc();
    let decoded = bool::from_zc(&zc);
    assert!(decoded == v);
}
