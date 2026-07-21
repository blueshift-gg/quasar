use super::*;

#[test]
fn uninit_cpi_account_count_sweep() {
    for n in 1..=8 {
        let mut bufs: Vec<AccountBuffer> = (0..n)
            .map(|i| {
                let mut b = AccountBuffer::new(0);
                b.init(
                    [i as u8; 32],
                    [0u8; 32],
                    i as u64,
                    0,
                    i % 2 == 0,
                    i % 2 == 1,
                );
                b
            })
            .collect();
        let views: Vec<AccountView> = bufs.iter_mut().map(|b| unsafe { b.view() }).collect();
        let program_id = Address::new_from_array([0u8; 32]);

        match n {
            1 => {
                let _: CpiCall<'_, 1, 1> = CpiCall::new(
                    &program_id,
                    [InstructionAccount::writable(views[0].address())],
                    [&views[0]],
                    [0u8],
                );
            }
            2 => {
                let _: CpiCall<'_, 2, 1> = CpiCall::new(
                    &program_id,
                    [
                        InstructionAccount::writable(views[0].address()),
                        InstructionAccount::readonly(views[1].address()),
                    ],
                    [&views[0], &views[1]],
                    [0u8],
                );
            }
            3 => {
                let _: CpiCall<'_, 3, 1> = CpiCall::new(
                    &program_id,
                    [
                        InstructionAccount::writable(views[0].address()),
                        InstructionAccount::readonly(views[1].address()),
                        InstructionAccount::writable_signer(views[2].address()),
                    ],
                    [&views[0], &views[1], &views[2]],
                    [0u8],
                );
            }
            4 => {
                let _: CpiCall<'_, 4, 1> = CpiCall::new(
                    &program_id,
                    [
                        InstructionAccount::writable_signer(views[0].address()),
                        InstructionAccount::writable(views[1].address()),
                        InstructionAccount::readonly_signer(views[2].address()),
                        InstructionAccount::readonly(views[3].address()),
                    ],
                    [&views[0], &views[1], &views[2], &views[3]],
                    [0u8],
                );
            }
            5 => {
                let _: CpiCall<'_, 5, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            6 => {
                let _: CpiCall<'_, 6, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            7 => {
                let _: CpiCall<'_, 7, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            8 => {
                let _: CpiCall<'_, 8, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn uninit_cpi_flag_pattern_exhaustive() {
    let program_id = Address::new_from_array([0u8; 32]);

    for &(is_signer, is_writable, executable) in SWEEP_FLAG_COMBOS {
        let mut buf = AccountBuffer::new(0);
        buf.init_with_executable(
            [1u8; 32],
            [2u8; 32],
            100,
            0,
            is_signer,
            is_writable,
            executable,
        );
        let view = unsafe { buf.view() };
        let _: CpiCall<'_, 1, 1> = CpiCall::new(
            &program_id,
            [InstructionAccount::writable(view.address())],
            [&view],
            [0u8],
        );
    }
}

#[test]
fn uninit_cpi_create_account_data() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);

    let from = unsafe { from_buf.view() };
    let to = unsafe { to_buf.view() };
    let owner = Address::new_from_array([0xAA; 32]);

    let call = quasar_lang::cpi::system::create_account(&from, &to, 500_000u64, 100, &owner);
    let data = call.instruction_data();
    assert_eq!(data.len(), 52);
    assert_eq!(u32::from_le_bytes(data[0..4].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(data[4..12].try_into().unwrap()), 500_000);
    assert_eq!(u64::from_le_bytes(data[12..20].try_into().unwrap()), 100);
    assert_eq!(&data[20..52], &[0xAA; 32]);
}

#[test]
fn uninit_cpi_transfer_data() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);

    let from = unsafe { from_buf.view() };
    let to = unsafe { to_buf.view() };

    let call = quasar_lang::cpi::system::transfer(&from, &to, 42u64);
    let data = call.instruction_data();
    assert_eq!(data.len(), 12);
    assert_eq!(u32::from_le_bytes(data[0..4].try_into().unwrap()), 2);
    assert_eq!(u64::from_le_bytes(data[4..12].try_into().unwrap()), 42);
}

#[test]
fn uninit_cpi_assign_data() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 100, 0, true, true);
    let view = unsafe { buf.view() };
    let owner = Address::new_from_array([0xBB; 32]);

    let call = quasar_lang::cpi::system::assign(&view, &owner);
    let data = call.instruction_data();
    assert_eq!(data.len(), 36);
    assert_eq!(u32::from_le_bytes(data[0..4].try_into().unwrap()), 1);
    assert_eq!(&data[4..36], &[0xBB; 32]);
}

#[test]
fn uninit_cpi_transfer_boundary_values() {
    for &lamports in &[0u64, 1, u64::MAX] {
        let mut from_buf = AccountBuffer::new(0);
        from_buf.init([1u8; 32], [0u8; 32], lamports, 0, true, true);
        let mut to_buf = AccountBuffer::new(0);
        to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
        let from = unsafe { from_buf.view() };
        let to = unsafe { to_buf.view() };

        let call = quasar_lang::cpi::system::transfer(&from, &to, lamports);
        let data = call.instruction_data();
        assert_eq!(
            u64::from_le_bytes(data[4..12].try_into().unwrap()),
            lamports
        );
    }
}

#[test]
fn uninit_maybeuninit_account_view_array() {
    const N: usize = 4;
    let mut bufs: Vec<AccountBuffer> = (0..N)
        .map(|i| {
            let mut buf = AccountBuffer::new(0);
            buf.init([i as u8; 32], [0u8; 32], i as u64 * 100, 0, false, false);
            buf
        })
        .collect();

    let views: [AccountView; N] = {
        let mut arr = MaybeUninit::<[AccountView; N]>::uninit();
        let ptr = arr.as_mut_ptr() as *mut AccountView;
        for i in 0..N {
            let view = unsafe { bufs[i].view() };
            unsafe { core::ptr::write(ptr.add(i), view) };
        }
        unsafe { arr.assume_init() }
    };

    for (i, view) in views.iter().enumerate() {
        assert_eq!(view.lamports(), i as u64 * 100);
    }
}

#[test]
fn uninit_maybeuninit_zero_length() {
    let arr: [u8; 0] = {
        let arr = MaybeUninit::<[u8; 0]>::uninit();
        unsafe { arr.assume_init() }
    };
    assert_eq!(arr.len(), 0);
}

#[test]
fn uninit_parse_simulation_dup_from_partially_initialized() {
    let acct0_data_len = 8usize;
    let acct1_data_len = 0usize;
    let acct0_size = (ACCOUNT_HEADER + acct0_data_len + 7) & !7;
    let acct1_size = (ACCOUNT_HEADER + acct1_data_len + 7) & !7;
    let dup_size = size_of::<u64>();
    let total = size_of::<u64>() + acct0_size + acct1_size + dup_size;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;

    unsafe { *(base as *mut u64) = 3 };
    let accounts_start = unsafe { base.add(size_of::<u64>()) };

    let raw0 = accounts_start as *mut RuntimeAccount;
    unsafe {
        (*raw0).borrow_state = NOT_BORROWED;
        (*raw0).is_signer = 1;
        (*raw0).is_writable = 1;
        (*raw0).executable = 0;
        (*raw0).padding = [0u8; 4];
        (*raw0).address = Address::new_from_array([0x01; 32]);
        (*raw0).owner = Address::new_from_array([0xAA; 32]);
        (*raw0).lamports = 100;
        (*raw0).data_len = acct0_data_len as u64;
    }

    let raw1 = unsafe { accounts_start.add(acct0_size) as *mut RuntimeAccount };
    unsafe {
        (*raw1).borrow_state = NOT_BORROWED;
        (*raw1).is_signer = 0;
        (*raw1).is_writable = 1;
        (*raw1).executable = 0;
        (*raw1).padding = [0u8; 4];
        (*raw1).address = Address::new_from_array([0x02; 32]);
        (*raw1).owner = Address::new_from_array([0xBB; 32]);
        (*raw1).lamports = 200;
        (*raw1).data_len = acct1_data_len as u64;
    }

    let acct2_offset = acct0_size + acct1_size;
    unsafe { *accounts_start.add(acct2_offset) = 0u8 };

    const N: usize = 3;
    let mut buf = MaybeUninit::<[AccountView; N]>::uninit();
    let arr_ptr = buf.as_mut_ptr() as *mut AccountView;

    // Exercise the REAL production walk (`parse_all_accounts_unchecked`) under
    // Miri instead of re-implementing the parse loop here.
    let boundary = unsafe { accounts_start.add(acct0_size + acct1_size + dup_size) } as *const u8;
    let (parsed, _end) = unsafe {
        quasar_lang::__internal::parse_all_accounts_unchecked(accounts_start, arr_ptr, N, boundary)
    }
    .expect("parse_all_accounts_unchecked");
    assert_eq!(parsed, N);

    let accounts = unsafe { buf.assume_init() };
    assert_eq!(accounts[0].lamports(), 100);
    assert_eq!(accounts[1].lamports(), 200);
    assert_eq!(accounts[2].address(), accounts[0].address());
    assert_eq!(accounts[2].lamports(), 100);
}

#[test]
fn uninit_parse_simulation_many_dups() {
    let acct_size = (ACCOUNT_HEADER + 7) & !7;
    let dup_size = size_of::<u64>();
    let total = size_of::<u64>() + acct_size * 2 + dup_size * 3;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;
    unsafe { *(base as *mut u64) = 5 };
    let accounts_start = unsafe { base.add(size_of::<u64>()) };

    for idx in 0..2 {
        let raw = unsafe { accounts_start.add(idx * acct_size) as *mut RuntimeAccount };
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = 0;
            (*raw).is_writable = 1;
            (*raw).executable = 0;
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array([(idx + 1) as u8; 32]);
            (*raw).owner = Address::new_from_array([0xAA; 32]);
            (*raw).lamports = (idx as u64 + 1) * 100;
            (*raw).data_len = 0;
        }
    }

    let dup_base = unsafe { accounts_start.add(acct_size * 2) };
    unsafe {
        *dup_base = 0u8;
        *dup_base.add(dup_size) = 1u8;
        *dup_base.add(dup_size * 2) = 0u8;
    }

    const N: usize = 5;
    let mut buf = MaybeUninit::<[AccountView; N]>::uninit();
    let arr_ptr = buf.as_mut_ptr() as *mut AccountView;

    // Exercise the REAL production walk (`parse_all_accounts_unchecked`) under
    // Miri instead of re-implementing the parse loop here.
    let boundary = unsafe { accounts_start.add(acct_size * 2 + dup_size * 3) } as *const u8;
    let (parsed, _end) = unsafe {
        quasar_lang::__internal::parse_all_accounts_unchecked(accounts_start, arr_ptr, N, boundary)
    }
    .expect("parse_all_accounts_unchecked");
    assert_eq!(parsed, N);

    let accounts = unsafe { buf.assume_init() };
    assert_eq!(accounts[0].lamports(), 100);
    assert_eq!(accounts[1].lamports(), 200);
    assert_eq!(accounts[2].address(), accounts[0].address());
    assert_eq!(accounts[3].address(), accounts[1].address());
    assert_eq!(accounts[4].address(), accounts[0].address());
}

#[test]
fn parse_dup_allowed_does_not_preborrow_canonical_account() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [1; 32],
            owner: [2; 32],
            lamports: 10,
            data_len: 4,
            data: Some(vec![1, 2, 3, 4]),
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Duplicate { original_index: 0 },
    ]);
    let program_id = Address::new_from_array([9; 32]);
    let mut views = MaybeUninit::<[AccountView; 2]>::uninit();
    let base = views.as_mut_ptr() as *mut AccountView;

    let first = unsafe {
        quasar_lang::__internal::parse_account_dup(
            buf.as_mut_ptr(),
            base,
            0,
            &program_id,
            ParseFlags {
                expected: quasar_lang::__internal::NODUP_MUT,
                mask: 0x00FF_FFFF,
                flag_mask: 0x00FF_0000,
                is_optional: false,
                is_ref_mut: true,
                allow_dup: false,
            },
        )
        .unwrap()
    };
    unsafe {
        quasar_lang::__internal::parse_account_dup(
            first,
            base,
            1,
            &program_id,
            ParseFlags {
                expected: quasar_lang::__internal::NODUP_MUT,
                mask: 0x00FF_FFFF,
                flag_mask: 0x00FF_0000,
                is_optional: false,
                is_ref_mut: true,
                allow_dup: true,
            },
        )
        .unwrap();
    }

    let mut parsed = unsafe { views.assume_init() };
    assert_eq!(
        unsafe { (*buf.as_mut_ptr().cast::<RuntimeAccount>()).borrow_state },
        NOT_BORROWED
    );

    {
        let (first, second) = parsed.split_at_mut(1);
        let mut data = first[0].try_borrow_mut().unwrap();
        data[0] = 9;
        assert!(second[0].try_borrow().is_err());
    }

    assert_eq!(parsed[1].try_borrow().unwrap()[0], 9);
}

#[test]
fn parse_dup_rejects_duplicate_without_dup_flag_even_readonly() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [1; 32],
            owner: [2; 32],
            lamports: 10,
            data_len: 0,
            data: None,
            is_signer: false,
            is_writable: false,
        },
        MultiAccountEntry::Duplicate { original_index: 0 },
    ]);
    let program_id = Address::new_from_array([9; 32]);
    let mut views = MaybeUninit::<[AccountView; 2]>::uninit();
    let base = views.as_mut_ptr() as *mut AccountView;

    let first = unsafe {
        quasar_lang::__internal::parse_account_dup(
            buf.as_mut_ptr(),
            base,
            0,
            &program_id,
            ParseFlags {
                expected: quasar_lang::__internal::NODUP,
                mask: 0x0000_00FF,
                flag_mask: 0,
                is_optional: false,
                is_ref_mut: false,
                allow_dup: false,
            },
        )
        .unwrap()
    };
    let err = unsafe {
        quasar_lang::__internal::parse_account_dup(
            first,
            base,
            1,
            &program_id,
            ParseFlags {
                expected: quasar_lang::__internal::NODUP,
                mask: 0x0000_00FF,
                flag_mask: 0,
                is_optional: false,
                is_ref_mut: false,
                allow_dup: false,
            },
        )
        .unwrap_err()
    };

    assert_eq!(err, ProgramError::AccountBorrowFailed);
}

#[test]
fn uninit_sysvar_maybeuninit_write_bytes_assume_init() {
    use quasar_lang::sysvars::rent::Rent;

    let rent: Rent = {
        let mut var = MaybeUninit::<Rent>::uninit();
        let var_addr = var.as_mut_ptr() as *mut u8;
        unsafe { var_addr.write_bytes(0, size_of::<Rent>()) };
        unsafe { var.assume_init() }
    };
    assert_eq!(rent.minimum_balance_unchecked(100), 0);
}

#[test]
fn uninit_sysvar_rent_2x_threshold() {
    use quasar_lang::sysvars::rent::{Rent, ACCOUNT_STORAGE_OVERHEAD};

    let rent: Rent = {
        let mut var = MaybeUninit::<Rent>::uninit();
        let ptr = var.as_mut_ptr() as *mut u8;
        unsafe {
            let lpb: u64 = 3480;
            core::ptr::copy_nonoverlapping(lpb.to_le_bytes().as_ptr(), ptr, 8);
            let threshold: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 64];
            core::ptr::copy_nonoverlapping(threshold.as_ptr(), ptr.add(8), 8);
            var.assume_init()
        }
    };

    let data_len = 100usize;
    let expected = 2 * (ACCOUNT_STORAGE_OVERHEAD + data_len as u64) * 3480;
    assert_eq!(rent.minimum_balance_unchecked(data_len), expected);
}

#[test]
fn generated_parse_validates_explicit_rent_before_using_it_for_init() {
    use init_with_rent_fixture::InitWithRent;

    let mut payer_buf = AccountBuffer::new(0);
    payer_buf.init([1; 32], [9; 32], 1_000_000, 0, true, true);

    let mut short_fake_rent = AccountBuffer::exact(size_of::<RuntimeAccount>());
    short_fake_rent.init([2; 32], [9; 32], 1_000_000, 0, false, false);

    let mut target_buf = AccountBuffer::new(0);
    target_buf.init([3; 32], [0; 32], 0, 0, false, true);

    let mut accounts = unsafe { [payer_buf.view(), short_fake_rent.view(), target_buf.view()] };
    let result = unsafe { InitWithRent::parse_unchecked(&mut accounts, &ID) };

    assert_eq!(result.map(|_| ()), Err(ProgramError::IncorrectProgramId));
}
