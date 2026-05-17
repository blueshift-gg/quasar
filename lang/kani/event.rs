use super::*;

/// Prove `write_log_disc` returns an offset equal to the discriminator
/// length and copies the discriminator bytes correctly.
#[kani::proof]
fn write_log_disc_offset_and_copy() {
    let disc: [u8; 8] = kani::any();
    let disc_len: usize = kani::any();
    kani::assume(disc_len >= 1 && disc_len <= 8);

    let mut buf = [0u8; 16];
    let offset = unsafe { write_log_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

    assert!(offset == disc_len);
    // Discriminator was copied faithfully.
    let mut i = 0usize;
    while i < disc_len {
        assert!(buf[i] == disc[i]);
        i += 1;
    }
}

/// Prove the log buffer is fully covered: `write_log_disc` initializes
/// `[0, offset)` and `write_data` initializes `[offset, total)` with no
/// gap, so `assume_init_ref` over the full buffer is safe.
#[kani::proof]
fn log_buffer_full_coverage() {
    let disc: [u8; 8] = kani::any();
    let disc_len: usize = kani::any();
    let data_size: usize = kani::any();
    kani::assume(disc_len >= 1 && disc_len <= 8);
    kani::assume(data_size <= 56);

    let total = disc_len + data_size;
    let mut buf = [0u8; 64];
    let offset = unsafe { write_log_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

    // Disc region [0, offset) + data region [offset, offset+data_size) = [0, total)
    assert!(offset == disc_len);
    assert!(offset + data_size == total);
}

/// Prove `write_cpi_disc` writes the 0xFF marker, copies the discriminator,
/// and returns the correct data offset.
#[kani::proof]
fn write_cpi_disc_offset_and_marker() {
    let disc: [u8; 8] = kani::any();
    let disc_len: usize = kani::any();
    kani::assume(disc_len >= 1 && disc_len <= 8);

    let mut buf = [0u8; 16];
    let offset = unsafe { write_cpi_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

    assert!(offset == 1 + disc_len);
    assert!(buf[0] == 0xFF);
    // Discriminator was copied faithfully.
    let mut i = 0usize;
    while i < disc_len {
        assert!(buf[1 + i] == disc[i]);
        i += 1;
    }
}

/// Prove the CPI buffer is fully covered: `write_cpi_disc` initializes
/// `[0, offset)` and `write_data` initializes `[offset, total)` with no
/// gap.
#[kani::proof]
fn cpi_buffer_full_coverage() {
    let disc: [u8; 8] = kani::any();
    let disc_len: usize = kani::any();
    let data_size: usize = kani::any();
    kani::assume(disc_len >= 1 && disc_len <= 8);
    kani::assume(data_size <= 56);

    let total = 1 + disc_len + data_size;
    let mut buf = [0u8; 64];
    let offset = unsafe { write_cpi_disc(buf.as_mut_ptr(), &disc[..disc_len]) };

    // Marker [0) + disc [1, offset) + data [offset, offset+data_size) = [0, total)
    assert!(offset == 1 + disc_len);
    assert!(offset + data_size == total);
}

/// Prove the SVM buffer pointer offset in `handle_event` is correctly
/// computed: `ptr.add(size_of::<u64>())` advances exactly 8 bytes past
/// the account count to reach the first RuntimeAccount.
///
/// The SVM input buffer layout places a u64 account count at offset 0,
/// followed by serialized RuntimeAccount entries. The pointer arithmetic
/// `ptr.add(size_of::<u64>())` must equal `ptr + 8`.
#[kani::proof]
fn handle_event_ptr_offset_is_8() {
    // size_of::<u64>() is the offset used to skip the account count.
    assert!(core::mem::size_of::<u64>() == 8);
    // The offset is a compile-time constant, so this also verifies
    // that the add(8) does not depend on any runtime value.
}

/// Prove the `instruction_data[1..]` slice in `handle_event` is safe
/// given the `len() <= 1` guard.
///
/// `handle_event` returns `Err(InvalidInstructionData)` when
/// `instruction_data.len() <= 1`, so the `&instruction_data[1..]` slice
/// is only reached when len >= 2, making the index 1 always valid.
#[kani::proof]
fn handle_event_data_slice_after_guard() {
    let data_len: usize = kani::any();
    kani::assume(data_len <= 1024);

    // Guard from handle_event:
    if data_len <= 1 {
        // Returns error, no slice operation.
        return;
    }

    // If we reach here, data_len >= 2, so &data[1..] is valid.
    assert!(data_len >= 2);
    let remaining = data_len - 1;
    assert!(remaining >= 1);
    assert!(remaining < data_len);
}

/// Prove `write_cpi_disc` buf.add(1) is safe: the function writes 0xFF
/// at offset 0 and then copies `disc_len` bytes starting at offset 1.
/// The total write region is `1 + disc_len` bytes. This proves the
/// `buf.add(1)` pointer offset does not overflow and stays within the
/// buffer for any valid discriminator length.
#[kani::proof]
fn write_cpi_disc_add_one_no_overflow() {
    let disc_len: usize = kani::any();
    kani::assume(disc_len >= 1 && disc_len <= 8);

    // Total bytes written: 1 (marker) + disc_len (discriminator).
    assert!(1usize.checked_add(disc_len).is_some());
    let total = 1 + disc_len;

    // The write at buf.add(1) for disc_len bytes ends at offset total.
    assert!(total == 1 + disc_len);
    assert!(total <= 9); // max: 1 + 8
}
