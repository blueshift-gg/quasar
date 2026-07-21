use super::*;

#[test]
fn bounds_account_view_exact_size_sweep() {
    for &data_len in SWEEP_DATA_LENS {
        let exact_size = size_of::<RuntimeAccount>() + data_len;
        let mut buf = AccountBuffer::exact(exact_size);
        buf.init([1u8; 32], [2u8; 32], 100, data_len as u64, false, true);

        let view = unsafe { buf.view() };
        assert_eq!(view.lamports(), 100);
        assert_eq!(view.data_len(), data_len);
        assert!(view.is_writable());
        assert_eq!(view.data_ptr(), unsafe {
            buf.as_mut_ptr().add(size_of::<RuntimeAccount>())
        });

        if data_len > 0 {
            let data = unsafe { view.borrow_unchecked() };
            assert_eq!(data.len(), data_len);
        }
    }
}

#[test]
fn bounds_zero_data_len() {
    let mut buf = AccountBuffer::exact(size_of::<RuntimeAccount>());
    buf.init([0u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert_eq!(view.data_len(), 0);
    let data = unsafe { view.borrow_unchecked() };
    assert_eq!(data.len(), 0);
}

#[test]
fn bounds_deref_exact_size_buffer() {
    let disc_len = 4;
    let data_len = disc_len + size_of::<TestZcData>();
    let exact_size = size_of::<RuntimeAccount>() + data_len;
    let mut buf = AccountBuffer::exact(exact_size);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        100,
        data_len as u64,
        true,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[..disc_len].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    data[disc_len..disc_len + 8].copy_from_slice(&99u64.to_le_bytes());
    data[disc_len + 8] = 1;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked(&view) };
    let zc: &TestZcData = &*account;
    assert_eq!(zc.value.get(), 99);
    assert!(zc.flag.get());
}

#[test]
fn bounds_remaining_data_len_sweep() {
    for &data_len in SWEEP_DATA_LENS {
        let mut buf = MultiAccountBuffer::new(&[MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len,
            data: Some(vec![0xCC; data_len]),
            is_signer: false,
            is_writable: true,
        }]);
        let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };
        let v = remaining.get(0).unwrap().unwrap();
        assert_eq!(v.data_len(), data_len);
        assert!(remaining.get(1).unwrap().is_none());
    }
}

#[test]
fn bounds_remaining_walk_varied_data_lengths() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len: 1,
            data: Some(vec![0xFF]),
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x02; 32],
            owner: [0xBB; 32],
            lamports: 200,
            data_len: 7,
            data: Some(vec![0xEE; 7]),
            is_signer: true,
            is_writable: false,
        },
        MultiAccountEntry::Full {
            address: [0x03; 32],
            owner: [0xCC; 32],
            lamports: 300,
            data_len: 8,
            data: Some(vec![0xDD; 8]),
            is_signer: false,
            is_writable: true,
        },
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let v0 = remaining.get(0).unwrap().unwrap();
    assert_eq!(v0.lamports(), 100);
    assert_eq!(v0.data_len(), 1);
    let v1 = remaining.get(1).unwrap().unwrap();
    assert_eq!(v1.lamports(), 200);
    assert_eq!(v1.data_len(), 7);
    let v2 = remaining.get(2).unwrap().unwrap();
    assert_eq!(v2.lamports(), 300);
    assert_eq!(v2.data_len(), 8);
    assert!(remaining.get(3).unwrap().is_none());
}

#[test]
fn bounds_remaining_max_capacity_64_accounts() {
    let entries: Vec<_> = (0..64)
        .map(|i| MultiAccountEntry::account_with_data(i as u8, vec![i as u8]))
        .collect();
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 64);
    for (i, v) in views.iter().enumerate() {
        assert_eq!(v.data_len(), 1);
        let data = v.try_borrow_data().unwrap();
        assert_eq!(data[0], i as u8);
    }
}

#[test]
fn bounds_remaining_dup_index_sweep() {
    let mut declared_bufs: Vec<AccountBuffer> = (0..5)
        .map(|i| {
            let mut b = AccountBuffer::new(0);
            b.init(
                [i as u8; 32],
                [0xAA; 32],
                (i as u64 + 1) * 100,
                0,
                true,
                false,
            );
            b
        })
        .collect();
    let declared: Vec<AccountView> = declared_bufs
        .iter_mut()
        .map(|b| unsafe { b.view() })
        .collect();

    for dup_idx in 0..5 {
        let mut buf = MultiAccountBuffer::new(&[
            MultiAccountEntry::account(0x10, 0),
            MultiAccountEntry::duplicate(dup_idx),
        ]);
        let remaining =
            unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &declared) };
        let v = remaining.get(1).unwrap().unwrap();
        assert_eq!(v.address(), &Address::new_from_array([dup_idx as u8; 32]));
    }
}

#[test]
fn bounds_remaining_iterator_fuses_on_unresolvable_dup() {
    // First entry is a duplicate whose original index (200) cannot resolve:
    // there are no declared accounts and the iterator cache is empty. The
    // iterator must fuse after yielding the error, terminating instead of
    // desyncing onto the trailing account entry (which would be mis-cached).
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::duplicate(200),
        MultiAccountEntry::account(0x99, 0),
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };
    let mut it = remaining.iter();

    match it.next() {
        Some(Err(e)) => assert_eq!(e, QuasarError::RemainingAccountDuplicate.into()),
        _ => panic!("first item must be the unresolvable-duplicate error"),
    }
    // Fused: the trailing account is not yielded and iteration terminates.
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn bounds_remaining_iterator_duplicate_preserved() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0),
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };
    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 2);
    assert_eq!(views[0].address(), views[1].address());
}

#[test]
fn bounds_remaining_iterator_dup_cache_resolution() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0),
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };
    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 2);
    assert_eq!(views[0].address(), views[1].address());
}

#[test]
fn bounds_remaining_get_preserves_prior_remaining_duplicate() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0),
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };
    let first = remaining.get(0).unwrap().unwrap();
    let second = remaining.get(1).unwrap().unwrap();
    assert_eq!(first.address(), second.address());
}

#[test]
fn bounds_remaining_duplicate_checked_borrow_conflicts() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account_with_data(0x01, vec![7]),
        MultiAccountEntry::duplicate(0),
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };
    let mut iter = remaining.iter();
    let mut first = iter.next().unwrap().unwrap();
    let second = iter.next().unwrap().unwrap();

    let mut data = first.try_borrow_data_mut().unwrap();
    data[0] = 9;

    assert!(second.try_borrow_data().is_err());
}

#[test]
fn bounds_typed_remaining_rejects_remaining_duplicate() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0),
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let err = match remaining.parse::<UncheckedAccount, 4>() {
        Ok(_) => panic!("typed remaining must reject duplicate aliases"),
        Err(err) => err,
    };
    assert_eq!(err, QuasarError::RemainingAccountDuplicate.into());
}

#[test]
fn bounds_typed_remaining_rejects_declared_duplicate() {
    let mut declared_buf = AccountBuffer::new(0);
    declared_buf.init([0x01; 32], [0xAA; 32], 1_000_000, 0, false, false);
    let declared = [unsafe { declared_buf.view() }];

    let mut buf = MultiAccountBuffer::new(&[MultiAccountEntry::account(0x01, 0)]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &declared) };

    let err = match remaining.parse::<UncheckedAccount, 4>() {
        Ok(_) => panic!("typed remaining must reject aliases to declared accounts"),
        Err(err) => err,
    };
    assert_eq!(err, QuasarError::RemainingAccountDuplicate.into());
}

#[test]
fn bounds_typed_remaining_group_parses_variable_chunks() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::account(0x02, 0),
        MultiAccountEntry::account(0x03, 0),
        MultiAccountEntry::account(0x04, 0),
    ]);
    let program_id = Address::new_from_array([0x55; 32]);
    let remaining = unsafe {
        RemainingAccounts::new_with_context(buf.as_mut_ptr(), buf.boundary(), &[], &program_id, &[])
    };

    let parsed = remaining
        .parse::<remaining_group_fixture::RemainingPair, 2>()
        .unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed.as_slice()[0].first.address(),
        &Address::new_from_array([0x01; 32])
    );
    assert_eq!(
        parsed.as_slice()[1].second.address(),
        &Address::new_from_array([0x04; 32])
    );
}

#[test]
fn bounds_typed_remaining_group_enforces_item_capacity() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::account(0x02, 0),
        MultiAccountEntry::account(0x03, 0),
        MultiAccountEntry::account(0x04, 0),
        MultiAccountEntry::account(0x05, 0),
        MultiAccountEntry::account(0x06, 0),
    ]);
    let program_id = Address::new_from_array([0x55; 32]);
    let remaining = unsafe {
        RemainingAccounts::new_with_context(buf.as_mut_ptr(), buf.boundary(), &[], &program_id, &[])
    };

    let err = match remaining.parse::<remaining_group_fixture::RemainingPair, 2>() {
        Ok(_) => panic!("typed remaining must enforce capacity in items"),
        Err(err) => err,
    };
    assert_eq!(err, QuasarError::RemainingAccountsOverflow.into());
}

#[test]
fn bounds_typed_remaining_group_rejects_duplicate_addresses_across_chunks() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::account(0x02, 0),
        MultiAccountEntry::account(0x03, 0),
        MultiAccountEntry::account(0x01, 0),
    ]);
    let program_id = Address::new_from_array([0x55; 32]);
    let remaining = unsafe {
        RemainingAccounts::new_with_context(buf.as_mut_ptr(), buf.boundary(), &[], &program_id, &[])
    };

    let err = match remaining.parse::<remaining_group_fixture::RemainingPair, 2>() {
        Ok(_) => panic!("typed remaining must reject duplicate addresses across chunks"),
        Err(err) => err,
    };
    assert_eq!(err, QuasarError::RemainingAccountDuplicate.into());
}

#[test]
fn bounds_typed_remaining_group_rejects_partial_chunk() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::account(0x02, 0),
        MultiAccountEntry::account(0x03, 0),
    ]);
    let program_id = Address::new_from_array([0x55; 32]);
    let remaining = unsafe {
        RemainingAccounts::new_with_context(buf.as_mut_ptr(), buf.boundary(), &[], &program_id, &[])
    };

    let err = match remaining.parse::<remaining_group_fixture::RemainingPair, 4>() {
        Ok(_) => panic!("remaining groups must consume complete chunks"),
        Err(err) => err,
    };
    assert_eq!(err, ProgramError::NotEnoughAccountKeys);
}

#[test]
fn bounds_remaining_iterator_overflow_returns_error() {
    const LIMIT: usize = 64;
    let entries: Vec<_> = (0..=LIMIT)
        .map(|i| MultiAccountEntry::account(i as u8, 0))
        .collect();
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let mut iter = remaining.iter();
    for _ in 0..LIMIT {
        iter.next().unwrap().unwrap();
    }
    let err = match iter.next().unwrap() {
        Ok(_) => panic!("expected overflow error"),
        Err(err) => err,
    };
    assert_eq!(err, QuasarError::RemainingAccountsOverflow.into());
    assert!(iter.next().is_none());
}

#[test]
fn bounds_typed_single_remaining_overflow_returns_error() {
    const LIMIT: usize = quasar_lang::remaining::MAX_REMAINING_ACCOUNTS;
    let entries: Vec<_> = (0..=LIMIT)
        .map(|i| MultiAccountEntry::account(i as u8, 0))
        .collect();
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let err = match remaining.parse::<UncheckedAccount, { LIMIT + 1 }>() {
        Ok(_) => panic!("typed remaining must enforce the raw account cap"),
        Err(err) => err,
    };
    assert_eq!(err, QuasarError::RemainingAccountsOverflow.into());
}

#[test]
fn bounds_remaining_empty() {
    let mut buf: Vec<u64> = vec![0; 1];
    let ptr = buf.as_mut_ptr() as *mut u8;
    let boundary = ptr as *const u8;
    let remaining = unsafe { RemainingAccounts::new(ptr, boundary, &[]) };
    assert!(remaining.is_empty());
    assert!(remaining.get(0).unwrap().is_none());
    assert_eq!(remaining.iter().count(), 0);
}

#[test]
fn bounds_remaining_boundary_pointer_subtraction() {
    let remaining_size = ACCOUNT_HEADER + 8;
    let remaining_aligned = (remaining_size + 7) & !7;
    let ix_data_len = 8usize;
    let total = remaining_aligned + size_of::<u64>() + ix_data_len + 32;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;

    let raw = base as *mut RuntimeAccount;
    unsafe {
        (*raw).borrow_state = NOT_BORROWED;
        (*raw).is_signer = 0;
        (*raw).is_writable = 1;
        (*raw).executable = 0;
        (*raw).padding = [0u8; 4];
        (*raw).address = Address::new_from_array([0x01; 32]);
        (*raw).owner = Address::new_from_array([0xAA; 32]);
        (*raw).lamports = 100;
        (*raw).data_len = 8;
    }

    let ix_len_offset = remaining_aligned;
    unsafe { *(base.add(ix_len_offset) as *mut u64) = ix_data_len as u64 };

    let ix_data_offset = ix_len_offset + size_of::<u64>();
    let ix_data = unsafe { std::slice::from_raw_parts(base.add(ix_data_offset), ix_data_len) };
    let boundary = unsafe { ix_data.as_ptr().sub(size_of::<u64>()) };
    assert_eq!(boundary, unsafe { base.add(ix_len_offset) as *const u8 });

    let remaining = unsafe { RemainingAccounts::new(base, boundary, &[]) };
    let v = remaining.get(0).unwrap().unwrap();
    assert_eq!(v.lamports(), 100);
    assert!(remaining.get(1).unwrap().is_none());
}

#[test]
fn bounds_discriminator_read_various_lengths() {
    let ix_data: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];

    let disc4: [u8; 4] = unsafe { *(ix_data.as_ptr() as *const [u8; 4]) };
    assert_eq!(disc4, [0xDE, 0xAD, 0xBE, 0xEF]);
    let disc1: [u8; 1] = unsafe { *(ix_data.as_ptr() as *const [u8; 1]) };
    assert_eq!(disc1, [0xDE]);
    let disc8: [u8; 8] = unsafe { *(ix_data.as_ptr() as *const [u8; 8]) };
    assert_eq!(disc8, [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn bounds_program_id_read_from_end_of_slice() {
    let mut combined = vec![0u8; 8 + 32];
    combined[8..].copy_from_slice(&[0x42; 32]);
    let ix_data = &combined[..8];
    let program_id: &[u8; 32] =
        unsafe { &*(ix_data.as_ptr().add(ix_data.len()) as *const [u8; 32]) };
    assert_eq!(program_id, &[0x42; 32]);
}
