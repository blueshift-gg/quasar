//! Deterministic UB regression tests, also run by ordinary `cargo test`.
//! Fixtures own aligned ABI buffers until every account handle is dropped.

use alloc::{string::String, vec, vec::Vec};

use crate::{
    Address, ProgramError,
    account::{Account, Kind},
    input::{Cursor, Header, NOT_DUPLICATE, entry_size},
    remaining::Remaining,
    wrapper::{ProgramId, SystemProgram, UncheckedAccount, Wrapper},
};

pub(super) const OWNER: Address = Address::new_from_array([7; 32]);

/// `duplicates` are compact ABI entries following the full accounts.
/// Handles must only be used inside `run`, while the backing allocation lives.
pub(super) fn with_accounts(
    accounts: &[(u8, &[u8])],
    duplicates: &[u8],
    run: impl FnOnce(&mut Cursor),
) {
    let bytes = 8
        + accounts.iter().map(|(_, data)| entry_size(data.len())).sum::<usize>()
        + duplicates.len() * 8;
    let mut input = vec![0u64; bytes / 8];
    input[0] = (accounts.len() + duplicates.len()) as u64;
    let base = input.as_mut_ptr().cast::<u8>();
    let mut offset = 8;
    for &(key, data) in accounts {
        let header = Header {
            dup: NOT_DUPLICATE,
            is_signer: 1,
            is_writable: 1,
            executable: 1,
            resize_delta: 0,
            key: Address::new_from_array([key; 32]),
            owner: OWNER,
            lamports: 100,
            data_len: data.len() as u64,
        };
        // SAFETY: each aligned entry has a full header, data, reserve and rent epoch.
        unsafe {
            base.add(offset).cast::<Header>().write(header);
            base.add(offset + size_of::<Header>())
                .copy_from_nonoverlapping(data.as_ptr(), data.len());
        }
        offset += entry_size(data.len());
    }
    for &duplicate in duplicates {
        // SAFETY: each duplicate occupies eight initialized bytes in `input`.
        unsafe { base.add(offset).write(duplicate) };
        offset += 8;
    }
    // SAFETY: the buffer follows ABIv1, stays alive through `run`, and has one cursor.
    let mut cursor = unsafe { Cursor::new(base, &SystemProgram::ID) };
    run(&mut cursor);
}

#[derive(zeropod::ZeroPod)]
pub(super) struct Counter {
    pub(super) value: u64,
}

impl Kind for Counter {
    const OWNER: Address = OWNER;
    const DISCRIMINATOR: &'static [u8] = &[9];
    const PROGRAM_OWNED: bool = true;
}

#[test]
fn account_views_alternate_reads_writes_and_reload() {
    // The one-byte discriminator deliberately leaves the u64 unaligned.
    with_accounts(&[(1, &[9, 0, 0, 0, 0, 0, 0, 0, 0])], &[], |cursor| {
        let mut account = cursor.load::<Account<Counter>>(true).unwrap();
        for value in [1, u64::MAX, 0, 42] {
            account.set_value(value).unwrap();
            assert_eq!(account.value(), value);
            assert_eq!(&account.data()[1..], &value.to_le_bytes());
            account.reload().unwrap();
            assert_eq!(account.value(), value);
        }
        account.set_inner(&Counter { value: 123 }).unwrap();
        assert_eq!(account.value(), 123);
    });
}

#[derive(zeropod::ZeroPod)]
struct Dynamic {
    marker: u64,
    #[max_len(16)]
    label: String,
    #[max_len(4)]
    values: Vec<u64>,
}

impl Kind for Dynamic {
    const OWNER: Address = OWNER;
    const DISCRIMINATOR: &'static [u8] = &[9];
    const PROGRAM_OWNED: bool = true;
}

#[test]
fn dynamic_views_shift_the_tail_and_flush_the_length() {
    // A fixed u64 followed by an empty string and vector.
    let mut data = [0; 17];
    data[0] = 9;
    with_accounts(&[(1, &data)], &[], |cursor| {
        let mut account = cursor.load::<Account<Dynamic>>(true).unwrap();
        account.set_marker(u64::MAX).unwrap();
        for label in ["abcdefghijklmnop", "", "é", "short"] {
            account.set_label(label).unwrap();
            account.set_values(&[1, 2, 3, u64::MAX]).unwrap();
            account.flush();
            assert_eq!(account.label(), label);
            assert_eq!(account.marker(), u64::MAX);
            assert_eq!(account.data().len(), 17 + label.len() + 32);
            account.reload().unwrap();
            // Shrinking the string moves the following vector without losing its contents.
            account.set_label("").unwrap();
            account.flush();
            assert_eq!(&account.data()[17..25], &1u64.to_le_bytes());
            assert_eq!(&account.data()[41..49], &u64::MAX.to_le_bytes());
            account.set_values(&[]).unwrap();
            account.flush();
            assert_eq!(account.data().len(), 17);
            assert_eq!(account.marker(), u64::MAX);
        }
    });
}

#[test]
fn resize_uses_only_its_reserve_and_zeroes_regrowth() {
    for len in [0, 1, 7, 8, 9] {
        let data = vec![0xa5; len];
        with_accounts(&[(1, &data), (2, &[0x5a])], &[], |cursor| {
            let mut first = cursor.load::<UncheckedAccount>(true).unwrap();
            let second = cursor.load::<UncheckedAccount>(false).unwrap();
            let entry = first.entry_mut();
            let limit = len + 10 * 1024;
            entry.resize(limit).unwrap();
            assert!(entry.data()[len..].iter().all(|byte| *byte == 0));
            entry.data_mut().fill(0xff);
            entry.resize(0).unwrap();
            entry.resize(limit).unwrap();
            assert!(entry.data().iter().all(|byte| *byte == 0));
            assert_eq!(entry.resize(limit + 1), Err(ProgramError::InvalidRealloc));
            assert_eq!(entry.data().len(), limit);
            assert_eq!(second.data(), &[0x5a]);
            assert_eq!(second.lamports(), 100);
        });
    }
}

#[test]
fn aliases_are_used_sequentially_and_poison_preserves_provenance() {
    with_accounts(&[(1, &[0; 8])], &[], |cursor| {
        let mut account = cursor.load::<UncheckedAccount>(true).unwrap();
        // SAFETY: accesses through the two handles are sequential, with no live data references.
        let mut alias = unsafe { account.entry().alias() };
        for value in 0..8 {
            alias.set_lamports(value);
            assert_eq!(account.lamports(), value);
            account.entry_mut().data_mut()[0] = value as u8;
            assert_eq!(alias.data()[0], value as u8);
            alias.poison();
            alias.restore();
            assert_eq!(alias.lamports(), value);
        }
    });
}

#[test]
fn remaining_accounts_walk_alignment_boundaries_without_aliasing() {
    for len in 0..16 {
        let data = vec![0xa5; len];
        with_accounts(&[(1, &data), (2, &[0x5a])], &[], |cursor| {
            let mut remaining = Remaining::<UncheckedAccount>::new(cursor, true);
            assert!(cursor.load::<UncheckedAccount>(false).is_err());
            let mut first = remaining.next().unwrap().unwrap();
            let second = remaining.next().unwrap().unwrap();
            first.entry_mut().data_mut().fill(0xff);
            assert_eq!(second.data(), &[0x5a]);
            assert!(remaining.next().is_none());
            assert!(remaining.is_empty());
        });
    }
}

#[test]
fn compact_duplicates_are_rejected_or_skipped_when_optional() {
    with_accounts(&[(1, &[])], &[0], |cursor| {
        let _first = cursor.load::<UncheckedAccount>(true).unwrap();
        assert!(cursor.load::<UncheckedAccount>(true).is_err());
    });
    with_accounts(&[(0, &[])], &[0], |cursor| {
        assert!(cursor.load_optional::<UncheckedAccount>(false).unwrap().is_none());
        assert!(cursor.load_optional::<UncheckedAccount>(false).unwrap().is_none());
        assert!(cursor.is_empty());
    });
}

#[test]
fn instruction_data_reads_the_declared_length_including_empty() {
    for len in [0usize, 1, 7, 8, 9, 32] {
        let mut words = vec![0u64; 1 + len.div_ceil(8)];
        words[0] = len as u64;
        // SAFETY: the data follows its aligned u64 length, with `len` initialized bytes.
        let data = unsafe { crate::input::instruction_data(words.as_ptr().cast::<u8>().add(8)) };
        assert_eq!(data, vec![0; len]);
    }
}
