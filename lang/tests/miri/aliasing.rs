use super::*;

#[test]
fn aliasing_shared_to_mut_cast_read_lamports() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 500_000, 64, true, true);
    let mut view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as quasar_lang::account_load::AccountLoad>::check(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
    assert_eq!(account.to_account_view().lamports(), 500_000);
}

#[test]
fn aliasing_shared_to_mut_cast_write_lamports() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);
    let mut view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
    set_lamports(account.to_account_view(), 999);
    assert_eq!(account.to_account_view().lamports(), 999);
}

#[test]
fn aliasing_write_then_read_original_view() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);
    let mut view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
    set_lamports(account.to_account_view(), 777);
    assert_eq!(view.lamports(), 777);
}

#[test]
fn aliasing_interleaved_50_cycles() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 0, 64, true, true);
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view2) };

    for i in 0u64..50 {
        set_lamports(account.to_account_view(), i);
        assert_eq!(view.lamports(), i);
        set_lamports(&view, i + 1000);
        assert_eq!(account.to_account_view().lamports(), i + 1000);
    }
}

#[test]
fn aliasing_triple_ref_view_account_zc() {
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view2) };

    set_lamports(&view, 111);
    assert_eq!(account.to_account_view().lamports(), 111);
    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(777u64);
    }
    let data = unsafe { view.borrow_unchecked() };
    let written = u64::from_le_bytes(data[4..12].try_into().unwrap());
    assert_eq!(written, 777);
    assert_eq!(view.lamports(), 111);
}

#[test]
fn aliasing_deref_mut_offset_sweep() {
    let disc_len = 4;
    let zc_size = size_of::<TestZcData>();
    for &extra_slack in &[0usize, 1, 7, 8, 15, 100] {
        let data_len = disc_len + zc_size + extra_slack;
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
        data[disc_len..disc_len + 8].copy_from_slice(&(extra_slack as u64).to_le_bytes());
        data[disc_len + 8] = 1;
        buf.write_data(&data);

        let mut view = unsafe { buf.view() };
        let account =
            unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
        let zc: &mut TestZcData = &mut *account;
        assert_eq!(zc.value.get(), extra_slack as u64);
        zc.value = PodU64::from(42u64);
        assert_eq!(zc.value.get(), 42);
    }
}

#[test]
fn aliasing_duplicate_accounts_2_mut_refs() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 1_000_000, 64, true, true);

    let mut view_a = unsafe { buf.view() };
    let mut view_b = unsafe { AccountView::new_unchecked(buf.raw()) };

    let acct_a =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_a) };
    let acct_b =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_b) };

    for i in 0u64..20 {
        set_lamports(acct_a.to_account_view(), i);
        assert_eq!(acct_b.to_account_view().lamports(), i);
        set_lamports(acct_b.to_account_view(), i + 1000);
        assert_eq!(acct_a.to_account_view().lamports(), i + 1000);
    }
}

#[test]
fn aliasing_duplicate_accounts_3_mut_refs() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 0, 64, true, true);

    let mut view_a = unsafe { buf.view() };
    let mut view_b = unsafe { AccountView::new_unchecked(buf.raw()) };
    let mut view_c = unsafe { AccountView::new_unchecked(buf.raw()) };

    let a = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_a) };
    let b = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_b) };
    let c = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_c) };

    set_lamports(a.to_account_view(), 1);
    assert_eq!(b.to_account_view().lamports(), 1);
    set_lamports(b.to_account_view(), 2);
    assert_eq!(c.to_account_view().lamports(), 2);
    set_lamports(c.to_account_view(), 3);
    assert_eq!(a.to_account_view().lamports(), 3);
}

#[test]
fn aliasing_duplicate_accounts_4_deref_mut_to_same_data() {
    let disc_len = 4;
    let data_len = disc_len + size_of::<TestZcData>();
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        true,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[..disc_len].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    buf.write_data(&data);

    let mut views: Vec<AccountView> = (0..4)
        .map(|_| unsafe { AccountView::new_unchecked(buf.raw()) })
        .collect();
    let accts: Vec<&mut Account<TestAccountType>> = views
        .iter_mut()
        .map(|v| unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(v) })
        .collect();

    for (i, acct) in accts.iter().enumerate() {
        let zc: &mut TestZcData =
            unsafe { &mut *(acct.to_account_view().data_ptr().add(4) as *mut TestZcData) };
        zc.value = PodU64::from((i as u64 + 1) * 100);
    }
    let final_val = unsafe {
        (*(accts[0].to_account_view().data_ptr().add(4) as *const TestZcData))
            .value
            .get()
    };
    assert_eq!(final_val, 400);
}

#[test]
fn aliasing_borrow_unchecked_mut_rapid_cycling() {
    let mut buf = AccountBuffer::new(32);
    buf.init([1u8; 32], [0u8; 32], 100, 32, false, true);
    let mut view = unsafe { buf.view() };

    for i in 0u64..50 {
        let data = unsafe { view.borrow_unchecked_mut() };
        data[0..8].copy_from_slice(&i.to_le_bytes());
    }
    let data = unsafe { view.borrow_unchecked() };
    assert_eq!(u64::from_le_bytes(data[0..8].try_into().unwrap()), 49);
}

#[test]
fn aliasing_unchecked_account_write_read() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 500, 0, false, true);
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    let unchecked = unsafe { UncheckedAccount::from_account_view_unchecked_mut(&mut view2) };

    for i in 0u64..10 {
        set_lamports(unchecked.to_account_view(), i);
        assert_eq!(view.lamports(), i);
        set_lamports(&view, i + 100);
        assert_eq!(unchecked.to_account_view().lamports(), i + 100);
    }
}

#[test]
fn aliasing_signer_write_read() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 500, 0, true, true);
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    <SignerAccount as checks::Signer>::check(&view).unwrap();
    let signer = unsafe { SignerAccount::from_account_view_unchecked_mut(&mut view2) };

    for i in 0u64..10 {
        set_lamports(signer.to_account_view(), i);
        assert_eq!(view.lamports(), i);
        set_lamports(&view, i + 100);
        assert_eq!(signer.to_account_view().lamports(), i + 100);
    }
}

#[test]
fn aliasing_deref_mut_write_then_deref_read() {
    let mut buf = make_zc_buffer();
    let mut view = unsafe { buf.view() };
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };

    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(7777u64);
    }
    let zc: &TestZcData = &*account;
    assert_eq!(zc.value.get(), 7777);
}

#[test]
fn aliasing_deref_mut_write_then_read_via_view() {
    let mut buf = make_zc_buffer();
    let mut view = unsafe { buf.view() };
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };

    let zc: &mut TestZcData = &mut *account;
    zc.value = PodU64::from(12345u64);

    let data = unsafe { view.borrow_unchecked() };
    let written = u64::from_le_bytes(data[4..12].try_into().unwrap());
    assert_eq!(written, 12345);
}

#[test]
fn aliasing_multiple_deref_mut_calls() {
    let mut buf = make_zc_buffer();
    let mut view = unsafe { buf.view() };
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };

    for i in 0u64..10 {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(i);
        assert_eq!(zc.value.get(), i);
    }
}
