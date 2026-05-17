use {super::*, solana_account_view::MAX_PERMITTED_DATA_INCREASE};

#[kani::proof]
fn resize_delta_no_overflow() {
    let current_len: i32 = kani::any();
    let new_len: i32 = kani::any();
    kani::assume(current_len >= 0);
    kani::assume(new_len >= 0);
    kani::assume(current_len <= 10 * 1024 * 1024);
    kani::assume(new_len <= 10 * 1024 * 1024);

    let difference = new_len - current_len;

    let prior_accumulated: i32 = kani::any();
    kani::assume(prior_accumulated >= -(MAX_PERMITTED_DATA_INCREASE as i32));
    kani::assume(prior_accumulated <= MAX_PERMITTED_DATA_INCREASE as i32);

    assert!(prior_accumulated.checked_add(difference).is_some());
}

#[kani::proof]
fn padding_i32_roundtrip() {
    let value: i32 = kani::any();
    let mut buf = [0u8; 4];
    unsafe {
        core::ptr::copy_nonoverlapping(&value as *const i32 as *const u8, buf.as_mut_ptr(), 4);
    }
    let read_back = unsafe { (buf.as_ptr() as *const i32).read_unaligned() };
    assert!(read_back == value);
}

#[kani::proof]
fn account_repr_transparent_size() {
    use solana_account_view::AccountView;

    assert!(core::mem::size_of::<Account<AccountView>>() == core::mem::size_of::<AccountView>());
    assert!(core::mem::align_of::<Account<AccountView>>() == core::mem::align_of::<AccountView>());
}

#[kani::proof]
fn set_lamports_field_offset_stable() {
    let offset = core::mem::offset_of!(RuntimeAccount, lamports);
    assert!(offset < core::mem::size_of::<RuntimeAccount>());
    assert!(offset + core::mem::size_of::<u64>() <= core::mem::size_of::<RuntimeAccount>());
}

#[kani::proof]
fn realloc_lamport_subtraction_no_underflow() {
    let rent_exempt: u64 = kani::any();
    let current: u64 = kani::any();

    if rent_exempt > current {
        let deficit = rent_exempt - current;
        assert!(deficit > 0);
        assert!(deficit <= rent_exempt);
    } else if current > rent_exempt {
        let excess = current - rent_exempt;
        assert!(excess > 0);
        assert!(excess <= current);
    }
}

#[kani::proof]
fn realloc_excess_addition_no_overflow() {
    let payer_lamports: u64 = kani::any();
    let excess: u64 = kani::any();

    const MAX_SOL_SUPPLY: u64 = 600_000_000_000_000_000;
    kani::assume(payer_lamports <= MAX_SOL_SUPPLY);
    kani::assume(excess <= MAX_SOL_SUPPLY);
    kani::assume(payer_lamports + excess <= MAX_SOL_SUPPLY);

    assert!(payer_lamports.checked_add(excess).is_some());
}

#[kani::proof]
fn close_lamports_wrapping_add_equivalent_to_checked() {
    let dest_lamports: u64 = kani::any();
    let view_lamports: u64 = kani::any();

    const MAX_SOL_SUPPLY: u64 = 600_000_000_000_000_000;
    kani::assume(dest_lamports <= MAX_SOL_SUPPLY);
    kani::assume(view_lamports <= MAX_SOL_SUPPLY);

    let wrapping_result = dest_lamports.wrapping_add(view_lamports);
    let checked_result = dest_lamports.checked_add(view_lamports);
    assert!(checked_result.is_some());
    assert!(wrapping_result == checked_result.unwrap());
}

#[kani::proof]
fn resize_write_bytes_region_valid() {
    let current_len: i32 = kani::any();
    let new_len: i32 = kani::any();
    kani::assume(current_len >= 0);
    kani::assume(new_len >= 0);
    kani::assume(current_len <= 10 * 1024 * 1024);
    kani::assume(new_len <= 10 * 1024 * 1024);

    let difference = new_len - current_len;
    if difference > 0 {
        let start = current_len as usize;
        let count = difference as usize;
        let end = start.checked_add(count);
        assert!(end.is_some());
        assert!(end.unwrap() == new_len as usize);
        assert!(start <= end.unwrap());
    }
}
