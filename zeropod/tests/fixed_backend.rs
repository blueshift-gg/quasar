use zeropod::{pod::*, ZeroPod, ZeroPodFixed};

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Simple {
    pub value: u64,
    pub flag: bool,
}

#[test]
fn fixed_size() {
    assert_eq!(<Simple as zeropod::ZeroPodFixed>::SIZE, 8 + 1);
}

#[test]
fn fixed_alignment() {
    assert_eq!(
        core::mem::align_of::<<Simple as zeropod::ZeroPodFixed>::Zc>(),
        1
    );
}

#[test]
fn fixed_read() {
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&42u64.to_le_bytes());
    buf[8] = 1;
    let zc = Simple::from_bytes(&buf).unwrap();
    assert_eq!(zc.value.get(), 42);
    assert!(zc.flag.get());
}

#[test]
fn fixed_write() {
    let mut buf = [0u8; 9];
    let zc = Simple::from_bytes_mut(&mut buf).unwrap();
    zc.value = 100.into();
    zc.flag = true.into();
    assert_eq!(buf[0..8], 100u64.to_le_bytes());
    assert_eq!(buf[8], 1);
}

#[test]
fn fixed_validate_bad_bool() {
    let mut buf = [0u8; 9];
    buf[8] = 2;
    assert!(Simple::from_bytes(&buf).is_err());
}

#[test]
fn fixed_buffer_too_small() {
    let buf = [0u8; 4];
    assert!(Simple::from_bytes(&buf).is_err());
}

#[test]
fn fixed_unchecked() {
    let buf = [0u8; 9];
    let zc = unsafe { Simple::from_bytes_unchecked(&buf) };
    assert_eq!(zc.value.get(), 0);
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct WithCollections {
    pub authority: [u8; 32],
    pub name: zeropod::String<32>,
    pub scores: zeropod::Vec<u8, 10>,
    pub active: bool,
    pub maybe: Option<u64>,
}

#[test]
fn fixed_collections_size() {
    // [u8;32](32) + PodString<32,1>(1+32=33) + PodVec<u8,10,2>(2+10=12) +
    // PodBool(1) + PodOption<PodU64>(1+8=9) = 87
    assert_eq!(<WithCollections as zeropod::ZeroPodFixed>::SIZE, 87);
}

#[test]
fn fixed_collections_alignment() {
    assert_eq!(
        core::mem::align_of::<<WithCollections as zeropod::ZeroPodFixed>::Zc>(),
        1
    );
}

#[test]
fn fixed_string_field() {
    let mut buf = [0u8; 87];
    let zc = WithCollections::from_bytes_mut(&mut buf).unwrap();
    let _ = zc.name.set("hello");
    assert_eq!(zc.name.as_str(), "hello");
}

#[test]
fn fixed_vec_field() {
    let mut buf = [0u8; 87];
    let zc = WithCollections::from_bytes_mut(&mut buf).unwrap();
    let _ = zc.scores.push(1);
    let _ = zc.scores.push(2);
    assert_eq!(zc.scores.as_slice(), &[1, 2]);
}

#[test]
fn fixed_option_field() {
    let mut buf = [0u8; 87];
    let zc = WithCollections::from_bytes_mut(&mut buf).unwrap();
    assert!(zc.maybe.is_none());
    zc.maybe.set(Some(PodU64::from(42u64)));
    assert_eq!(zc.maybe.get(), Some(PodU64::from(42u64)));
}

#[test]
fn fixed_zc_is_zc_elem() {
    fn assert_zc_elem<T: zeropod::ZcElem>() {}
    assert_zc_elem::<SimpleZc>();
}
