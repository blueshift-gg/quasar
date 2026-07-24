use zeropod::{ZeroPod, ZeroPodCompact, ZeroPodFixed};

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

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct ConstTailProfile<const BIO_CAP: usize, const TAG_CAP: usize> {
    pub bio: zeropod::String<BIO_CAP>,
    pub tags: zeropod::Vec<[u8; 4], TAG_CAP>,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct OptionalTailProfile {
    pub nickname: Option<zeropod::String<8>>,
    pub tags: Option<zeropod::Vec<[u8; 4], 3>>,
    pub note: zeropod::String<8>,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct FixedEventPayload {
    pub amount: u64,
    pub enabled: bool,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct CompactEventPayload {
    pub label: zeropod::String<8>,
    pub points: zeropod::Vec<u16, 3>,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
#[repr(u8)]
enum CompactEvent {
    Empty = 0,
    Label(zeropod::String<8>) = 1,
    Points(zeropod::Vec<u16, 3>) = 2,
    Fixed(FixedEventPayload) = 3,
    #[zeropod(compact)]
    Nested(CompactEventPayload) = 4,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
#[repr(u16)]
enum WideCompactEvent {
    Empty = 0,
    Label(zeropod::String<8>) = 300,
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

#[test]
fn compact_const_generic_tail_capacity() {
    assert_eq!(
        <ConstTailProfile<5, 2> as zeropod::ZeroPodCompact>::HEADER_SIZE,
        3
    );

    let mut buf = vec![0u8; 32];
    {
        let mut profile = ConstTailProfileMut::<5, 2>::new(&mut buf).unwrap();
        profile.set_bio("hello").unwrap();
        profile.set_tags(&[[1; 4], [2; 4]]).unwrap();
        let new_size = profile.commit().unwrap();
        assert_eq!(new_size, 3 + 5 + 8);
    }

    {
        let profile = ConstTailProfileRef::<5, 2>::new(&buf).unwrap();
        assert_eq!(profile.bio(), "hello");
        assert_eq!(profile.tags(), &[[1; 4], [2; 4]]);
    }

    let mut profile = ConstTailProfileMut::<5, 2>::new(&mut buf).unwrap();
    assert!(profile.set_bio("too long").is_err());
    assert!(profile.set_tags(&[[0; 4], [1; 4], [2; 4]]).is_err());
}

#[test]
fn compact_optional_dynamic_tails_store_only_active_payloads() {
    assert_eq!(
        <OptionalTailProfile as zeropod::ZeroPodCompact>::HEADER_SIZE,
        3
    );

    let mut buf = vec![0u8; 64];
    {
        let mut profile = OptionalTailProfileMut::new(&mut buf).unwrap();
        profile.set_nickname(None).unwrap();
        profile.set_tags(Some(&[[1; 4], [2; 4]])).unwrap();
        profile.set_note("ok").unwrap();
        let new_size = profile.commit().unwrap();

        assert_eq!(new_size, 3 + 2 + 8 + 2);
        assert_eq!(&buf[..3], &[0, 1, 2]);
        assert_eq!(&buf[3..5], &[2, 0]);
        assert_eq!(&buf[5..13], &[1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!(&buf[13..15], b"ok");
    }

    let profile = OptionalTailProfileRef::new(&buf[..15]).unwrap();
    assert_eq!(profile.nickname(), None);
    assert_eq!(profile.tags(), Some(&[[1; 4], [2; 4]][..]));
    assert_eq!(profile.note(), "ok");

    let mut none_only = vec![0u8; 3];
    {
        let mut profile = OptionalTailProfileMut::new(&mut none_only).unwrap();
        profile.set_nickname(None).unwrap();
        profile.set_tags(None).unwrap();
        let new_size = profile.commit().unwrap();
        assert_eq!(new_size, 3);
        assert_eq!(&none_only, &[0, 0, 0]);
    }
}

#[test]
fn compact_optional_dynamic_tails_validate_tags_and_payloads() {
    let mut bad_tag = vec![0u8; 3];
    bad_tag[0] = 2;
    assert_eq!(
        OptionalTailProfile::validate(&bad_tag),
        Err(zeropod::ZeroPodError::InvalidTag)
    );

    let missing_payload_prefix = [1u8, 0, 0];
    assert_eq!(
        OptionalTailProfile::validate(&missing_payload_prefix),
        Err(zeropod::ZeroPodError::BufferTooSmall)
    );

    let mut overlong = vec![0u8; 4];
    overlong[0] = 1;
    overlong[3] = 9;
    assert_eq!(
        OptionalTailProfile::validate(&overlong),
        Err(zeropod::ZeroPodError::InvalidLength)
    );
}

#[test]
fn compact_tagged_union_stores_only_active_variant_payload() {
    assert_eq!(<CompactEvent as zeropod::ZeroPodCompact>::HEADER_SIZE, 1);

    let label = [1u8, 2, b'o', b'k'];
    match CompactEventRef::new(&label).unwrap() {
        CompactEventRef::Label(value) => assert_eq!(value, "ok"),
        _ => panic!("expected label variant"),
    }

    let points = [2u8, 2, 0, 5, 0, 7, 0];
    match CompactEventRef::new(&points).unwrap() {
        CompactEventRef::Points(values) => {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0].get(), 5);
            assert_eq!(values[1].get(), 7);
        }
        _ => panic!("expected points variant"),
    }

    let mut fixed = vec![3u8; 1 + <FixedEventPayload as zeropod::ZeroPodFixed>::SIZE];
    fixed[1..9].copy_from_slice(&9u64.to_le_bytes());
    fixed[9] = 1;
    match CompactEventRef::new(&fixed).unwrap() {
        CompactEventRef::Fixed(value) => {
            assert_eq!(value.amount.get(), 9);
            assert!(value.enabled.get());
        }
        _ => panic!("expected fixed variant"),
    }

    let nested = [4u8, 2, 1, 0, b'o', b'k', 11, 0];
    match CompactEventRef::new(&nested).unwrap() {
        CompactEventRef::Nested(value) => {
            assert_eq!(value.label(), "ok");
            assert_eq!(value.points()[0].get(), 11);
        }
        _ => panic!("expected nested compact variant"),
    }
}

#[test]
fn compact_tagged_union_mutates_between_variant_shapes() {
    let mut buf = vec![0u8; 32];
    {
        let mut event = CompactEventMut::new(&mut buf).unwrap();
        event.set_label("hello").unwrap();
        assert_eq!(event.projected_size(), 1 + 1 + 5);
        assert_eq!(event.commit().unwrap(), 1 + 1 + 5);
    }
    assert_eq!(&buf[..7], &[1, 5, b'h', b'e', b'l', b'l', b'o']);

    {
        let mut event = CompactEventMut::new(&mut buf[..7]).unwrap();
        event.set_empty().unwrap();
        assert_eq!(event.projected_size(), 1);
        assert_eq!(event.commit().unwrap(), 1);
    }
    assert_eq!(buf[0], 0);
    assert!(matches!(
        CompactEventRef::new(&buf[..1]).unwrap(),
        CompactEventRef::Empty
    ));

    let points = [5u16.into(), 7u16.into(), 9u16.into()];
    {
        let mut event = CompactEventMut::new(&mut buf).unwrap();
        event.set_points(&points).unwrap();
        assert_eq!(event.projected_size(), 1 + 2 + 6);
        assert_eq!(event.commit().unwrap(), 1 + 2 + 6);
    }
    match CompactEventRef::new(&buf[..9]).unwrap() {
        CompactEventRef::Points(values) => {
            assert_eq!(values.len(), 3);
            assert_eq!(values[2].get(), 9);
        }
        _ => panic!("expected points variant"),
    }
}

#[test]
fn compact_tagged_union_mutates_fixed_and_compact_payloads() {
    let mut buf = vec![0u8; 32];

    let mut fixed_buf = vec![0u8; <FixedEventPayload as zeropod::ZeroPodFixed>::SIZE];
    let fixed = FixedEventPayload::from_bytes_mut(&mut fixed_buf).unwrap();
    fixed.amount = 42u64.into();
    fixed.enabled = true.into();

    {
        let mut event = CompactEventMut::new(&mut buf).unwrap();
        event.set_fixed(fixed).unwrap();
        assert_eq!(
            event.projected_size(),
            1 + <FixedEventPayload as zeropod::ZeroPodFixed>::SIZE
        );
        assert_eq!(
            event.commit().unwrap(),
            1 + <FixedEventPayload as zeropod::ZeroPodFixed>::SIZE
        );
    }
    match CompactEventRef::new(&buf[..10]).unwrap() {
        CompactEventRef::Fixed(value) => {
            assert_eq!(value.amount.get(), 42);
            assert!(value.enabled.get());
        }
        _ => panic!("expected fixed variant"),
    }

    let mut nested_buf = vec![0u8; 16];
    let nested_size = {
        let mut nested = CompactEventPayloadMut::new(&mut nested_buf).unwrap();
        let nested_points = [11u16.into()];
        nested.set_label("xy").unwrap();
        nested.set_points(&nested_points).unwrap();
        nested.commit().unwrap()
    };

    {
        let mut event = CompactEventMut::new(&mut buf).unwrap();
        event.set_nested(&nested_buf[..nested_size]).unwrap();
        assert_eq!(event.projected_size(), 1 + nested_size);
        assert_eq!(event.commit().unwrap(), 1 + nested_size);
    }
    match CompactEventRef::new(&buf[..1 + nested_size]).unwrap() {
        CompactEventRef::Nested(value) => {
            assert_eq!(value.label(), "xy");
            assert_eq!(value.points()[0].get(), 11);
        }
        _ => panic!("expected nested variant"),
    }
}

#[test]
fn compact_tagged_union_mutation_rejects_invalid_values_and_capacity() {
    let mut buf = vec![0u8; 4];
    let mut event = CompactEventMut::new(&mut buf).unwrap();

    assert_eq!(
        event.set_label("too-long!"),
        Err(zeropod::ZeroPodError::Overflow)
    );
    let too_many_points = [1u16.into(), 2u16.into(), 3u16.into(), 4u16.into()];
    assert_eq!(
        event.set_points(&too_many_points),
        Err(zeropod::ZeroPodError::Overflow)
    );

    event.set_label("abcd").unwrap();
    assert_eq!(event.commit(), Err(zeropod::ZeroPodError::BufferTooSmall));
}

#[test]
fn compact_tagged_union_honors_wide_tags() {
    assert_eq!(
        <WideCompactEvent as zeropod::ZeroPodCompact>::HEADER_SIZE,
        2
    );

    let mut buf = vec![0u8; 16];
    {
        let mut event = WideCompactEventMut::new(&mut buf).unwrap();
        event.set_label("hi").unwrap();
        assert_eq!(event.commit().unwrap(), 2 + 1 + 2);
    }

    assert_eq!(&buf[..5], &[44, 1, 2, b'h', b'i']);
    match WideCompactEventRef::new(&buf[..5]).unwrap() {
        WideCompactEventRef::Label(value) => assert_eq!(value, "hi"),
        _ => panic!("expected label variant"),
    }

    assert_eq!(
        WideCompactEvent::validate(&[1, 0]),
        Err(zeropod::ZeroPodError::InvalidDiscriminant)
    );
}

#[test]
fn compact_tagged_union_rejects_invalid_tags_and_payloads() {
    assert_eq!(
        CompactEvent::validate(&[9]),
        Err(zeropod::ZeroPodError::InvalidDiscriminant)
    );
    assert_eq!(
        CompactEvent::validate(&[1, 9, b'o', b'v', b'e', b'r']),
        Err(zeropod::ZeroPodError::InvalidLength)
    );
    assert_eq!(
        CompactEvent::validate(&[2, 4, 0, 1, 0, 2, 0, 3, 0, 4, 0]),
        Err(zeropod::ZeroPodError::InvalidLength)
    );
    assert_eq!(
        CompactEvent::validate(&[3, 0, 0, 0]),
        Err(zeropod::ZeroPodError::BufferTooSmall)
    );
    assert_eq!(
        CompactEvent::validate(&[4, 9, 0, 0]),
        Err(zeropod::ZeroPodError::InvalidLength)
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
