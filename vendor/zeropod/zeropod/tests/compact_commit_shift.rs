//! Regression tests for a data-corruption bug in the compact-layout
//! `commit()` path: when an edited tail field grew, the in-place write of
//! the new value clobbered the source bytes of later unedited tail fields
//! before they were relocated.

use zeropod::{ZeroPod, ZeroPodError};

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

#[test]
fn compact_mut_oversized_grow_returns_buffer_too_small_without_mutating() {
    let mut buf = vec![0u8; 300];
    let tag = [0xDDu8; 32];

    let committed_size = {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("hi").unwrap();
        let tags = [tag];
        profile.set_tags(&tags).unwrap();
        profile.commit().unwrap()
    };

    buf.truncate(committed_size);
    let snapshot = buf.clone();

    {
        // `String<64>` caps UTF-8 length at 64; stay within that while still
        // requiring more total bytes than the minimally-sized account slice.
        let long_bio = "y".repeat(48);
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio(&long_bio).unwrap();
        assert_eq!(
            profile.commit(),
            Err(ZeroPodError::BufferTooSmall),
            "grow must not clobber the buffer when the account is too small"
        );
    }

    assert_eq!(
        buf, snapshot,
        "failed commit must leave the serialized account unchanged"
    );

    let view = ProfileRef::new(&buf).unwrap();
    assert_eq!(view.bio(), "hi");
    assert_eq!(view.tags().len(), 1);
    assert_eq!(view.tags()[0], tag);
}

#[test]
fn compact_mut_bio_grow_preserves_tags() {
    let mut buf = vec![0u8; 300];
    let tag = [0xCCu8; 32];

    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("hi").unwrap();
        let tags = [tag];
        profile.set_tags(&tags).unwrap();
        profile.commit().unwrap();
    }

    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("much longer bio text here!").unwrap();
        let new_size = profile.commit().unwrap();

        let view = ProfileRef::new(&buf[..new_size]).unwrap();
        assert_eq!(view.bio(), "much longer bio text here!");
        assert_eq!(view.tags().len(), 1);
        assert_eq!(view.tags()[0], [0xCCu8; 32]);
    }
}

#[test]
fn compact_mut_bio_grow_preserves_multiple_tags() {
    let mut buf = vec![0u8; 1024];
    let t1 = [0x11u8; 32];
    let t2 = [0x22u8; 32];
    let t3 = [0x33u8; 32];

    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("x").unwrap();
        let tags = [t1, t2, t3];
        profile.set_tags(&tags).unwrap();
        profile.commit().unwrap();
    }

    let long_bio = "z".repeat(60);
    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio(&long_bio).unwrap();
        let new_size = profile.commit().unwrap();

        let view = ProfileRef::new(&buf[..new_size]).unwrap();
        assert_eq!(view.bio(), long_bio);
        assert_eq!(view.tags().len(), 3);
        assert_eq!(view.tags()[0], t1);
        assert_eq!(view.tags()[1], t2);
        assert_eq!(view.tags()[2], t3);
    }
}

#[test]
fn compact_mut_no_op_commit_preserves_all_tails() {
    let mut buf = vec![0u8; 300];
    let tag = [0xABu8; 32];

    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("hello there").unwrap();
        let tags = [tag];
        profile.set_tags(&tags).unwrap();
        profile.commit().unwrap();
    }

    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        let new_size = profile.commit().unwrap();

        let view = ProfileRef::new(&buf[..new_size]).unwrap();
        assert_eq!(view.bio(), "hello there");
        assert_eq!(view.tags().len(), 1);
        assert_eq!(view.tags()[0], tag);
    }
}

#[test]
fn compact_mut_grow_then_shrink_then_grow() {
    let mut buf = vec![0u8; 1024];
    let tag = [0x7Fu8; 32];

    {
        let mut profile = ProfileMut::new(&mut buf).unwrap();
        profile.set_bio("a").unwrap();
        let tags = [tag];
        profile.set_tags(&tags).unwrap();
        profile.commit().unwrap();
    }

    for bio in [
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "b",
        "cccccccccccccccccccccccccccccccccccccccccc",
    ] {
        {
            let mut profile = ProfileMut::new(&mut buf).unwrap();
            profile.set_bio(bio).unwrap();
            profile.commit().unwrap();
        }
        {
            let profile = ProfileRef::new(&buf).unwrap();
            assert_eq!(profile.bio(), bio);
            assert_eq!(profile.tags().len(), 1);
            assert_eq!(profile.tags()[0], tag);
        }
    }
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct MultiTail {
    pub id: u32,
    pub a: zeropod::String<32>,
    pub b: zeropod::String<32>,
    pub c: zeropod::Vec<u8, 32>,
}

#[test]
fn compact_mut_grow_first_preserves_middle_and_last() {
    let mut buf = vec![0u8; 512];

    {
        let mut m = MultiTailMut::new(&mut buf).unwrap();
        m.set_a("a").unwrap();
        m.set_b("bbbbb").unwrap();
        m.set_c(&[1, 2, 3, 4, 5, 6, 7]).unwrap();
        m.commit().unwrap();
    }

    {
        let mut m = MultiTailMut::new(&mut buf).unwrap();
        m.set_a("aaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let new_size = m.commit().unwrap();

        let view = MultiTailRef::new(&buf[..new_size]).unwrap();
        assert_eq!(view.a(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(view.b(), "bbbbb");
        assert_eq!(view.c(), &[1, 2, 3, 4, 5, 6, 7]);
    }
}

#[test]
fn compact_mut_mixed_grow_and_shrink_preserves_unedited_last() {
    let mut buf = vec![0u8; 512];

    {
        let mut m = MultiTailMut::new(&mut buf).unwrap();
        m.set_a("aaaaaaaaaa").unwrap();
        m.set_b("bbbbb").unwrap();
        m.set_c(&[9, 8, 7, 6, 5]).unwrap();
        m.commit().unwrap();
    }

    {
        let mut m = MultiTailMut::new(&mut buf).unwrap();
        m.set_a("aa").unwrap();
        m.set_b("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        let new_size = m.commit().unwrap();

        let view = MultiTailRef::new(&buf[..new_size]).unwrap();
        assert_eq!(view.a(), "aa");
        assert_eq!(view.b(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(view.c(), &[9, 8, 7, 6, 5]);
    }
}
