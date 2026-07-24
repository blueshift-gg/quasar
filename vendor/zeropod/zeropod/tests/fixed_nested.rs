use zeropod::{ZeroPod, ZeroPodFixed};

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Inner {
    pub threshold: u64,
    pub bump: u8,
    pub enabled: bool,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Outer {
    pub authority: [u8; 32],
    pub settings: Inner,
    pub value: u64,
}

#[test]
fn nested_size() {
    assert_eq!(<Inner as zeropod::ZeroPodFixed>::SIZE, 10); // PodU64(8) + u8(1) + PodBool(1)
    assert_eq!(<Outer as zeropod::ZeroPodFixed>::SIZE, 50); // [u8;32](32) +
                                                            // InnerZc(10) +
                                                            // PodU64(8)
}

#[test]
fn nested_alignment() {
    assert_eq!(
        core::mem::align_of::<<Inner as zeropod::ZeroPodFixed>::Zc>(),
        1
    );
    assert_eq!(
        core::mem::align_of::<<Outer as zeropod::ZeroPodFixed>::Zc>(),
        1
    );
}

#[test]
fn nested_field_access() {
    let mut buf = [0u8; 50];
    let zc = Outer::from_bytes_mut(&mut buf).unwrap();
    zc.settings.threshold = 100.into();
    zc.settings.bump = 7;
    zc.settings.enabled = true.into();
    zc.value = 42.into();

    assert_eq!(zc.settings.threshold.get(), 100);
    assert_eq!(zc.settings.bump, 7);
    assert!(zc.settings.enabled.get());
    assert_eq!(zc.value.get(), 42);
}

#[test]
fn nested_validation_propagates() {
    let mut buf = [0u8; 50];
    // Inner.enabled is at offset 32 (authority) + 8 (threshold) + 1 (bump) = 41
    buf[41] = 5; // invalid bool
    assert!(Outer::from_bytes(&buf).is_err());
}
