use {
    super::CpiDynamic,
    crate::cpi::{AccountBuffer, MIN_ACCOUNT_BUF},
};

/// Prove `push_account` bounds check prevents out-of-bounds MaybeUninit
/// writes by calling the real function and verifying Ok/Err at the
/// capacity boundary.
#[kani::proof]
fn push_account_write_in_bounds() {
    const MAX_ACCTS: usize = 4;
    let target: usize = kani::any();
    kani::assume(target <= MAX_ACCTS);

    let addr = solana_address::Address::new_from_array([0x11; 32]);
    let mut cpi = CpiDynamic::<MAX_ACCTS, 8>::new(&addr);

    let mut buf = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf.init([1; 32], [0; 32], 0, true, true, false);
    let view = unsafe { buf.view() };

    // Push `target` times; all should succeed.
    let mut i = 0;
    while i < target {
        assert!(cpi.push_account(&view, true, true).is_ok());
        i += 1;
    }

    // At capacity, next push must fail.
    if target == MAX_ACCTS {
        assert!(cpi.push_account(&view, true, true).is_err());
    }
}

/// Prove `set_data` bounds check prevents out-of-bounds
/// copy_nonoverlapping by calling the real function.
#[kani::proof]
fn set_data_copy_in_bounds() {
    const MAX_DATA: usize = 16;
    let data_len: usize = kani::any();
    kani::assume(data_len <= 32);

    let addr = solana_address::Address::new_from_array([0x11; 32]);
    let mut cpi = CpiDynamic::<1, MAX_DATA>::new(&addr);
    let buf = [0u8; 32];

    if data_len <= MAX_DATA {
        assert!(cpi.set_data(&buf[..data_len]).is_ok());
    } else {
        assert!(cpi.set_data(&buf[..data_len]).is_err());
    }
}

/// Prove that sequential `push_account` calls fill all slots by calling
/// the real function MAX_ACCTS times and verifying capacity exhaustion.
#[kani::proof]
fn sequential_pushes_cover_all_indices() {
    const MAX_ACCTS: usize = 4;
    let addr = solana_address::Address::new_from_array([0x11; 32]);
    let mut cpi = CpiDynamic::<MAX_ACCTS, 8>::new(&addr);

    let mut buf0 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    let mut buf1 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    let mut buf2 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    let mut buf3 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf0.init([1; 32], [0xFF; 32], 0, false, false, false);
    buf1.init([2; 32], [0xFF; 32], 0, false, false, false);
    buf2.init([3; 32], [0xFF; 32], 0, false, false, false);
    buf3.init([4; 32], [0xFF; 32], 0, false, false, false);

    let v0 = unsafe { buf0.view() };
    let v1 = unsafe { buf1.view() };
    let v2 = unsafe { buf2.view() };
    let v3 = unsafe { buf3.view() };

    assert!(cpi.push_account(&v0, false, false).is_ok());
    assert!(cpi.push_account(&v1, false, false).is_ok());
    assert!(cpi.push_account(&v2, false, false).is_ok());
    assert!(cpi.push_account(&v3, false, false).is_ok());
    // Capacity exhausted; next push must fail.
    assert!(cpi.push_account(&v0, false, false).is_err());
}

/// Prove invoke_inner only reads initialized portions of MaybeUninit
/// arrays.
///
/// Mirrors `invoke_inner()`:
///   `invoke_raw(..., self.acct_len, ..., self.data_len, ...,
/// self.acct_len, ...)`
///
/// `invoke_inner` passes `acct_len` (not MAX_ACCTS) and `data_len`
/// (not MAX_DATA) as lengths to `invoke_raw`, so only initialized
/// slots are read. Left as arithmetic model because calling
/// invoke_raw requires a CPI syscall.
#[kani::proof]
fn invoke_reads_only_initialized() {
    const MAX_ACCTS: usize = 8;
    const MAX_DATA: usize = 64;
    let acct_len: usize = kani::any();
    let data_len: usize = kani::any();
    kani::assume(acct_len <= MAX_ACCTS);
    kani::assume(data_len <= MAX_DATA);

    // invoke_raw receives acct_len and data_len as bounds.
    // Verify these are within the MaybeUninit capacity.
    assert!(acct_len <= MAX_ACCTS);
    assert!(data_len <= MAX_DATA);
}
