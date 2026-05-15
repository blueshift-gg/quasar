use {
    super::*,
    core::mem::{align_of, size_of},
};

/// Prove Clock has alignment 1; required for the pointer cast in
/// `from_bytes_unchecked` (`bytes.as_ptr() as *const Self`).
#[kani::proof]
fn clock_align_one() {
    assert!(align_of::<Clock>() == 1);
}

/// Prove Clock is exactly 40 bytes.
#[kani::proof]
fn clock_size_40() {
    assert!(size_of::<Clock>() == 40);
}
