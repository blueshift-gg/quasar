use zeropod::{ZeroPod, ZeroPodCompact};

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct Profile {
    pub authority: [u8; 32],
    pub level: u64,
    pub active: bool,
    pub bio: zeropod::String<64>,
    pub tags: zeropod::Vec<[u8; 32], 20>,
}

// --- Header tests ---

#[test]
fn compact_header_size() {
    // authority(32) + PodU64(8) + PodBool(1) + bio_len(1, PFX=1) + tags_len(2,
    // PFX=2) = 44
    assert_eq!(<Profile as zeropod::ZeroPodCompact>::HEADER_SIZE, 44);
}

#[test]
fn compact_header_alignment() {
    assert_eq!(
        core::mem::align_of::<<Profile as zeropod::ZeroPodCompact>::Header>(),
        1
    );
}

// --- Ref tests ---

#[test]
fn compact_ref_inline_via_deref() {
    let buf = vec![0u8; 100];
    let profile = ProfileRef::new(&buf).unwrap();
    assert_eq!(profile.level.get(), 0);
    assert!(!profile.active.get());
}

#[test]
fn compact_ref_empty_tails() {
    let buf = vec![0u8; 100];
    let profile = ProfileRef::new(&buf).unwrap();
    assert_eq!(profile.bio(), "");
    assert_eq!(profile.tags().len(), 0);
}

#[test]
fn compact_ref_bio_with_data() {
    let mut buf = vec![0u8; 100];
    // bio_len is at offset 41 (32+8+1), PFX=1
    buf[41] = 5;
    // bio data at offset 44 (header size)
    buf[44..49].copy_from_slice(b"hello");
    let profile = ProfileRef::new(&buf).unwrap();
    assert_eq!(profile.bio(), "hello");
}

#[test]
fn compact_ref_tags_with_data() {
    let mut buf = vec![0u8; 200];
    // bio_len = 0 (offset 41)
    // tags_len at offset 42-43, PFX=2
    buf[42] = 1;
    buf[43] = 0; // 1 tag
                 // tags data at offset 44 (header) + 0 (bio empty) = 44
    buf[44..76].copy_from_slice(&[0xAA; 32]);
    let profile = ProfileRef::new(&buf).unwrap();
    assert_eq!(profile.tags().len(), 1);
    assert_eq!(profile.tags()[0], [0xAA; 32]);
}

// --- Validation tests ---

#[test]
fn compact_validate_overlength_bio() {
    let mut buf = vec![0u8; 200];
    buf[41] = 65; // bio_len=65 > max 64
    assert!(Profile::validate(&buf).is_err());
}

#[test]
fn compact_validate_tail_overflow() {
    let mut buf = vec![0u8; 50]; // header(44) + only 6 bytes
    buf[41] = 10; // bio_len=10, needs 44+10=54
    assert!(Profile::validate(&buf).is_err());
}

// --- Mut tests ---

#[test]
fn compact_mut_inline_via_deref() {
    let mut buf = vec![0u8; 200];
    let mut profile = ProfileMut::new(&mut buf).unwrap();
    profile.level = 42u64.into();
    profile.active = true.into();
    assert_eq!(profile.level.get(), 42);
    assert!(profile.active.get());
}

#[test]
fn compact_mut_set_bio() {
    let mut buf = vec![0u8; 200];
    let mut profile = ProfileMut::new(&mut buf).unwrap();
    profile.set_bio("hello world").unwrap();
    let new_size = profile.commit().unwrap();
    assert_eq!(new_size, 44 + 11);

    let view = ProfileRef::new(&buf[..new_size]).unwrap();
    assert_eq!(view.bio(), "hello world");
}

#[test]
fn compact_mut_set_bio_and_tags() {
    let mut buf = vec![0u8; 200];
    let tag1 = [0xAA; 32];
    let tag2 = [0xBB; 32];

    let mut profile = ProfileMut::new(&mut buf).unwrap();
    profile.set_bio("test").unwrap();
    let tags = [tag1, tag2];
    profile.set_tags(&tags).unwrap();
    let new_size = profile.commit().unwrap();
    assert_eq!(new_size, 44 + 4 + 64);

    let view = ProfileRef::new(&buf[..new_size]).unwrap();
    assert_eq!(view.bio(), "test");
    assert_eq!(view.tags().len(), 2);
    assert_eq!(view.tags()[0], [0xAA; 32]);
    assert_eq!(view.tags()[1], [0xBB; 32]);
}

#[test]
fn compact_mut_projected_size() {
    let mut buf = vec![0u8; 200];
    let mut profile = ProfileMut::new(&mut buf).unwrap();
    assert_eq!(profile.projected_size(), 44);
    profile.set_bio("hello").unwrap();
    assert_eq!(profile.projected_size(), 44 + 5);
}

#[test]
fn compact_mut_overwrite_shorter() {
    let mut buf = vec![0u8; 200];
    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("hello world").unwrap();
        profile.commit().unwrap();
    }
    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("hi").unwrap();
        let new_size = profile.commit().unwrap();
        assert_eq!(new_size, 44 + 2);
    }
    let view = ProfileRef::new(&buf[..46]).unwrap();
    assert_eq!(view.bio(), "hi");
}

#[test]
fn compact_mut_overflow_rejected() {
    let mut buf = vec![0u8; 200];
    let mut profile = ProfileMut::new(&mut buf).unwrap();
    let long = "x".repeat(65);
    assert!(profile.set_bio(&long).is_err());
}

#[test]
fn compact_mut_commit_preserves_unedited() {
    let mut buf = vec![0u8; 200];
    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("hello").unwrap();
        profile.commit().unwrap();
    }
    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        let new_size = profile.commit().unwrap();
        assert_eq!(new_size, 44 + 5);
    }
    let view = ProfileRef::new(&buf[..49]).unwrap();
    assert_eq!(view.bio(), "hello");
}

#[test]
fn compact_mut_bio_shift_preserves_tags() {
    let mut buf = vec![0u8; 300];
    let tag = [0xCC; 32];

    // Write bio + tags
    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("long bio text here!").unwrap();
        let tags = [tag];
        profile.set_tags(&tags).unwrap();
        profile.commit().unwrap();
    }

    // Now shorten bio — tags must move but preserve content
    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("hi").unwrap();
        // Don't set tags — they should be preserved from old position
        let new_size = profile.commit().unwrap();

        let view = ProfileRef::new(&buf[..new_size]).unwrap();
        assert_eq!(view.bio(), "hi");
        assert_eq!(view.tags().len(), 1);
        assert_eq!(view.tags()[0], [0xCC; 32]);
    }
}
