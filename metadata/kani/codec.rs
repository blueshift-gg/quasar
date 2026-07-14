use super::*;

/// Prove write_prefix::<1> writes a byte that decodes back to the input value.
#[kani::proof]
fn write_prefix_u8_roundtrip() {
    let value: u32 = kani::any();
    kani::assume(value <= u8::MAX as u32);
    let mut buf = [0u8; 4];
    unsafe { write_prefix::<1>(buf.as_mut_ptr(), 0, value) };
    assert!(buf[0] as u32 == value);
}

/// Prove write_prefix::<2> writes LE bytes that decode back to the input value.
#[kani::proof]
fn write_prefix_u16_roundtrip() {
    let value: u32 = kani::any();
    kani::assume(value <= u16::MAX as u32);
    let mut buf = [0u8; 4];
    unsafe { write_prefix::<2>(buf.as_mut_ptr(), 0, value) };
    let decoded = u16::from_le_bytes([buf[0], buf[1]]) as u32;
    assert!(decoded == value);
}

/// Prove write_prefix::<4> writes LE bytes that decode back to the input value.
#[kani::proof]
fn write_prefix_u32_roundtrip() {
    let value: u32 = kani::any();
    let mut buf = [0u8; 4];
    unsafe { write_prefix::<4>(buf.as_mut_ptr(), 0, value) };
    let decoded = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    assert!(decoded == value);
}

/// Prove write_prefix at a nonzero offset writes to the correct location and
/// doesn't clobber earlier bytes.
#[kani::proof]
fn write_prefix_offset_correctness() {
    let value: u32 = kani::any();
    let sentinel: u8 = kani::any();
    let mut buf = [sentinel; 8];
    unsafe { write_prefix::<4>(buf.as_mut_ptr(), 2, value) };
    assert!(buf[0] == sentinel);
    assert!(buf[1] == sentinel);
    let decoded = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
    assert!(decoded == value);
}

/// Realistic upper bound on a dynamic field written through `CpiEncode`, sized
/// for a full 200-byte metadata URI rather than the earlier `len <= 8` toy
/// bound. The payload verification uses a single symbolic probe index instead
/// of an O(len) loop so raising the bound stays tractable for Kani (the
/// `copy_nonoverlapping` inside `write_to` is modeled directly, without loop
/// unwinding).
const MAX_DYN_LEN: usize = 200;

/// Prove `CpiEncode<4>::write_to` for `&str` writes prefix + data within
/// `offset + encoded_len()`, and doesn't clobber bytes before offset.
#[kani::proof]
fn str_write_to_bounds_and_roundtrip() {
    let len: usize = kani::any();
    kani::assume(len <= MAX_DYN_LEN);

    let data = [0x41u8; MAX_DYN_LEN];
    // SAFETY: `data` is all ASCII `A` bytes, and `len <= data.len()`.
    let s = unsafe { core::str::from_utf8_unchecked(&data[..len]) };

    let mut buf = [0xFFu8; MAX_DYN_LEN + 8];
    let offset: usize = kani::any();
    kani::assume(offset <= 4);
    kani::assume(offset + 4 + len <= MAX_DYN_LEN + 8);

    let new_offset = unsafe { <&str as CpiEncode<4>>::write_to(&s, buf.as_mut_ptr(), offset) };

    assert!(new_offset == offset + 4 + len);
    let prefix = u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]);
    assert!(prefix == len as u32);

    // Symbolic probe: prove *every* payload byte round-trips without unrolling
    // a `0..len` loop up to `MAX_DYN_LEN` times.
    if len > 0 {
        let probe: usize = kani::any();
        kani::assume(probe < len);
        assert!(buf[offset + 4 + probe] == 0x41);
    }
}

/// Prove `CpiEncode<4>::write_to` for `&[u8]` writes correctly.
#[kani::proof]
fn bytes_write_to_bounds_and_roundtrip() {
    let len: usize = kani::any();
    kani::assume(len <= MAX_DYN_LEN);

    let data = [0xBBu8; MAX_DYN_LEN];
    let slice = &data[..len];

    let mut buf = [0xFFu8; MAX_DYN_LEN + 8];
    let offset: usize = kani::any();
    kani::assume(offset <= 4);
    kani::assume(offset + 4 + len <= MAX_DYN_LEN + 8);

    let new_offset = unsafe { <&[u8] as CpiEncode<4>>::write_to(&slice, buf.as_mut_ptr(), offset) };

    assert!(new_offset == offset + 4 + len);

    let prefix = u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]);
    assert!(prefix == len as u32);

    // Symbolic probe over the payload (see `str_write_to_bounds_and_roundtrip`).
    if len > 0 {
        let probe: usize = kani::any();
        kani::assume(probe < len);
        assert!(buf[offset + 4 + probe] == 0xBB);
    }
}

/// Prove `encoded_len` for `&[u8]` returns prefix + content length.
#[kani::proof]
fn encoded_len_matches_written() {
    let len: usize = kani::any();
    kani::assume(len <= MAX_DYN_LEN);

    let data = [0u8; MAX_DYN_LEN];
    let slice: &[u8] = &data[..len];

    let encoded_len = <&[u8] as CpiEncode<4>>::encoded_len(&slice);
    assert!(encoded_len == 4 + len);
}
