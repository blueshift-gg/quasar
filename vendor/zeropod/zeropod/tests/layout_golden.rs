use zeropod::{pod::*, ZeroPod, ZeroPodFixed};

#[allow(dead_code)]
#[derive(ZeroPod)]
struct GoldenFixed {
    pub a: u8,
    pub b: u64,
    pub c: bool,
    pub d: [u8; 4],
}

#[test]
fn golden_fixed_size() {
    assert_eq!(<GoldenFixed as zeropod::ZeroPodFixed>::SIZE, 1 + 8 + 1 + 4);
}

#[test]
fn golden_fixed_alignment() {
    assert_eq!(
        core::mem::align_of::<<GoldenFixed as zeropod::ZeroPodFixed>::Zc>(),
        1
    );
}

#[test]
fn golden_fixed_field_offsets() {
    let buf = [0u8; 14];
    let zc = GoldenFixed::from_bytes(&buf).unwrap();
    let base = zc as *const _ as usize;
    assert_eq!(&zc.a as *const _ as usize - base, 0);
    assert_eq!(&zc.b as *const _ as usize - base, 1);
    assert_eq!(&zc.c as *const _ as usize - base, 9);
    assert_eq!(&zc.d as *const _ as usize - base, 10);
}

#[test]
fn golden_pod_sizes() {
    assert_eq!(core::mem::size_of::<PodU16>(), 2);
    assert_eq!(core::mem::size_of::<PodU32>(), 4);
    assert_eq!(core::mem::size_of::<PodU64>(), 8);
    assert_eq!(core::mem::size_of::<PodU128>(), 16);
    assert_eq!(core::mem::size_of::<PodBool>(), 1);
    assert_eq!(core::mem::size_of::<PodOption<PodU64>>(), 9);
    assert_eq!(core::mem::size_of::<PodString<32>>(), 33);
    assert_eq!(core::mem::size_of::<PodVec<u8, 10>>(), 12);
}

#[test]
fn golden_all_pod_align_1() {
    assert_eq!(core::mem::align_of::<PodU16>(), 1);
    assert_eq!(core::mem::align_of::<PodU32>(), 1);
    assert_eq!(core::mem::align_of::<PodU64>(), 1);
    assert_eq!(core::mem::align_of::<PodU128>(), 1);
    assert_eq!(core::mem::align_of::<PodI16>(), 1);
    assert_eq!(core::mem::align_of::<PodI32>(), 1);
    assert_eq!(core::mem::align_of::<PodI64>(), 1);
    assert_eq!(core::mem::align_of::<PodI128>(), 1);
    assert_eq!(core::mem::align_of::<PodBool>(), 1);
    assert_eq!(core::mem::align_of::<PodOption<PodU64>>(), 1);
    assert_eq!(core::mem::align_of::<PodString<32>>(), 1);
    assert_eq!(core::mem::align_of::<PodString<256, 2>>(), 1);
    assert_eq!(core::mem::align_of::<PodVec<u8, 10>>(), 1);
    assert_eq!(core::mem::align_of::<PodVec<[u8; 32], 20>>(), 1);
}
