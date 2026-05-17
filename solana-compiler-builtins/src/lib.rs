#![cfg_attr(target_arch = "bpf", no_std)]

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

#[cfg(target_arch = "bpf")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    if n > INLINE_MEMCMP_THRESHOLD {
        sol_memcmp(a, b, n)
    } else {
        let mut i = 0usize;
        while i + 8 <= n {
            let wa = unsafe { core::ptr::read_unaligned(a.add(i) as *const u64) };
            let wb = unsafe { core::ptr::read_unaligned(b.add(i) as *const u64) };
            if wa != wb {
                return 1;
            }
            i += 8;
        }

        while i < n {
            if unsafe { *a.add(i) != *b.add(i) } {
                return 1;
            }
            i += 1;
        }

        0
    }
}
