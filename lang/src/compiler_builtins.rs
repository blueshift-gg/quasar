//! Compiler-builtin `memcmp` for the SBF/BPF target.
//!
//! The SBF platform has no libc, so the compiler-generated calls to `memcmp`
//! that back `Ord`/`PartialEq`/`sort`/`BTreeMap` on byte slices have no symbol
//! to link against. This crate provides one with the C `memcmp` contract (the
//! sign of the first differing byte, not a boolean).
//!
//! Small compares (`n <= INLINE_MEMCMP_THRESHOLD`) are done inline to avoid the
//! syscall's fixed overhead; larger ones dispatch to the `sol_memcmp` syscall,
//! which is already correct. The threshold (32 bytes) is tuned so the inline
//! word-at-a-time scan wins for the short keys typical of on-chain data
//! (discriminators, addresses) while the syscall handles bulk buffers.
//!
//! On non-BPF targets only the unit-test implementation is compiled; host
//! builds use the platform libc.

#[cfg(target_arch = "bpf")]
const SOL_MEMCMP: usize = 0x5FDCDE31;
#[cfg(target_arch = "bpf")]
const INLINE_MEMCMP_THRESHOLD: usize = 32;

#[cfg(target_arch = "bpf")]
#[inline(always)]
fn sol_memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut result = 0i32;
    let syscall: unsafe extern "C" fn(*const u8, *const u8, usize, *mut i32) -> i32 =
        unsafe { core::mem::transmute(SOL_MEMCMP) };
    unsafe {
        syscall(a, b, n, &mut result as *mut i32);
    }
    result
}

/// Compare `n` bytes of `a` and `b` following the C `memcmp` contract: return
/// the signed difference of the first differing byte (`a[i] - b[i]`), or `0`
/// when all `n` bytes are equal.
///
/// `Ord`/`sort`/`BTreeMap` over byte slices lower to `memcmp` and rely on this
/// sign, so a mismatch must return the sign of the first differing byte rather
/// than a constant. The equal-path word loop is kept for CU: on a word mismatch
/// it breaks to a byte scan (starting at that word) to locate the first
/// differing byte.
///
/// # Safety
///
/// `a` and `b` must be valid for reads of `n` bytes.
#[cfg(any(target_arch = "bpf", test))]
#[inline(always)]
unsafe fn cmp_bytes(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0usize;
    while i + 8 <= n {
        // SAFETY: the caller guarantees both pointers are readable for `n`
        // bytes; this iteration stays within that range and permits unaligned
        // inputs by using `read_unaligned`.
        let wa = unsafe { core::ptr::read_unaligned(a.add(i) as *const u64) };
        // SAFETY: same range and alignment argument as for `wa`.
        let wb = unsafe { core::ptr::read_unaligned(b.add(i) as *const u64) };
        if wa != wb {
            // Mismatch somewhere in this word; the byte scan below finds the
            // first differing byte and returns its sign.
            break;
        }
        i += 8;
    }

    while i < n {
        // SAFETY: `i < n` and the caller guarantees `a` is readable for `n`.
        let ba = unsafe { *a.add(i) };
        // SAFETY: `i < n` and the caller guarantees `b` is readable for `n`.
        let bb = unsafe { *b.add(i) };
        if ba != bb {
            return ba as i32 - bb as i32;
        }
        i += 1;
    }

    0
}

#[cfg(target_arch = "bpf")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    if n > INLINE_MEMCMP_THRESHOLD {
        sol_memcmp(a, b, n)
    } else {
        // SAFETY: the C `memcmp` contract guarantees `a` and `b` are valid for
        // reads of `n` bytes.
        unsafe { cmp_bytes(a, b, n) }
    }
}

#[cfg(test)]
mod tests {
    use super::cmp_bytes;

    fn cmp(a: &[u8], b: &[u8]) -> i32 {
        assert_eq!(a.len(), b.len());
        // SAFETY: `a`/`b` are valid for reads of `a.len()` bytes.
        unsafe { cmp_bytes(a.as_ptr(), b.as_ptr(), a.len()) }
    }

    #[test]
    fn equal_returns_zero() {
        // Multi-word equal run exercises the word loop to completion.
        let a = [0x11u8; 24];
        let b = [0x11u8; 24];
        assert_eq!(cmp(&a, &b), 0);
    }

    #[test]
    fn len_zero_returns_zero() {
        let a: [u8; 0] = [];
        let b: [u8; 0] = [];
        assert_eq!(cmp(&a, &b), 0);
    }

    #[test]
    fn sign_at_byte_zero() {
        // Difference in the very first byte, both directions.
        let a = [0x01u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let b = [0x02u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(cmp(&a, &b) < 0);
        assert!(cmp(&b, &a) > 0);
        assert_eq!(cmp(&a, &b), 0x01 - 0x02);
    }

    #[test]
    fn sign_within_first_word() {
        // First word mismatches at byte 3; the word loop must break and the
        // byte scan must return the sign of byte 3.
        let mut a = [0u8; 8];
        let mut b = [0u8; 8];
        a[3] = 0x10;
        b[3] = 0x20;
        assert!(cmp(&a, &b) < 0);
        assert!(cmp(&b, &a) > 0);
    }

    #[test]
    fn sign_in_tail_after_full_word() {
        // First 8 bytes equal (one full word), difference in the tail byte 9.
        let mut a = [0x07u8; 11];
        let mut b = [0x07u8; 11];
        a[9] = 0xF0;
        b[9] = 0x0F;
        // 0xF0 (240) > 0x0F (15): a is greater.
        assert!(cmp(&a, &b) > 0);
        assert!(cmp(&b, &a) < 0);
    }

    #[test]
    fn sign_matches_slice_ordering() {
        // The sign must agree with `[u8]`'s `Ord` for the cases sort/BTreeMap
        // depend on (mismatch after a full equal word, unsigned byte compare).
        let cases: &[(&[u8], &[u8])] = &[
            (&[1, 2, 3, 4, 5, 6, 7, 8, 9], &[1, 2, 3, 4, 5, 6, 7, 8, 10]),
            (
                &[0, 0, 0, 0, 0, 0, 0, 0, 0x80],
                &[0, 0, 0, 0, 0, 0, 0, 0, 0x7F],
            ),
            (&[9, 9, 9, 9], &[9, 9, 9, 9]),
            (&[0xFF, 0x00], &[0x00, 0xFF]),
        ];
        for (a, b) in cases {
            assert_eq!(
                cmp(a, b).signum(),
                match a.cmp(b) {
                    core::cmp::Ordering::Less => -1,
                    core::cmp::Ordering::Equal => 0,
                    core::cmp::Ordering::Greater => 1,
                },
                "cmp_bytes sign must match slice Ord for {a:?} vs {b:?}",
            );
        }
    }
}
