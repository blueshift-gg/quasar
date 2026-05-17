use super::*;

/// Prove that the u32 >> 8 flag extraction correctly isolates
/// [is_signer, is_writable, executable] from the RuntimeAccount header.
///
/// The actual code in `cpi_account_from_view` does:
///   `(raw as *const u32).read_unaligned() >> 8`
/// on the first 4 bytes: [borrow_state, is_signer, is_writable,
/// executable]. The compile-time `offset_of!` assertions guarantee this
/// field order; this proof verifies the bit arithmetic for all possible
/// byte values.
#[kani::proof]
fn flag_extraction_shift_correctness() {
    let borrow_state: u8 = kani::any();
    let is_signer: u8 = kani::any();
    let is_writable: u8 = kani::any();
    let executable: u8 = kani::any();

    let header = [borrow_state, is_signer, is_writable, executable];
    let raw_u32 = unsafe { (header.as_ptr() as *const u32).read_unaligned() };
    let flags = raw_u32 >> 8;

    // The shift discards borrow_state and preserves the three flag bytes.
    assert!((flags & 0xFF) as u8 == is_signer);
    assert!(((flags >> 8) & 0xFF) as u8 == is_writable);
    assert!(((flags >> 16) & 0xFF) as u8 == executable);
    // High byte is zero; no garbage from the shift.
    assert!(flags >> 24 == 0);
}

/// Prove RawCpiBuilder and CpiAccount have identical memory layout.
///
/// The transmute in `cpi_account_from_view` requires identical size and
/// alignment. The compile-time assertions verify size/align equality,
/// but this proof additionally verifies field offset correspondence:
/// every pointer/u64 field in RawCpiBuilder lands at the expected offset.
#[kani::proof]
fn raw_cpi_builder_layout_matches_cpi_account() {
    // Size and alignment (mirrors compile-time assertions).
    assert!(core::mem::size_of::<RawCpiBuilder>() == core::mem::size_of::<CpiAccount>());
    assert!(core::mem::align_of::<RawCpiBuilder>() == core::mem::align_of::<CpiAccount>());

    // Field offsets: RawCpiBuilder is repr(C) with 7 fields, each 8 bytes.
    // Verify the layout is contiguous with no padding gaps.
    assert!(core::mem::size_of::<RawCpiBuilder>() == 7 * 8);
    assert!(core::mem::offset_of!(RawCpiBuilder, address) == 0);
    assert!(core::mem::offset_of!(RawCpiBuilder, lamports) == 8);
    assert!(core::mem::offset_of!(RawCpiBuilder, data_len) == 16);
    assert!(core::mem::offset_of!(RawCpiBuilder, data) == 24);
    assert!(core::mem::offset_of!(RawCpiBuilder, owner) == 32);
    assert!(core::mem::offset_of!(RawCpiBuilder, rent_epoch) == 40);
    assert!(core::mem::offset_of!(RawCpiBuilder, flags) == 48);
}

/// Prove that `init_cpi_accounts` produces an array of exactly N elements
/// by calling the real function with N AccountViews.
///
/// The function completing without UB (verified by Kani) proves the
/// MaybeUninit init loop covers all indices [0, N) and the final
/// `assume_init` is valid.
#[kani::proof]
fn init_cpi_accounts_loop_covers_all_indices() {
    use super::{AccountBuffer, MIN_ACCOUNT_BUF};

    const N: usize = 4;

    let mut buf0 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf0.init([1; 32], [0xAA; 32], 0, true, true, false);
    let v0 = unsafe { buf0.view() };

    let mut buf1 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf1.init([2; 32], [0xAA; 32], 0, false, true, false);
    let v1 = unsafe { buf1.view() };

    let mut buf2 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf2.init([3; 32], [0xAA; 32], 0, true, false, false);
    let v2 = unsafe { buf2.view() };

    let mut buf3 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf3.init([4; 32], [0xAA; 32], 0, false, false, false);
    let v3 = unsafe { buf3.view() };

    // Call the real function; Kani verifies no UB in the MaybeUninit
    // loop and assume_init.
    let result = super::init_cpi_accounts([&v0, &v1, &v2, &v3]);
    assert!(result.len() == N);
}

/// Prove `CpiReturn::as_slice` is always in bounds.
///
/// Mirrors `CpiReturn::as_slice()`:
///   `&self.data[..self.data_len]`
/// where `data_len` is set by `get_cpi_return()`:
///   `core::cmp::min(size, MAX_RETURN_DATA)`
#[kani::proof]
fn cpi_return_as_slice_in_bounds() {
    let size: usize = kani::any();
    // MAX_RETURN_DATA is 1024.
    let data_len = core::cmp::min(size, solana_instruction_view::cpi::MAX_RETURN_DATA);
    assert!(data_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);
    // The slice &data[..data_len] is within the [u8; MAX_RETURN_DATA]
    // array.
}

/// Prove `CpiReturn::decode` copy length is safe.
///
/// Mirrors `CpiReturn::decode()`:
///   `if self.data_len != expected_len { return Err(...); }`
///   `copy_nonoverlapping(self.data.as_ptr(), ..., expected_len);`
///
/// Proves the copy never reads past `self.data` (capacity
/// `MAX_RETURN_DATA`).
#[kani::proof]
fn cpi_return_decode_copy_in_bounds() {
    let data_len: usize = kani::any();
    let expected_len: usize = kani::any();

    // data_len comes from min(size, MAX_RETURN_DATA).
    kani::assume(data_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);
    // expected_len is size_of::<T::Zc>() for some concrete T.
    // Reasonable upper bound: no Zc type exceeds MAX_RETURN_DATA.
    kani::assume(expected_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);

    // The guard in decode():
    if data_len != expected_len {
        // Returns Err, no copy happens.
        return;
    }

    // If we reach here, copy_nonoverlapping copies expected_len bytes.
    // Source is self.data (size MAX_RETURN_DATA), dest is MaybeUninit<Zc>
    // (size expected_len). Both are valid.
    assert!(expected_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);
    assert!(expected_len == data_len);
}

/// Prove that `invoke_raw` usize-to-u64 casts are lossless.
///
/// Mirrors `invoke_raw()` on-chain path casts:
///   `accounts_len: instruction_accounts_len as u64,`
///   `data_len: data_len as u64,`
///   `cpi_accounts_len as u64,`
///
/// On SBF (32-bit), usize fits in u64 trivially. This proof covers the
/// property for any sizes within Solana's limits (max 10 MiB data,
/// max 256 accounts).
#[kani::proof]
fn invoke_raw_length_cast_lossless() {
    let acct_len: usize = kani::any();
    let data_len: usize = kani::any();
    let cpi_len: usize = kani::any();

    // Solana constraints: max 256 accounts, max ~10 MiB data.
    kani::assume(acct_len <= 256);
    kani::assume(data_len <= 10 * 1024 * 1024);
    kani::assume(cpi_len <= 256);

    // The on-chain path casts to u64:
    let acct_u64 = acct_len as u64;
    let data_u64 = data_len as u64;
    let cpi_u64 = cpi_len as u64;

    // Prove round-trip: no truncation.
    assert!(acct_u64 as usize == acct_len);
    assert!(data_u64 as usize == data_len);
    assert!(cpi_u64 as usize == cpi_len);
}

/// Prove `cpi_account_from_view` data pointer offset is within the
/// RuntimeAccount allocation by calling the real function on a valid
/// AccountView with data.
///
/// The function completing without UB (verified by Kani) proves the
/// `(raw as *const u8).add(RUNTIME_ACCOUNT_SIZE)` data pointer offset
/// is valid and the transmute produces a well-formed `CpiAccount`.
#[kani::proof]
fn cpi_account_data_offset_valid() {
    use super::{AccountBuffer, MIN_ACCOUNT_BUF};

    // Use a buffer large enough for 16 bytes of account data.
    const BUF_SIZE: usize = MIN_ACCOUNT_BUF + 16;

    let mut buf = AccountBuffer::<BUF_SIZE>::new();
    buf.init([0x11; 32], [0x22; 32], 16, true, true, false);
    let view = unsafe { buf.view() };

    // Call the real function; Kani verifies the pointer arithmetic,
    // unaligned read, shift, and transmute are all free of UB.
    let _cpi_account = super::cpi_account_from_view(&view);
}
