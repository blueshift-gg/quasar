use super::*;

// These tests exercise the interaction between mutable data writes
// (set_lamports / DerefMut via data_mut_ptr) and cpi_account_from_view(),
// which extracts *const pointers and shared references from the same
// RuntimeAccount. This is the aliasing pattern that set_inner() +
// CPI invocation creates in real programs: write account data through a
// raw mutable pointer, then pass the same AccountView into CpiCall::new
// which internally calls cpi_account_from_view() to create &-references
// to the RuntimeAccount fields.
//
// If any of these are UB under Tree Borrows, Miri will report it.

/// Write to account data via DerefMut (same path as set_inner's
/// data_mut_ptr write), then construct a CpiCall from an aliased view.
/// cpi_account_from_view extracts shared refs to the RuntimeAccount;
/// Miri checks that the prior mutable write didn't invalidate them.
#[test]
fn cpi_aliasing_deref_mut_then_cpi_call() {
    let mut buf = make_zc_buffer();
    // Two views to the same RuntimeAccount: one for mutation, one for CPI
    let cpi_view = unsafe { buf.view() };
    let mut mut_view = unsafe { AccountView::new_unchecked(buf.raw()) };

    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut mut_view) };

    // Write through DerefMut (raw pointer cast from data_ptr -> *mut)
    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(123_456_789u64);
    }

    // Construct CpiCall; internally calls cpi_account_from_view which
    // creates &(*raw).lamports, &(*raw).address etc. from the same
    // RuntimeAccount we just wrote to.
    let program_id = Address::new_from_array([0u8; 32]);
    let _call: CpiCall<'_, 1, 1> = CpiCall::new(
        &program_id,
        [InstructionAccount::writable(cpi_view.address())],
        [&cpi_view],
        [0u8],
    );

    // Read back through aliased view to verify data is intact
    let data = unsafe { cpi_view.borrow_unchecked() };
    let written = u64::from_le_bytes(data[4..12].try_into().unwrap());
    assert_eq!(written, 123_456_789);
}

/// Write lamports via set_lamports (raw *const -> *mut cast on
/// RuntimeAccount), then construct CpiCall. cpi_account_from_view
/// creates `&(*raw).lamports`, a shared ref to the same field
/// we just mutated through a const-to-mut cast.
#[test]
fn cpi_aliasing_set_lamports_then_cpi_call() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);
    let cpi_view = unsafe { buf.view() };
    let mut mut_view = unsafe { AccountView::new_unchecked(buf.raw()) };

    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut mut_view) };

    set_lamports(account.to_account_view(), 42_000);

    let program_id = Address::new_from_array([0u8; 32]);
    let _call: CpiCall<'_, 1, 1> = CpiCall::new(
        &program_id,
        [InstructionAccount::writable(cpi_view.address())],
        [&cpi_view],
        [0u8],
    );

    assert_eq!(cpi_view.lamports(), 42_000);
}

/// Combined: write data via DerefMut AND lamports via set_lamports,
/// then construct CpiCall. Both the data region and the lamports field
/// in RuntimeAccount have been mutated through raw pointers before
/// cpi_account_from_view extracts shared references to them.
#[test]
fn cpi_aliasing_data_and_lamports_then_cpi_call() {
    let mut buf = make_zc_buffer();
    let cpi_view = unsafe { buf.view() };
    let mut mut_view = unsafe { AccountView::new_unchecked(buf.raw()) };

    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut mut_view) };

    // Mutate data
    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(777u64);
        zc.flag = PodBool::from(true);
    }

    // Mutate lamports
    set_lamports(account.to_account_view(), 5_000_000);

    // CpiCall construction reads both regions
    let program_id = Address::new_from_array([0u8; 32]);
    let _call: CpiCall<'_, 1, 1> = CpiCall::new(
        &program_id,
        [InstructionAccount::writable(cpi_view.address())],
        [&cpi_view],
        [0u8],
    );

    let data = unsafe { cpi_view.borrow_unchecked() };
    assert_eq!(u64::from_le_bytes(data[4..12].try_into().unwrap()), 777);
    assert_eq!(cpi_view.lamports(), 5_000_000);
}

/// Interleaved write and CpiCall construction for several cycles.
/// Each cycle creates new shared refs via cpi_account_from_view after
/// a mutable write, stressing Tree Borrows tag tracking.
#[test]
fn cpi_aliasing_interleaved_write_cpi_cycles() {
    let mut buf = make_zc_buffer();
    let cpi_view = unsafe { buf.view() };
    let mut mut_view = unsafe { AccountView::new_unchecked(buf.raw()) };

    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut mut_view) };

    let program_id = Address::new_from_array([0u8; 32]);

    for i in 0u64..20 {
        // Mutable write
        {
            let zc: &mut TestZcData = &mut *account;
            zc.value = PodU64::from(i);
        }
        set_lamports(account.to_account_view(), i * 1000);

        // CpiCall construction (cpi_account_from_view extracts shared refs)
        let _call: CpiCall<'_, 1, 1> = CpiCall::new(
            &program_id,
            [InstructionAccount::writable(cpi_view.address())],
            [&cpi_view],
            [0u8],
        );

        // Verify through aliased view
        let data = unsafe { cpi_view.borrow_unchecked() };
        assert_eq!(u64::from_le_bytes(data[4..12].try_into().unwrap()), i);
        assert_eq!(cpi_view.lamports(), i * 1000);
    }
}

/// Two separate AccountView instances to the same RuntimeAccount:
/// write via one, construct CpiCall via the other.
/// This is the most aggressive aliasing variant: completely separate
/// view objects sharing the same underlying raw pointer.
#[test]
fn cpi_aliasing_two_views_write_one_cpi_other() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);

    let view_for_cpi = unsafe { buf.view() };
    let mut view_for_write = unsafe { AccountView::new_unchecked(buf.raw()) };

    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_for_write) };

    // Write through account (view_for_write path)
    set_lamports(account.to_account_view(), 99_999);

    // CpiCall from the OTHER view (view_for_cpi path)
    let program_id = Address::new_from_array([0u8; 32]);
    let _call: CpiCall<'_, 1, 1> = CpiCall::new(
        &program_id,
        [InstructionAccount::writable(view_for_cpi.address())],
        [&view_for_cpi],
        [0u8],
    );

    assert_eq!(view_for_cpi.lamports(), 99_999);
}

/// Multi-account CPI: write to multiple accounts, then construct a
/// single CpiCall referencing all of them. Exercises init_cpi_accounts
/// which calls cpi_account_from_view for each view.
#[test]
fn cpi_aliasing_multi_account_cpi_after_writes() {
    let mut buf0 = AccountBuffer::new(64);
    buf0.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);
    let mut buf1 = AccountBuffer::new(64);
    buf1.init([2u8; 32], TEST_OWNER.to_bytes(), 200, 64, false, true);

    let cpi_view0 = unsafe { buf0.view() };
    let mut mut_view0 = unsafe { AccountView::new_unchecked(buf0.raw()) };
    let cpi_view1 = unsafe { buf1.view() };
    let mut mut_view1 = unsafe { AccountView::new_unchecked(buf1.raw()) };

    let account0 =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut mut_view0) };
    let account1 =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut mut_view1) };

    // Write to both accounts
    set_lamports(account0.to_account_view(), 1_000);
    set_lamports(account1.to_account_view(), 2_000);

    let program_id = Address::new_from_array([0u8; 32]);
    let _call: CpiCall<'_, 2, 1> = CpiCall::new(
        &program_id,
        [
            InstructionAccount::writable(cpi_view0.address()),
            InstructionAccount::readonly(cpi_view1.address()),
        ],
        [&cpi_view0, &cpi_view1],
        [0u8],
    );

    assert_eq!(cpi_view0.lamports(), 1_000);
    assert_eq!(cpi_view1.lamports(), 2_000);
}

/// Boundary: write to account with exact minimum buffer size,
/// then construct CpiCall. Tests that cpi_account_from_view's pointer
/// arithmetic (data ptr = raw + RUNTIME_ACCOUNT_SIZE) doesn't go OOB
/// when the data region is zero-length.
#[test]
fn cpi_aliasing_zero_data_len() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 500, 0, true, true);
    let view = unsafe { buf.view() };

    set_lamports(&view, 12_345);

    let program_id = Address::new_from_array([0u8; 32]);
    let _call: CpiCall<'_, 1, 1> = CpiCall::new(
        &program_id,
        [InstructionAccount::writable(view.address())],
        [&view],
        [0u8],
    );

    assert_eq!(view.lamports(), 12_345);
}

/// Sweep all flag combinations with data write + CPI aliasing.
/// cpi_account_from_view reads flags as a u32 from the header;
/// ensure this unaligned read doesn't conflict with prior writes.
#[test]
fn cpi_aliasing_flag_sweep() {
    for &(is_signer, is_writable, executable) in SWEEP_FLAG_COMBOS {
        let data_len = 64usize;
        let mut buf = AccountBuffer::new(data_len);
        buf.init_with_executable(
            [1u8; 32],
            TEST_OWNER.to_bytes(),
            100,
            data_len as u64,
            is_signer,
            is_writable,
            executable,
        );

        let view = unsafe { buf.view() };
        set_lamports(&view, 7777);

        let program_id = Address::new_from_array([0u8; 32]);
        let _call: CpiCall<'_, 1, 1> = CpiCall::new(
            &program_id,
            [InstructionAccount::writable(view.address())],
            [&view],
            [0u8],
        );

        assert_eq!(view.lamports(), 7777);
    }
}
